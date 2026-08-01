import Foundation

public struct WhisperCliError: Error, CustomStringConvertible {
  public let message: String

  public var description: String {
    message
  }
}

/// Runs whisper-cli once per utterance chunk inside a per-session temp
/// directory. Each call writes chunk-<startMs>.wav, invokes the cli, parses
/// the JSON it wrote next to it, and removes both files.
///
/// The helper's own stdout is the JSONL event protocol and its stderr is
/// surfaced in the UI as helper-error events, so the subprocess must not
/// inherit either: results go to the null device, logs are collected into
/// the debug log.
final class WhisperCliRunner: @unchecked Sendable {
  private let cliPath: String
  private let modelPath: String
  private let tempDirectory: URL
  private let inFlightLock = NSLock()
  private var inFlightProcess: Process?

  init(cliPath: String, modelPath: String) throws {
    self.cliPath = cliPath
    self.modelPath = modelPath
    self.tempDirectory = FileManager.default.temporaryDirectory
      .appendingPathComponent("lpt-whisper-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
  }

  func transcribe(_ chunk: UtteranceChunk, sampleRate: Double) async throws -> WhisperCliResult {
    let outputBase = tempDirectory.appendingPathComponent("chunk-\(chunk.startMs)")
    let wavUrl = outputBase.appendingPathExtension("wav")
    let jsonUrl = outputBase.appendingPathExtension("json")
    defer {
      try? FileManager.default.removeItem(at: wavUrl)
      try? FileManager.default.removeItem(at: jsonUrl)
    }

    try writeInt16MonoWav(samples: chunk.samples, sampleRate: sampleRate, to: wavUrl)

    let process = Process()
    process.executableURL = URL(fileURLWithPath: cliPath)
    process.arguments = whisperCliArguments(
      modelPath: modelPath,
      audioPath: wavUrl.path,
      outputBase: outputBase.path
    )
    process.standardInput = FileHandle.nullDevice
    process.standardOutput = FileHandle.nullDevice
    let stderrPipe = Pipe()
    process.standardError = stderrPipe
    let stderrCollector = PipeCollector(stderrPipe)

    setInFlight(process)
    defer { setInFlight(nil) }
    let status = try await runUntilExit(process, timeoutSeconds: whisperCliTimeoutSeconds)
    guard status == 0 else {
      throw WhisperCliError(
        message: "whisper-cli exited with status \(status): \(stderrCollector.tail())"
      )
    }

    return try parsedWhisperResult(try Data(contentsOf: jsonUrl))
  }

  /// Kills the currently running whisper-cli, if any. Called on shutdown so
  /// an in-flight inference never outlives the helper as an orphan pegging
  /// the performance cores.
  func terminateInFlight() {
    inFlightLock.lock()
    let process = inFlightProcess
    inFlightLock.unlock()

    if let process {
      terminateProcessWithEscalation(process)
    }
  }

  func cleanup() {
    try? FileManager.default.removeItem(at: tempDirectory)
  }

  private func setInFlight(_ process: Process?) {
    inFlightLock.lock()
    inFlightProcess = process
    inFlightLock.unlock()
  }
}

/// One chunk is at most 28 s of audio; even slow hardware transcribes it in
/// well under this. Hitting the timeout means the cli is wedged (Metal/ANE
/// hang, model on a stalled volume) and blocking the whole queue.
let whisperCliTimeoutSeconds: Double = 120

/// Awaits process exit without blocking a cooperative-pool thread for the
/// duration of the inference. The process is killed on timeout and on task
/// cancellation; both surface as a non-zero exit status through the normal
/// termination path, so the continuation always resumes exactly once.
func runUntilExit(_ process: Process, timeoutSeconds: Double) async throws -> Int32 {
  try await withTaskCancellationHandler {
    try await withCheckedThrowingContinuation { continuation in
      process.terminationHandler = { finished in
        continuation.resume(returning: finished.terminationStatus)
      }

      do {
        try process.run()
        DispatchQueue.global().asyncAfter(deadline: .now() + timeoutSeconds) {
          guard process.isRunning else {
            return
          }

          helperDebugLog("whisper-cli-timeout pid=\(process.processIdentifier)")
          terminateProcessWithEscalation(process)
        }
      } catch {
        process.terminationHandler = nil
        continuation.resume(throwing: error)
      }
    }
  } onCancel: {
    terminateProcessWithEscalation(process)
  }
}

/// SIGTERM first so the cli can drop cleanly, SIGKILL two seconds later if
/// it ignored that.
func terminateProcessWithEscalation(_ process: Process) {
  guard process.isRunning else {
    return
  }

  process.terminate()
  DispatchQueue.global().asyncAfter(deadline: .now() + 2) {
    if process.isRunning {
      kill(process.processIdentifier, SIGKILL)
    }
  }
}

/// Drains a pipe via readabilityHandler so the subprocess never stalls on a
/// full pipe buffer and no thread blocks on the read.
final class PipeCollector: @unchecked Sendable {
  private let lock = NSLock()
  private var collected = Data()

  init(_ pipe: Pipe) {
    pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
      let data = handle.availableData
      guard !data.isEmpty else {
        handle.readabilityHandler = nil
        return
      }

      guard let self else {
        return
      }

      self.lock.lock()
      self.collected.append(data)
      self.lock.unlock()
    }
  }

  func tail(lines: Int = 4) -> String {
    lock.lock()
    let data = collected
    lock.unlock()

    guard let text = String(data: data, encoding: .utf8) else {
      return ""
    }

    return text
      .split(separator: "\n")
      .suffix(lines)
      .joined(separator: " | ")
  }
}
