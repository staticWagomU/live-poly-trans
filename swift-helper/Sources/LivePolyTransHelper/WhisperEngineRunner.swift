@preconcurrency import AVFAudio
import Foundation

public final class WhisperEngineRunner: @unchecked Sendable {
  public let outputLines: AsyncStream<String>
  private let process: Process
  private let stdin: FileHandle
  private let stderrCollector: PipeCollector
  private let outputContinuation: AsyncStream<String>.Continuation
  private let writeLock = NSLock()

  public init(enginePath: String) throws {
    process = Process()
    process.executableURL = URL(fileURLWithPath: enginePath)
    process.arguments = []

    let stdinPipe = Pipe()
    let stdoutPipe = Pipe()
    let stderrPipe = Pipe()
    process.standardInput = stdinPipe
    process.standardOutput = stdoutPipe
    process.standardError = stderrPipe
    stdin = stdinPipe.fileHandleForWriting
    stderrCollector = PipeCollector(stderrPipe)

    var continuation: AsyncStream<String>.Continuation!
    outputLines = AsyncStream<String> { continuation = $0 }
    outputContinuation = continuation

    let lineAccumulator = JsonLineAccumulator()
    stdoutPipe.fileHandleForReading.readabilityHandler = { [outputContinuation, lineAccumulator] handle in
      let data = handle.availableData
      guard !data.isEmpty else {
        handle.readabilityHandler = nil
        for line in lineAccumulator.finish() {
          outputContinuation.yield(line)
        }
        outputContinuation.finish()
        return
      }

      for line in lineAccumulator.consume(data) {
        outputContinuation.yield(line)
      }
    }
  }

  public func start() throws {
    try process.run()
  }

  public func sendLine(_ line: String) throws {
    writeLock.lock()
    defer { writeLock.unlock() }
    try stdin.write(contentsOf: Data("\(line)\n".utf8))
  }

  public func closeInput() throws {
    writeLock.lock()
    defer { writeLock.unlock() }
    try stdin.close()
  }

  public func terminate() {
    terminateProcessWithEscalation(process)
  }

  public func stderrTail() -> String {
    stderrCollector.tail()
  }
}

@available(macOS 26.0, *)
public func runWhisperEngineTranscription(
  stream: AudioStream,
  sourceLanguage: String?,
  targetLanguage: String?,
  segmentDirectory: String?,
  recordFile: String? = nil,
  transcriptFile: String? = nil,
  modelPath: String,
  enginePath: String,
  translationEnabled: Bool = true
) async throws {
  helperDebugLog(
    "whisper-engine-stream-start stream=\(stream.rawValue) source=\(sourceLanguage ?? "-") target=\(targetLanguage ?? "-") model=\(modelPath) engine=\(enginePath) record=\(recordFile ?? "-")"
  )

  guard
    let whisperFormat = AVAudioFormat(
      commonFormat: .pcmFormatFloat32,
      sampleRate: whisperSampleRate,
      channels: 1,
      interleaved: false
    )
  else {
    throw AudioFileToolsError.missingTargetFormat
  }

  let emitter = HelperEventEmitter(
    segmentWriter: SegmentWriter(directoryPath: segmentDirectory),
    transcriptWriter: JsonlFileWriter(path: transcriptFile)
  )
  let languages = transcriptionLanguages(
    requestedLanguages: [],
    sourceLanguage: sourceLanguage,
    targetLanguage: targetLanguage
  )
  let translators = translationEnabled ? Dictionary(uniqueKeysWithValues: languages.map { language in
    (
      language,
      LiveTranslator(
        sourceLanguage: language,
        targetLanguage: oppositeLanguage(
          for: language,
          sourceLanguage: sourceLanguage,
          targetLanguage: targetLanguage
        )
      )
    )
  }) : [:]

  let inputSource = try await makeWhisperInputSource(
    stream: stream,
    recordFile: recordFile,
    whisperFormat: whisperFormat
  )
  let runner = try WhisperEngineRunner(enginePath: enginePath)
  try runner.start()

  installWhisperEngineShutdownHandlers(
    stream: stream,
    inputSource: inputSource,
    runner: runner,
    emitter: emitter
  )

  let outputTask = Task {
    for await line in runner.outputLines {
      guard let data = line.data(using: .utf8) else {
        continue
      }
      let output = try? JSONDecoder().decode(WhisperEngineOutputLine.self, from: data)
      guard let output else {
        helperDebugLog("whisper-engine-invalid-json line=\(line)")
        continue
      }

      switch output.type {
      case "status":
        helperDebugLog("whisper-engine-status stream=\(stream.rawValue) state=\(output.state ?? "-")")
      case "error":
        let message = output.message ?? "unknown whisper engine error"
        helperDebugLog("whisper-engine-error stream=\(stream.rawValue) fatal=\(output.fatal ?? false) message=\(message)")
        if output.fatal == true {
          inputSource.cleanup()
        }
      case "transcript":
        if output.isFinal == true,
          let candidate = transcriptCandidate(
            from: output,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage
          )
        {
          await emitFinalTranscript(
            candidate,
            stream: stream,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage,
            translators: translators,
            emitter: emitter
          )
        } else if let event = transcriptEvent(
          from: output,
          stream: stream,
          sourceLanguage: sourceLanguage,
          targetLanguage: targetLanguage
        ) {
          await emitter.emitInterim(event)
        }
      default:
        helperDebugLog("whisper-engine-ignored-output type=\(output.type)")
      }
    }
  }

  do {
    try runner.sendLine(whisperEngineConfigLine(
      modelPath: modelPath,
      language: "auto",
      sampleRate: Int(whisperSampleRate)
    ))
    await emitter.emitStatus(StatusEvent(stream: stream, state: "control-ready"))

    var seq = 0
    var timestampMs: Int64 = 0
    for try await captured in inputSource.sequence {
      let samples = floatSamples(from: captured.buffer)
      try runner.sendLine(whisperEngineAudioLine(
        stream: stream,
        seq: seq,
        timestampMs: timestampMs,
        samples: samples
      ))
      seq += 1
      timestampMs += Int64((Double(samples.count) * 1000 / whisperSampleRate).rounded())
    }

    try runner.sendLine(whisperEngineFlushLine(stream: stream))
    try runner.sendLine(whisperEngineShutdownLine())
    try runner.closeInput()
    await outputTask.value
    inputSource.recordingSink.stopRecording()
    helperDebugLog("whisper-engine-stream-finished stream=\(stream.rawValue)")
  } catch {
    helperDebugLog("whisper-engine-stream-error stream=\(stream.rawValue) error=\(diagnosticDescription(for: error)) stderr=\(runner.stderrTail())")
    outputTask.cancel()
    inputSource.cleanup()
    inputSource.recordingSink.stopRecording()
    runner.terminate()
    throw error
  }
}

@available(macOS 26.0, *)
func installWhisperEngineShutdownHandlers(
  stream: AudioStream,
  inputSource: AudioInputSource<CapturedAudioBuffer>,
  runner: WhisperEngineRunner,
  emitter: HelperEventEmitter
) {
  let trigger: @Sendable () -> Void = {
    whisperShutdownOnce.run {
      helperDebugLog("whisper-engine-shutdown-begin stream=\(stream.rawValue)")
      inputSource.cleanup()
      DispatchQueue.global().asyncAfter(deadline: .now() + whisperShutdownGraceSeconds) {
        helperDebugLog("whisper-engine-shutdown-timeout stream=\(stream.rawValue)")
        runner.terminate()
        inputSource.recordingSink.stopRecording()
        exit(0)
      }
    }
  }

  runHelperControlLoop(
    stream: stream,
    sink: inputSource.recordingSink,
    emitter: emitter,
    onShutdown: trigger
  )

  signal(SIGTERM, SIG_IGN)
  let source = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .global())
  source.setEventHandler(handler: trigger)
  source.resume()
  whisperShutdownSignalSource = source
}
