import AVFAudio
import Foundation

public let helperDebugLogPrefix = "live-poly-trans-helper debug:"

public func helperDebugLog(_ message: @autoclosure () -> String) {
  let line = "\(helperDebugLogPrefix) \(ISO8601DateFormatter().string(from: Date())) \(message())\n"
  fputs(line, stderr)
  fflush(stderr)
}

public func audioFormatDescription(_ format: AVAudioFormat) -> String {
  let description = format.streamDescription.pointee
  return [
    "sampleRate=\(format.sampleRate)",
    "channels=\(format.channelCount)",
    "commonFormat=\(format.commonFormat.rawValue)",
    "interleaved=\(format.isInterleaved)",
    "bytesPerFrame=\(description.mBytesPerFrame)",
    "framesPerPacket=\(description.mFramesPerPacket)"
  ].joined(separator: " ")
}

public final class AudioDebugCounter: @unchecked Sendable {
  private let label: String
  private let lock = NSLock()
  private var count = 0

  public init(label: String) {
    self.label = label
  }

  public func record(_ buffer: AVAudioPCMBuffer) {
    lock.lock()
    count += 1
    let currentCount = count
    lock.unlock()

    guard currentCount == 1 || currentCount % 100 == 0 else {
      return
    }

    helperDebugLog(
      "\(label) audio-buffer count=\(currentCount) frames=\(buffer.frameLength) level={\(audioLevelDescription(buffer))} format={\(audioFormatDescription(buffer.format))}"
    )
  }
}

public func audioLevelDescription(_ buffer: AVAudioPCMBuffer) -> String {
  let buffers = UnsafeMutableAudioBufferListPointer(buffer.mutableAudioBufferList)
  return audioLevelDescription(buffers: buffers, commonFormat: buffer.format.commonFormat)
}

public func audioLevelDescription(_ bufferList: UnsafePointer<AudioBufferList>, format: AVAudioFormat) -> String {
  let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: bufferList))
  return audioLevelDescription(buffers: buffers, commonFormat: format.commonFormat)
}

public func audioLevelDescription(_ bufferList: UnsafeMutablePointer<AudioBufferList>, format: AVAudioFormat) -> String {
  let buffers = UnsafeMutableAudioBufferListPointer(bufferList)
  return audioLevelDescription(buffers: buffers, commonFormat: format.commonFormat)
}

private func audioLevelDescription(
  buffers: UnsafeMutableAudioBufferListPointer,
  commonFormat: AVAudioCommonFormat
) -> String {
  var sampleCount = 0
  var sumSquares = 0.0
  var peak = 0.0

  for audioBuffer in buffers {
    guard let data = audioBuffer.mData else {
      continue
    }

    switch commonFormat {
    case .pcmFormatFloat32:
      accumulateSamples(
        data.assumingMemoryBound(to: Float.self),
        count: Int(audioBuffer.mDataByteSize) / MemoryLayout<Float>.size,
        scale: 1,
        sampleCount: &sampleCount,
        sumSquares: &sumSquares,
        peak: &peak
      )
    case .pcmFormatFloat64:
      accumulateSamples(
        data.assumingMemoryBound(to: Double.self),
        count: Int(audioBuffer.mDataByteSize) / MemoryLayout<Double>.size,
        scale: 1,
        sampleCount: &sampleCount,
        sumSquares: &sumSquares,
        peak: &peak
      )
    case .pcmFormatInt16:
      accumulateSamples(
        data.assumingMemoryBound(to: Int16.self),
        count: Int(audioBuffer.mDataByteSize) / MemoryLayout<Int16>.size,
        scale: Double(Int16.max),
        sampleCount: &sampleCount,
        sumSquares: &sumSquares,
        peak: &peak
      )
    case .pcmFormatInt32:
      accumulateSamples(
        data.assumingMemoryBound(to: Int32.self),
        count: Int(audioBuffer.mDataByteSize) / MemoryLayout<Int32>.size,
        scale: Double(Int32.max),
        sampleCount: &sampleCount,
        sumSquares: &sumSquares,
        peak: &peak
      )
    default:
      return "unsupported-format samples=0 rms=0.000000 peak=0.000000"
    }
  }

  guard sampleCount > 0 else {
    return "samples=0 rms=0.000000 peak=0.000000"
  }

  let rms = sqrt(sumSquares / Double(sampleCount))
  return String(format: "samples=%d rms=%.6f peak=%.6f", sampleCount, rms, peak)
}

private func accumulateSamples<T: BinaryFloatingPoint>(
  _ samples: UnsafePointer<T>,
  count: Int,
  scale: Double,
  sampleCount: inout Int,
  sumSquares: inout Double,
  peak: inout Double
) {
  for index in 0..<count {
    let value = Double(samples[index]) / scale
    sampleCount += 1
    sumSquares += value * value
    peak = max(peak, abs(value))
  }
}

private func accumulateSamples<T: BinaryInteger>(
  _ samples: UnsafePointer<T>,
  count: Int,
  scale: Double,
  sampleCount: inout Int,
  sumSquares: inout Double,
  peak: inout Double
) {
  for index in 0..<count {
    let value = Double(samples[index]) / scale
    sampleCount += 1
    sumSquares += value * value
    peak = max(peak, abs(value))
  }
}
