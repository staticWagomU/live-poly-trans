@preconcurrency import AVFAudio
import Foundation
import Speech
import CoreMedia

public enum HelperRuntimeError: Error, CustomStringConvertible {
  case microphonePermissionDenied
  case unsupportedOperatingSystem
  case missingAnalyzerAudioFormat
  case missingConvertedAudioBuffer
  case audioConversionFailed(String)
  case speechLanguageNotInstalled(String)

  public var description: String {
    switch self {
    case .microphonePermissionDenied:
      "Microphone permission was denied."
    case .unsupportedOperatingSystem:
      "LivePolyTrans requires macOS 26 or later."
    case .missingAnalyzerAudioFormat:
      "SpeechAnalyzer did not provide a compatible audio format."
    case .missingConvertedAudioBuffer:
      "Could not allocate a converted audio buffer."
    case let .audioConversionFailed(message):
      "Audio conversion failed: \(message)"
    case let .speechLanguageNotInstalled(language):
      "Speech language is not installed: \(language). Open Refresh Languages and select an installed language."
    }
  }
}

@available(macOS 14.0, *)
public func requestMicrophonePermission() async -> Bool {
  await withCheckedContinuation { continuation in
    AVAudioApplication.requestRecordPermission { granted in
      continuation.resume(returning: granted)
    }
  }
}

@available(macOS 26.0, *)
public func runMicrophoneTranscription(
  stream: AudioStream = .mic,
  sourceLanguage: String?,
  targetLanguage: String?,
  languages requestedLanguages: [String],
  segmentDirectory: String?
) async throws {
  let languages = transcriptionLanguages(
    requestedLanguages: requestedLanguages,
    sourceLanguage: sourceLanguage,
    targetLanguage: targetLanguage
  )
  helperDebugLog(
    "stream-start stream=\(stream.rawValue) source=\(sourceLanguage ?? "-") target=\(targetLanguage ?? "-") languages=\(languages.joined(separator: ","))"
  )
  let transcribers = languages.map {
    SpeechTranscriber(
      locale: Locale(identifier: $0),
      transcriptionOptions: [],
      reportingOptions: [.volatileResults, .fastResults],
      attributeOptions: []
    )
  }
  try await ensureInstalledLanguages(languages)
  helperDebugLog("installed-language-check-ok languages=\(languages.joined(separator: ","))")
  let analyzer = SpeechAnalyzer(modules: transcribers)
  let segmentWriter = SegmentWriter(directoryPath: segmentDirectory)
  let inputSource = try await makeInputSource(stream: stream, analyzer: analyzer, transcribers: transcribers)
  let resultTasks = zip(languages, transcribers).map { language, transcriber in
    Task {
      try await emitResults(
        stream: stream,
        language: language,
        oppositeLanguage: oppositeLanguage(for: language, sourceLanguage: sourceLanguage, targetLanguage: targetLanguage),
        transcriber: transcriber,
        segmentWriter: segmentWriter
      )
    }
  }

  do {
    helperDebugLog("speech-analyzer-start stream=\(stream.rawValue)")
    try await analyzer.start(inputSequence: inputSource.sequence)
    for task in resultTasks {
      try await task.value
    }
  } catch {
    helperDebugLog("stream-error stream=\(stream.rawValue) error=\(diagnosticDescription(for: error))")
    resultTasks.forEach { $0.cancel() }
    inputSource.cleanup()
    throw error
  }
}

@available(macOS 26.0, *)
private func makeInputSource(
  stream: AudioStream,
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber]
) async throws -> AudioInputSource {
  switch stream {
  case .mic:
    return try await makeMicrophoneInputSource(analyzer: analyzer, transcribers: transcribers)
  case .speaker:
    return try await makeSpeakerInputSource(analyzer: analyzer, transcribers: transcribers)
  }
}

@available(macOS 26.0, *)
private func makeMicrophoneInputSource(
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber]
) async throws -> AudioInputSource {
  guard await requestMicrophonePermission() else {
    throw HelperRuntimeError.microphonePermissionDenied
  }

  helperDebugLog("microphone-permission granted=true")
  let engine = AVAudioEngine()
  let input = engine.inputNode
  let inputFormat = input.outputFormat(forBus: 0)
  let analyzerFormat = try await analyzerAudioFormat(compatibleWith: transcribers, naturalFormat: inputFormat)
  let converter = audioConverter(from: inputFormat, to: analyzerFormat)
  let audioCounter = AudioDebugCounter(label: "mic")
  let convertedAudioCounter = AudioDebugCounter(label: "mic-converted")

  helperDebugLog("mic-input-format {\(audioFormatDescription(inputFormat))}")
  helperDebugLog("mic-analyzer-format {\(audioFormatDescription(analyzerFormat))} converter=\(converter == nil ? "none" : "enabled")")

  try await analyzer.prepareToAnalyze(in: analyzerFormat)
  helperDebugLog("mic-analyzer-prepared")

  let inputSequence = AsyncThrowingStream<AnalyzerInput, Error> { continuation in
    input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { buffer, _ in
      do {
        audioCounter.record(buffer)
        let analyzerBuffer = try convertBuffer(buffer, to: analyzerFormat, using: converter)
        convertedAudioCounter.record(analyzerBuffer)
        continuation.yield(AnalyzerInput(buffer: analyzerBuffer))
      } catch {
        continuation.finish(throwing: error)
      }
    }
  }

  try engine.start()
  helperDebugLog("mic-engine-started")
  return AudioInputSource(sequence: inputSequence) {
    helperDebugLog("mic-cleanup")
    engine.stop()
    input.removeTap(onBus: 0)
  }
}

@available(macOS 26.0, *)
private func makeSpeakerInputSource(
  analyzer: SpeechAnalyzer,
  transcribers: [SpeechTranscriber]
) async throws -> AudioInputSource {
  let speakerInput = try await SpeakerTapInput()
  let analyzerFormat = try await analyzerAudioFormat(
    compatibleWith: transcribers,
    naturalFormat: speakerInput.audioFormat
  )
  helperDebugLog("speaker-input-format {\(audioFormatDescription(speakerInput.audioFormat))}")
  helperDebugLog("speaker-analyzer-format {\(audioFormatDescription(analyzerFormat))}")
  try await analyzer.prepareToAnalyze(in: analyzerFormat)
  helperDebugLog("speaker-analyzer-prepared")
  let sequence = try await speakerInput.makeInputSequence(analyzerFormat: analyzerFormat)

  return AudioInputSource(sequence: sequence) {
    helperDebugLog("speaker-cleanup")
    speakerInput.stop()
  }
}

@available(macOS 26.0, *)
private func emitResults(
  stream: AudioStream,
  language: String,
  oppositeLanguage: String?,
  transcriber: SpeechTranscriber,
  segmentWriter: SegmentWriter?
) async throws {
  let translator = LiveTranslator(sourceLanguage: language, targetLanguage: oppositeLanguage)
  helperDebugLog("result-listener-start stream=\(stream.rawValue) language=\(language) target=\(oppositeLanguage ?? "-")")

  for try await result in transcriber.results {
    let text = String(result.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)
    let segmentId = segmentIdentifier(for: result.range)
    helperDebugLog(
      "speech-result stream=\(stream.rawValue) language=\(language) final=\(result.isFinal) segment=\(segmentId) chars=\(text.count) text=\"\(text)\""
    )

    guard isMeaningfulTranscript(text, isFinal: result.isFinal) else {
      helperDebugLog(
        "speech-result-dropped stream=\(stream.rawValue) language=\(language) final=\(result.isFinal) segment=\(segmentId) reason=too-short-or-punctuation text=\"\(text)\""
      )
      continue
    }

    let translation = await translator.translate(text)
    let timestamp = Date()
    let event = transcriptEvent(
      stream: stream,
      language: language,
      text: text,
      translation: translation,
      isFinal: result.isFinal,
      timestamp: timestamp,
      segmentId: segmentId
    )

    if result.isFinal {
      try? segmentWriter?.write(event, at: timestamp)
    }

    print(try jsonLine(for: event))
    fflush(stdout)
    helperDebugLog(
      "transcript-emitted stream=\(stream.rawValue) language=\(language) final=\(result.isFinal) segment=\(segmentId) translation=\(translation == nil ? "none" : "present")"
    )
  }
}

private func isMeaningfulTranscript(_ text: String, isFinal: Bool) -> Bool {
  let hasSpeechLikeContent = text.contains { character in
    character.isLetter || character.isNumber
  }

  guard hasSpeechLikeContent else {
    return false
  }

  let minimumLength = isFinal ? 2 : 3
  return text.count >= minimumLength
}

private func segmentIdentifier(for range: CMTimeRange) -> String {
  let start = Int64((range.start.seconds * 1000).rounded())
  let duration = Int64((range.duration.seconds * 1000).rounded())
  return "\(start)-\(duration)"
}

private struct AudioInputSource {
  let sequence: AsyncThrowingStream<AnalyzerInput, Error>
  let cleanup: () -> Void
}

@available(macOS 26.0, *)
private func ensureInstalledLanguages(_ languages: [String]) async throws {
  let installedIdentifiers = await Set(
    SpeechTranscriber.installedLocales.map {
      $0.identifier(.bcp47)
    }
  )
  helperDebugLog(
    "installed-languages available=\(installedIdentifiers.sorted().joined(separator: ",")) requested=\(languages.joined(separator: ","))"
  )

  for language in languages where !installedIdentifiers.contains(language) {
    throw HelperRuntimeError.speechLanguageNotInstalled(language)
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

func audioConverter(from sourceFormat: AVAudioFormat, to targetFormat: AVAudioFormat) -> AVAudioConverter? {
  if sourceFormat == targetFormat {
    return nil
  }

  return AVAudioConverter(from: sourceFormat, to: targetFormat)
}

func convertBuffer(
  _ buffer: AVAudioPCMBuffer,
  to targetFormat: AVAudioFormat,
  using converter: AVAudioConverter?
) throws -> AVAudioPCMBuffer {
  guard let converter else {
    return buffer
  }

  let ratio = targetFormat.sampleRate / buffer.format.sampleRate
  let frameCapacity = AVAudioFrameCount(max(1, ceil(Double(buffer.frameLength) * ratio)))
  guard let converted = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: frameCapacity) else {
    throw HelperRuntimeError.missingConvertedAudioBuffer
  }

  var didProvideInput = false
  var conversionError: NSError?
  let status = converter.convert(to: converted, error: &conversionError) { _, outStatus in
    if didProvideInput {
      outStatus.pointee = .noDataNow
      return nil
    }

    didProvideInput = true
    outStatus.pointee = .haveData
    return buffer
  }

  switch status {
  case .haveData, .inputRanDry, .endOfStream:
    return converted
  case .error:
    throw HelperRuntimeError.audioConversionFailed(conversionError?.localizedDescription ?? "unknown error")
  @unknown default:
    throw HelperRuntimeError.audioConversionFailed("unknown converter status")
  }
}

private func transcriptionLanguages(
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

private func oppositeLanguage(for language: String, sourceLanguage: String?, targetLanguage: String?) -> String? {
  if language == sourceLanguage {
    return targetLanguage
  }

  if language == targetLanguage {
    return sourceLanguage
  }

  return targetLanguage ?? sourceLanguage
}
