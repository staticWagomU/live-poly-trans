import AVFAudio
import Foundation

public enum AudioFileToolsError: Error, CustomStringConvertible {
  case missingTargetFormat
  case unreadableChannelData(String)
  case invalidMixInputs(Int)

  public var description: String {
    switch self {
    case .missingTargetFormat:
      "Could not create the audio processing format."
    case let .unreadableChannelData(path):
      "Could not read float channel data from: \(path)"
    case let .invalidMixInputs(count):
      "Mixing requires exactly 2 input files, got \(count)."
    }
  }
}

/// Writes float samples as a 16-bit mono WAV, the input whisper-cli decodes
/// without resampling when the sample rate is already 16 kHz.
public func writeInt16MonoWav(samples: [Float], sampleRate: Double, to url: URL) throws {
  guard
    let bufferFormat = AVAudioFormat(
      commonFormat: .pcmFormatFloat32,
      sampleRate: sampleRate,
      channels: 1,
      interleaved: false
    )
  else {
    throw AudioFileToolsError.missingTargetFormat
  }

  let settings: [String: Any] = [
    AVFormatIDKey: kAudioFormatLinearPCM,
    AVSampleRateKey: sampleRate,
    AVNumberOfChannelsKey: 1,
    AVLinearPCMBitDepthKey: 16,
    AVLinearPCMIsFloatKey: false,
    AVLinearPCMIsBigEndianKey: false,
    AVLinearPCMIsNonInterleaved: false,
  ]
  let file = try AVAudioFile(
    forWriting: url,
    settings: settings,
    commonFormat: .pcmFormatFloat32,
    interleaved: false
  )

  guard
    let buffer = AVAudioPCMBuffer(
      pcmFormat: bufferFormat,
      frameCapacity: AVAudioFrameCount(samples.count)
    ),
    let channelData = buffer.floatChannelData
  else {
    throw AudioFileToolsError.unreadableChannelData(url.path)
  }

  buffer.frameLength = AVAudioFrameCount(samples.count)
  samples.withUnsafeBufferPointer { pointer in
    guard let baseAddress = pointer.baseAddress else {
      return
    }

    channelData[0].update(from: baseAddress, count: samples.count)
  }

  try file.write(from: buffer)
}

public struct WaveformData: Codable, Equatable {
  public let durationMs: Int64
  public let peaks: [Double]

  public init(durationMs: Int64, peaks: [Double]) {
    self.durationMs = durationMs
    self.peaks = peaks
  }
}

public let defaultWaveformBuckets = 1_200
private let audioFileChunkFrames: AVAudioFrameCount = 65_536

public func waveformBucketIndex(frame: Int64, framesPerBucket: Int64, bucketCount: Int) -> Int {
  guard framesPerBucket > 0, bucketCount > 0 else {
    return 0
  }

  return min(bucketCount - 1, Int(frame / framesPerBucket))
}

public func mixedSampleValue(_ left: Float, _ right: Float) -> Float {
  min(1, max(-1, left + right))
}

/// Extracts per-bucket peak amplitudes without decoding the whole file into
/// memory; hour-long recordings stream through in fixed-size chunks.
public func computeWaveform(path: String, buckets: Int = defaultWaveformBuckets) throws -> WaveformData {
  let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
  let sampleRate = file.processingFormat.sampleRate
  let totalFrames = Int64(file.length)
  let durationMs = sampleRate > 0 ? Int64((Double(totalFrames) / sampleRate * 1000).rounded()) : 0

  guard totalFrames > 0 else {
    return WaveformData(durationMs: 0, peaks: [])
  }

  let bucketCount = max(1, min(buckets, Int(totalFrames)))
  let framesPerBucket = max(1, totalFrames / Int64(bucketCount))
  var peaks = [Double](repeating: 0, count: bucketCount)
  var frameIndex: Int64 = 0

  guard let chunk = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: audioFileChunkFrames) else {
    throw AudioFileToolsError.missingTargetFormat
  }

  while true {
    try file.read(into: chunk)
    let frames = Int(chunk.frameLength)
    guard frames > 0 else {
      break
    }

    guard let channels = chunk.floatChannelData else {
      throw AudioFileToolsError.unreadableChannelData(path)
    }

    let channelCount = Int(chunk.format.channelCount)
    for frame in 0..<frames {
      var amplitude: Float = 0
      for channel in 0..<channelCount {
        amplitude = max(amplitude, abs(channels[channel][frame]))
      }

      let bucket = waveformBucketIndex(
        frame: frameIndex + Int64(frame),
        framesPerBucket: framesPerBucket,
        bucketCount: bucketCount
      )
      peaks[bucket] = max(peaks[bucket], Double(amplitude))
    }

    frameIndex += Int64(frames)
  }

  return WaveformData(
    durationMs: durationMs,
    peaks: peaks.map { ($0 * 10_000).rounded() / 10_000 }
  )
}

/// Streams two recordings (mic + speaker) into a single mono AAC mix.
public func mixAudioFiles(inputs: [String], outputPath: String) throws {
  guard inputs.count == 2 else {
    throw AudioFileToolsError.invalidMixInputs(inputs.count)
  }

  guard
    let targetFormat = AVAudioFormat(
      standardFormatWithSampleRate: recordingSampleRate,
      channels: recordingChannelCount
    )
  else {
    throw AudioFileToolsError.missingTargetFormat
  }

  let readers = try inputs.map { try ConvertedAudioReader(path: $0, targetFormat: targetFormat) }
  let outputURL = URL(fileURLWithPath: outputPath)
  try FileManager.default.createDirectory(
    at: outputURL.deletingLastPathComponent(),
    withIntermediateDirectories: true
  )
  let outputFile = try AVAudioFile(
    forWriting: outputURL,
    settings: [
      AVFormatIDKey: kAudioFormatMPEG4AAC,
      AVSampleRateKey: recordingSampleRate,
      AVNumberOfChannelsKey: Int(recordingChannelCount),
      AVEncoderBitRateKey: 96_000
    ],
    commonFormat: .pcmFormatFloat32,
    interleaved: false
  )

  let mixFrames = 4_096
  var left = [Float](repeating: 0, count: mixFrames)
  var right = [Float](repeating: 0, count: mixFrames)

  while true {
    left.replaceSubrange(0..<mixFrames, with: repeatElement(0, count: mixFrames))
    right.replaceSubrange(0..<mixFrames, with: repeatElement(0, count: mixFrames))

    let leftFrames = try left.withUnsafeMutableBufferPointer {
      try readers[0].read(into: $0.baseAddress!, frameCount: mixFrames)
    }
    let rightFrames = try right.withUnsafeMutableBufferPointer {
      try readers[1].read(into: $0.baseAddress!, frameCount: mixFrames)
    }

    let frames = max(leftFrames, rightFrames)
    guard frames > 0 else {
      break
    }

    guard
      let outputBuffer = AVAudioPCMBuffer(
        pcmFormat: targetFormat,
        frameCapacity: AVAudioFrameCount(frames)
      ),
      let outputChannel = outputBuffer.floatChannelData?[0]
    else {
      throw AudioFileToolsError.missingTargetFormat
    }

    for frame in 0..<frames {
      outputChannel[frame] = mixedSampleValue(left[frame], right[frame])
    }

    outputBuffer.frameLength = AVAudioFrameCount(frames)
    try outputFile.write(from: outputBuffer)
  }
}

/// Reads an audio file and hands out fixed-size mono float chunks in the
/// target format, converting sample rate/channels on the fly.
private final class ConvertedAudioReader {
  private let path: String
  private let file: AVAudioFile
  private let targetFormat: AVAudioFormat
  private let converter: AVAudioConverter?
  private var pending: AVAudioPCMBuffer?
  private var pendingOffset = 0
  private var exhausted = false

  init(path: String, targetFormat: AVAudioFormat) throws {
    self.path = path
    self.file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
    self.targetFormat = targetFormat
    self.converter = audioConverter(from: file.processingFormat, to: targetFormat)
  }

  func read(into destination: UnsafeMutablePointer<Float>, frameCount: Int) throws -> Int {
    var written = 0

    while written < frameCount {
      if pending == nil || pendingOffset >= Int(pending?.frameLength ?? 0) {
        guard let next = try readNextConvertedChunk() else {
          break
        }

        pending = next
        pendingOffset = 0
      }

      guard let pending, let source = pending.floatChannelData?[0] else {
        throw AudioFileToolsError.unreadableChannelData(path)
      }

      let available = Int(pending.frameLength) - pendingOffset
      let copyCount = min(available, frameCount - written)
      memcpy(
        destination + written,
        source + pendingOffset,
        copyCount * MemoryLayout<Float>.size
      )
      written += copyCount
      pendingOffset += copyCount
    }

    return written
  }

  private func readNextConvertedChunk() throws -> AVAudioPCMBuffer? {
    guard !exhausted else {
      return nil
    }

    guard
      let chunk = AVAudioPCMBuffer(
        pcmFormat: file.processingFormat,
        frameCapacity: audioFileChunkFrames
      )
    else {
      throw AudioFileToolsError.missingTargetFormat
    }

    try file.read(into: chunk)
    guard chunk.frameLength > 0 else {
      exhausted = true
      return nil
    }

    return try convertBuffer(chunk, to: targetFormat, using: converter)
  }
}
