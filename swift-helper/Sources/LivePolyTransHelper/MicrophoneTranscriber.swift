import AVFAudio
import Foundation
import Speech

public enum HelperRuntimeError: Error, CustomStringConvertible {
  case microphonePermissionDenied
  case unsupportedStream(AudioStream)
  case unsupportedOperatingSystem

  public var description: String {
    switch self {
    case .microphonePermissionDenied:
      "Microphone permission was denied."
    case let .unsupportedStream(stream):
      "The \(stream.rawValue) stream is not implemented in this MVP."
    case .unsupportedOperatingSystem:
      "LivePolyTrans requires macOS 26 or later."
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
public func runMicrophoneTranscription(sourceLanguage: String?) async throws {
  guard await requestMicrophonePermission() else {
    throw HelperRuntimeError.microphonePermissionDenied
  }

  let language = sourceLanguage ?? "en-US"
  let locale = Locale(identifier: language)
  let transcriber = SpeechTranscriber(locale: locale, preset: .progressiveTranscription)
  let analyzer = SpeechAnalyzer(modules: [transcriber])
  let engine = AVAudioEngine()
  let input = engine.inputNode
  let inputFormat = input.outputFormat(forBus: 0)

  try await analyzer.prepareToAnalyze(in: inputFormat)

  let inputSequence = AsyncThrowingStream<AnalyzerInput, Error> { continuation in
    input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { buffer, _ in
      continuation.yield(AnalyzerInput(buffer: buffer))
    }
  }

  let resultsTask = Task {
    for try await result in transcriber.results {
      let event = transcriptEvent(
        stream: .mic,
        language: language,
        text: String(result.text.characters),
        translation: nil,
        isFinal: result.isFinal,
        timestamp: Date()
      )
      print(try jsonLine(for: event))
      fflush(stdout)
    }
  }

  do {
    try engine.start()
    try await analyzer.start(inputSequence: inputSequence)
    try await resultsTask.value
  } catch {
    resultsTask.cancel()
    engine.stop()
    input.removeTap(onBus: 0)
    throw error
  }
}
