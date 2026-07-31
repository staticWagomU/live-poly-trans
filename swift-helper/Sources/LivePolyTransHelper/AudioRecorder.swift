@preconcurrency import AVFAudio
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

/// The recorder a live capture writes through. Capture taps hold this sink
/// for their whole lifetime while the stdin control channel attaches and
/// detaches actual recorders, so a recording can start and end without
/// restarting audio capture.
public final class RecordingSink: @unchecked Sendable {
  private let lock = NSLock()
  private let sourceFormat: AVAudioFormat
  private var recorder: AudioRecorder?

  public init(sourceFormat: AVAudioFormat, initialRecorder: AudioRecorder? = nil) {
    self.sourceFormat = sourceFormat
    self.recorder = initialRecorder
  }

  public func write(_ buffer: AVAudioPCMBuffer) {
    lock.lock()
    let current = recorder
    lock.unlock()
    current?.write(buffer)
  }

  public func startRecording(directory: String, stream: AudioStream) {
    let path = nextRecordingFilePath(directory: directory, stream: stream)
    do {
      let newRecorder = try AudioRecorder(path: path, sourceFormat: sourceFormat)
      lock.lock()
      let previous = recorder
      recorder = newRecorder
      lock.unlock()
      previous?.finalize()
    } catch {
      helperDebugLog("recording-start-failed path=\(path) error=\(error.localizedDescription)")
      // Plain (non-debug) stderr surfaces in the UI as a helper-error event.
      FileHandle.standardError.write(
        Data("Could not start the audio recording: \(error.localizedDescription)\n".utf8)
      )
    }
  }

  /// Finalizes and detaches the current recorder, if any. Also the shutdown
  /// path, so a recording in progress survives an app quit as a valid m4a.
  public func stopRecording() {
    lock.lock()
    let current = recorder
    recorder = nil
    lock.unlock()
    current?.finalize()
  }
}
