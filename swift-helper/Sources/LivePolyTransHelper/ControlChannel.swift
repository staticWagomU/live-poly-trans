import Foundation

/// Line-oriented control protocol on the helper's stdin.
///
/// The app keeps one helper running per stream for the whole session and
/// switches recording on and off around it with these JSON lines, so a
/// recording can start and end without restarting audio capture. EOF on
/// stdin still means shutdown (the Rust side drops the pipe to stop us).
public enum HelperControlCommand: Equatable, Sendable {
  case startRecording(directory: String)
  case stopRecording
}

/// Returns nil for anything that is not a well-formed control line. Unknown
/// commands are ignored rather than fatal so an older helper paired with a
/// newer app degrades to "no recording" instead of dying mid-transcription.
public func parseHelperControlLine(_ line: String) -> HelperControlCommand? {
  guard
    let data = line.data(using: .utf8),
    let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
    let command = object["cmd"] as? String
  else {
    return nil
  }

  switch command {
  case "start-recording":
    guard let directory = object["dir"] as? String, !directory.isEmpty else {
      return nil
    }
    return .startRecording(directory: directory)
  case "stop-recording":
    return .stopRecording
  default:
    return nil
  }
}

/// Mirrors the Rust side's `next_recording_file_path`: a helper restarted
/// mid-recording must append a numbered file instead of clobbering the audio
/// the previous run already wrote into the same recording directory.
public func nextRecordingFilePath(
  directory: String,
  stream: AudioStream,
  fileExists: (String) -> Bool = { FileManager.default.fileExists(atPath: $0) }
) -> String {
  let directoryUrl = URL(fileURLWithPath: directory)
  let base = directoryUrl.appendingPathComponent("\(stream.rawValue).m4a").path
  guard fileExists(base) else {
    return base
  }

  for attempt in 2..<1000 {
    let candidate = directoryUrl.appendingPathComponent("\(stream.rawValue)-\(attempt).m4a").path
    if !fileExists(candidate) {
      return candidate
    }
  }

  return base
}

public func recordingTranscriptFilePath(directory: String, stream: AudioStream) -> String {
  URL(fileURLWithPath: directory).appendingPathComponent("\(stream.rawValue).jsonl").path
}

/// Reads stdin until EOF, applying recording control lines to the running
/// capture. Replaces the old "drain stdin, EOF = shutdown" loop; EOF still
/// calls `onShutdown` exactly as before.
public func runHelperControlLoop(
  stream: AudioStream,
  sink: RecordingSink,
  emitter: HelperEventEmitter,
  onShutdown: @escaping @Sendable () -> Void
) {
  DispatchQueue.global().async {
    while let line = readLine(strippingNewline: true) {
      guard let command = parseHelperControlLine(line) else {
        continue
      }

      switch command {
      case let .startRecording(directory):
        helperDebugLog("control-start-recording stream=\(stream.rawValue) dir=\(directory)")
        sink.startRecording(directory: directory, stream: stream)
        let writer = JsonlFileWriter(
          path: recordingTranscriptFilePath(directory: directory, stream: stream)
        )
        Task {
          await emitter.setTranscriptWriter(writer)
          await emitter.emitStatus(StatusEvent(stream: stream, state: "recording-started"))
        }
      case .stopRecording:
        helperDebugLog("control-stop-recording stream=\(stream.rawValue)")
        sink.stopRecording()
        Task {
          await emitter.setTranscriptWriter(nil)
          await emitter.emitStatus(StatusEvent(stream: stream, state: "recording-stopped"))
        }
      }
    }

    onShutdown()
  }
}
