@preconcurrency import AVFAudio
import Foundation
import Speech
import CoreMedia

@available(macOS 26.0, *)
public func runMicrophoneTranscription(
  stream: AudioStream = .mic,
  sourceLanguage: String?,
  targetLanguage: String?,
  languages requestedLanguages: [String],
  segmentDirectory: String?,
  recordFile: String? = nil,
  transcriptFile: String? = nil
) async throws {
  let languages = transcriptionLanguages(
    requestedLanguages: requestedLanguages,
    sourceLanguage: sourceLanguage,
    targetLanguage: targetLanguage
  )
  helperDebugLog(
    "stream-start stream=\(stream.rawValue) source=\(sourceLanguage ?? "-") target=\(targetLanguage ?? "-") languages=\(languages.joined(separator: ",")) record=\(recordFile ?? "-")"
  )
  let transcribers = languages.map {
    SpeechTranscriber(
      locale: Locale(identifier: $0),
      transcriptionOptions: [],
      reportingOptions: [.volatileResults, .fastResults],
      attributeOptions: transcriptAttributeOptions()
    )
  }
  let emitter = HelperEventEmitter(
    segmentWriter: SegmentWriter(directoryPath: segmentDirectory),
    transcriptWriter: JsonlFileWriter(path: transcriptFile)
  )
  try await ensureInstalledLanguages(
    languages,
    transcribers: transcribers,
    stream: stream,
    emitter: emitter
  )
  helperDebugLog("installed-language-check-ok languages=\(languages.joined(separator: ","))")

  let analyzer = SpeechAnalyzer(modules: transcribers)
  let inputSource = try await makeInputSource(
    stream: stream,
    analyzer: analyzer,
    transcribers: transcribers,
    recordFile: recordFile,
    emitter: emitter
  )

  let translators = Dictionary(uniqueKeysWithValues: languages.map { language in
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
  })
  let arbiter = TranscriptArbiter(languageCount: languages.count) { output in
    await emitArbitratedOutput(
      output,
      stream: stream,
      sourceLanguage: sourceLanguage,
      targetLanguage: targetLanguage,
      translators: translators,
      emitter: emitter
    )
  }

  let resultTasks = zip(languages, transcribers).map { language, transcriber in
    Task {
      try await consumeResults(
        stream: stream,
        language: language,
        transcriber: transcriber,
        arbiter: arbiter
      )
    }
  }

  installShutdownHandlers(
    stream: stream,
    analyzer: analyzer,
    inputSource: inputSource,
    emitter: emitter
  )
  await emitter.emitStatus(StatusEvent(stream: stream, state: "control-ready"))

  do {
    helperDebugLog("speech-analyzer-start stream=\(stream.rawValue)")
    try await analyzer.start(inputSequence: inputSource.sequence)
    for task in resultTasks {
      try await task.value
    }
    await arbiter.flushAll()
    inputSource.recordingSink.stopRecording()
    helperDebugLog("stream-finished stream=\(stream.rawValue)")
  } catch {
    helperDebugLog("stream-error stream=\(stream.rawValue) error=\(diagnosticDescription(for: error))")
    resultTasks.forEach { $0.cancel() }
    inputSource.cleanup()
    inputSource.recordingSink.stopRecording()
    throw error
  }
}

// MARK: - Arbitrated event emission

@available(macOS 26.0, *)
private func emitArbitratedOutput(
  _ output: TranscriptArbiter.Output,
  stream: AudioStream,
  sourceLanguage: String?,
  targetLanguage: String?,
  translators: [String: LiveTranslator],
  emitter: HelperEventEmitter
) async {
  switch output {
  case let .interim(candidate):
    let event = transcriptEvent(
      stream: stream,
      language: candidate.language,
      text: candidate.text,
      translation: nil,
      isFinal: false,
      timestamp: Date(),
      segmentId: candidate.segmentId,
      confidence: candidate.confidence,
      spans: candidate.spans
    )
    await emitter.emitInterim(event)
  case let .final(candidate):
    await emitFinalTranscript(
      candidate,
      stream: stream,
      sourceLanguage: sourceLanguage,
      targetLanguage: targetLanguage,
      translators: translators,
      emitter: emitter
    )
  }
}

@available(macOS 26.0, *)
private func consumeResults(
  stream: AudioStream,
  language: String,
  transcriber: SpeechTranscriber,
  arbiter: TranscriptArbiter
) async throws {
  helperDebugLog("result-listener-start stream=\(stream.rawValue) language=\(language)")

  for try await result in transcriber.results {
    let text = String(result.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)

    guard isMeaningfulTranscript(text, isFinal: result.isFinal) else {
      continue
    }

    let spans = transcriptSpans(from: result.text)
    let detection = detectedTranscriptLanguage(text)
    let candidate = TranscriptCandidate(
      language: language,
      text: text,
      isFinal: result.isFinal,
      startMs: milliseconds(from: result.range.start) ?? 0,
      durationMs: milliseconds(from: result.range.duration) ?? 0,
      confidence: transcriptConfidence(spans: spans),
      detectedLanguage: detection?.language,
      detectedLanguageConfidence: detection?.confidence,
      spans: spans
    )

    if result.isFinal {
      helperDebugLog(
        "speech-result stream=\(stream.rawValue) language=\(language) segment=\(candidate.segmentId) chars=\(text.count) text=\"\(text)\""
      )
    }

    await arbiter.receive(candidate)
  }

  helperDebugLog("result-listener-end stream=\(stream.rawValue) language=\(language)")
}

// MARK: - Graceful shutdown

/// Triggered by stdin EOF (Rust drops the pipe on stop) or SIGTERM. Ending
/// the input sequence lets the analyzer flush pending finals and lets the
/// recorder finalize the m4a container instead of being SIGKILLed mid-write.
private let shutdownOnce = ShutdownOnce()
private nonisolated(unsafe) var shutdownSignalSource: DispatchSourceSignal?

final class ShutdownOnce: @unchecked Sendable {
  private let lock = NSLock()
  private var executed = false

  func run(_ work: () -> Void) {
    lock.lock()
    let shouldRun = !executed
    executed = true
    lock.unlock()

    if shouldRun {
      work()
    }
  }
}

@available(macOS 26.0, *)
private func installShutdownHandlers(
  stream: AudioStream,
  analyzer: SpeechAnalyzer,
  inputSource: AudioInputSource<AnalyzerInput>,
  emitter: HelperEventEmitter
) {
  let trigger: @Sendable () -> Void = {
    shutdownOnce.run {
      helperDebugLog("shutdown-begin stream=\(stream.rawValue)")
      inputSource.cleanup()
      Task {
        try? await analyzer.finalizeAndFinishThroughEndOfInput()
      }
      DispatchQueue.global().asyncAfter(deadline: .now() + 5) {
        helperDebugLog("shutdown-timeout stream=\(stream.rawValue)")
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
  shutdownSignalSource = source
}

// MARK: - Input sources

@available(macOS 26.0, *)
private func makeInputSource(
  stream: AudioStream,
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber],
  recordFile: String?,
  emitter: HelperEventEmitter
) async throws -> AudioInputSource<AnalyzerInput> {
  switch stream {
  case .mic:
    return try await makeMicrophoneInputSource(
      analyzer: analyzer,
      transcribers: transcribers,
      recordFile: recordFile,
      emitter: emitter
    )
  case .speaker:
    return try await makeSpeakerInputSource(
      analyzer: analyzer,
      transcribers: transcribers,
      recordFile: recordFile,
      emitter: emitter
    )
  }
}

@available(macOS 26.0, *)
private func makeMicrophoneInputSource(
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber],
  recordFile: String?,
  emitter: HelperEventEmitter
) async throws -> AudioInputSource<AnalyzerInput> {
  let levelLimiter = AudioLevelEventLimiter(minimumInterval: audioLevelEventMinimumInterval)
  try await makeMicrophoneCaptureSource(
    recordFile: recordFile,
    targetFormat: { naturalFormat in
      let analyzerFormat = try await analyzerAudioFormat(
        compatibleWith: transcribers,
        naturalFormat: naturalFormat
      )
      try await analyzer.prepareToAnalyze(in: analyzerFormat)
      helperDebugLog("mic-analyzer-prepared")
      return analyzerFormat
    },
    onAudioLevel: { level in
      let timestamp = Date()
      guard levelLimiter.shouldEmit(at: timestamp) else {
        return
      }
      Task {
        await emitter.emitAudioLevel(audioLevelEvent(stream: .mic, level: level, timestamp: timestamp))
      }
    },
    transform: { AnalyzerInput(buffer: $0.buffer) }
  )
}

/// Engine-agnostic mic capture: taps the input node, converts every buffer
/// to the resolved target format, and yields whatever `transform` makes of
/// it. `targetFormat` receives the microphone's natural format so callers
/// can negotiate (builtin) or fix (whisper) the conversion target, and runs
/// before the engine starts so it may also prepare downstream consumers.
@available(macOS 26.0, *)
func makeMicrophoneCaptureSource<Element: Sendable>(
  recordFile: String?,
  targetFormat resolveTargetFormat: (AVAudioFormat) async throws -> AVAudioFormat,
  onAudioLevel: (@Sendable (AudioSignalLevel) -> Void)? = nil,
  transform: @escaping @Sendable (CapturedAudioBuffer) -> Element
) async throws -> AudioInputSource<Element> {
  guard await requestMicrophonePermission() else {
    throw HelperRuntimeError.microphonePermissionDenied
  }

  helperDebugLog("microphone-permission granted=true")
  let engine = AVAudioEngine()
  let input = engine.inputNode
  let inputFormat = input.outputFormat(forBus: 0)
  let targetFormat = try await resolveTargetFormat(inputFormat)
  let converter = audioConverter(from: inputFormat, to: targetFormat)
  let recordingSink = RecordingSink(
    sourceFormat: inputFormat,
    initialRecorder: try AudioRecorder(path: recordFile, sourceFormat: inputFormat)
  )
  let audioCounter = AudioDebugCounter(label: "mic")

  helperDebugLog("mic-input-format {\(audioFormatDescription(inputFormat))}")
  helperDebugLog("mic-target-format {\(audioFormatDescription(targetFormat))} converter=\(converter == nil ? "none" : "enabled")")

  let continuationBox = ContinuationBox<Element>()
  let inputSequence = AsyncThrowingStream<Element, Error> { continuation in
    continuationBox.store(continuation)
    input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { buffer, _ in
      do {
        audioCounter.record(buffer)
        onAudioLevel?(audioSignalLevel(buffer))
        recordingSink.write(buffer)
        let convertedBuffer = try convertBuffer(buffer, to: targetFormat, using: converter)
        continuation.yield(transform(CapturedAudioBuffer(buffer: convertedBuffer)))
      } catch {
        continuation.finish(throwing: error)
      }
    }
  }

  try engine.start()
  helperDebugLog("mic-engine-started")
  return AudioInputSource(
    sequence: inputSequence,
    recordingSink: recordingSink
  ) {
    helperDebugLog("mic-cleanup")
    engine.stop()
    input.removeTap(onBus: 0)
    continuationBox.finish()
  }
}

@available(macOS 26.0, *)
private func makeSpeakerInputSource(
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber],
  recordFile: String?,
  emitter: HelperEventEmitter
) async throws -> AudioInputSource<AnalyzerInput> {
  let speakerInput = try await SpeakerTapInput()
  let analyzerFormat = try await analyzerAudioFormat(
    compatibleWith: transcribers,
    naturalFormat: speakerInput.audioFormat
  )
  helperDebugLog("speaker-input-format {\(audioFormatDescription(speakerInput.audioFormat))}")
  helperDebugLog("speaker-analyzer-format {\(audioFormatDescription(analyzerFormat))}")
  try await analyzer.prepareToAnalyze(in: analyzerFormat)
  helperDebugLog("speaker-analyzer-prepared")
  let recordingSink = RecordingSink(
    sourceFormat: speakerInput.audioFormat,
    initialRecorder: try AudioRecorder(path: recordFile, sourceFormat: speakerInput.audioFormat)
  )
  let levelLimiter = AudioLevelEventLimiter(minimumInterval: audioLevelEventMinimumInterval)
  let sequence = try await speakerInput.makeInputSequence(
    analyzerFormat: analyzerFormat,
    recordingSink: recordingSink,
    onAudioLevel: { level in
      let timestamp = Date()
      guard levelLimiter.shouldEmit(at: timestamp) else {
        return
      }
      Task {
        await emitter.emitAudioLevel(audioLevelEvent(stream: .speaker, level: level, timestamp: timestamp))
      }
    }
  )

  return AudioInputSource(sequence: sequence, recordingSink: recordingSink) {
    helperDebugLog("speaker-cleanup")
    speakerInput.stop()
  }
}

final class ContinuationBox<Element: Sendable>: @unchecked Sendable {
  private let lock = NSLock()
  private var continuation: AsyncThrowingStream<Element, Error>.Continuation?

  func store(_ continuation: AsyncThrowingStream<Element, Error>.Continuation) {
    lock.lock()
    self.continuation = continuation
    lock.unlock()
  }

  func finish() {
    lock.lock()
    let current = continuation
    continuation = nil
    lock.unlock()
    current?.finish()
  }
}

struct AudioInputSource<Element: Sendable>: @unchecked Sendable {
  let sequence: AsyncThrowingStream<Element, Error>
  let recordingSink: RecordingSink
  let cleanup: @Sendable () -> Void

  init(
    sequence: AsyncThrowingStream<Element, Error>,
    recordingSink: RecordingSink,
    cleanup: @escaping @Sendable () -> Void
  ) {
    self.sequence = sequence
    self.recordingSink = recordingSink
    self.cleanup = cleanup
  }
}

// MARK: - Languages

@available(macOS 26.0, *)
private func ensureInstalledLanguages(
  _ languages: [String],
  transcribers: [SpeechTranscriber],
  stream: AudioStream,
  emitter: HelperEventEmitter
) async throws {
  let installedIdentifiers = await Set(
    SpeechTranscriber.installedLocales.map {
      $0.identifier(.bcp47)
    }
  )
  helperDebugLog(
    "installed-languages available=\(installedIdentifiers.sorted().joined(separator: ",")) requested=\(languages.joined(separator: ","))"
  )

  let missing = zip(languages, transcribers).filter { !installedIdentifiers.contains($0.0) }
  guard !missing.isEmpty else {
    return
  }

  let supportedIdentifiers = await Set(
    SpeechTranscriber.supportedLocales.map {
      $0.identifier(.bcp47)
    }
  )

  for (language, transcriber) in missing {
    guard supportedIdentifiers.contains(language) else {
      throw HelperRuntimeError.speechLanguageNotInstalled(language)
    }

    await emitter.emitStatus(StatusEvent(stream: stream, state: "downloading-language", lang: language))
    helperDebugLog("language-download-start language=\(language)")

    do {
      if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
        try await request.downloadAndInstall()
      }
    } catch {
      helperDebugLog("language-download-failed language=\(language) error=\(diagnosticDescription(for: error))")
      throw HelperRuntimeError.speechLanguageNotInstalled(language)
    }

    await emitter.emitStatus(StatusEvent(stream: stream, state: "language-ready", lang: language))
    helperDebugLog("language-download-done language=\(language)")
  }
}

@available(macOS 26.0, *)
private func analyzerAudioFormat(
  compatibleWith modules: [any SpeechModule],
  naturalFormat: AVAudioFormat
) async throws -> AVAudioFormat {
  guard
    let format = await SpeechAnalyzer.bestAvailableAudioFormat(
      compatibleWith: modules,
      considering: naturalFormat
    )
  else {
    throw HelperRuntimeError.missingAnalyzerAudioFormat
  }

  return format
}

func transcriptionLanguages(
  requestedLanguages: [String],
  sourceLanguage: String?,
  targetLanguage: String?
) -> [String] {
  var languages: [String] = []
  let candidates = requestedLanguages.isEmpty
    ? [sourceLanguage, targetLanguage].compactMap { $0 }
    : requestedLanguages

  for language in candidates where !language.isEmpty {
    if !languages.contains(language) {
      languages.append(language)
    }
  }

  return languages.isEmpty ? ["en-US"] : languages
}

func oppositeLanguage(for language: String, sourceLanguage: String?, targetLanguage: String?) -> String? {
  if language == sourceLanguage {
    return targetLanguage
  }

  if language == targetLanguage {
    return sourceLanguage
  }

  return targetLanguage ?? sourceLanguage
}
