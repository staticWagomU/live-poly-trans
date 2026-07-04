import AVFAudio
import Foundation

public let recordingSampleRate: Double = 48_000
public let recordingChannelCount: AVAudioChannelCount = 1

public enum AudioRecorderError: Error, CustomStringConvertible {
  case missingRecordingFormat

  public var description: String {
    switch self {
    case .missingRecordingFormat:
      "Could not create the recording audio format."
    }
  }
}

/// Writes captured audio to an AAC (.m4a) file on a serial queue.
/// Buffers must be fed BEFORE any silence gating so the file timeline stays
/// aligned with the analyzer's capture clock (transcript startMs == file
/// position).
public final class AudioRecorder: @unchecked Sendable {
  private let queue = DispatchQueue(label: "com.staticwagomu.live-poly-trans.recorder")
  private let targetFormat: AVAudioFormat
  private let converter: AVAudioConverter?
  private var file: AVAudioFile?
  private var finished = false

  public init?(path: String?, sourceFormat: AVAudioFormat) throws {
    guard let path, !path.isEmpty else {
      return nil
    }

    guard
      let targetFormat = AVAudioFormat(
        standardFormatWithSampleRate: recordingSampleRate,
        channels: recordingChannelCount
      )
    else {
      throw AudioRecorderError.missingRecordingFormat
    }

    self.targetFormat = targetFormat
    self.converter = audioConverter(from: sourceFormat, to: targetFormat)

    let url = URL(fileURLWithPath: path)
    try FileManager.default.createDirectory(
      at: url.deletingLastPathComponent(),
      withIntermediateDirectories: true
    )
    self.file = try AVAudioFile(
      forWriting: url,
      settings: [
        AVFormatIDKey: kAudioFormatMPEG4AAC,
        AVSampleRateKey: recordingSampleRate,
        AVNumberOfChannelsKey: Int(recordingChannelCount),
        AVEncoderBitRateKey: 96_000
      ],
      commonFormat: .pcmFormatFloat32,
      interleaved: false
    )
    helperDebugLog("recorder-open path=\(path) source={\(audioFormatDescription(sourceFormat))}")
  }

  public func write(_ buffer: AVAudioPCMBuffer) {
    queue.async { [self] in
      guard !finished, let file else {
        return
      }

      do {
        let converted = try convertBuffer(buffer, to: targetFormat, using: converter)
        guard converted.frameLength > 0 else {
          return
        }
        try file.write(from: converted)
      } catch {
        helperDebugLog("recorder-write-error error=\(error.localizedDescription)")
      }
    }
  }

  /// Closes the file so the m4a container is finalized. Safe to call twice.
  public func finalize() {
    queue.sync {
      guard !finished else {
        return
      }

      finished = true
      file = nil
      helperDebugLog("recorder-finalized")
    }
  }
}
