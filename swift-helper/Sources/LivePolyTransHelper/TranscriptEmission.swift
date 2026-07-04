import Foundation

/// Shared final-transcript emission used by every transcription engine:
/// emits the final event, then fires the translation as a detached task so
/// its latency never delays the next recognition result.
@available(macOS 26.0, *)
public func emitFinalTranscript(
  _ candidate: TranscriptCandidate,
  stream: AudioStream,
  sourceLanguage: String?,
  targetLanguage: String?,
  translators: [String: LiveTranslator],
  emitter: HelperEventEmitter
) async {
  let timestamp = Date()
  let event = transcriptEvent(
    stream: stream,
    language: candidate.language,
    text: candidate.text,
    translation: nil,
    isFinal: true,
    timestamp: timestamp,
    segmentId: candidate.segmentId,
    confidence: candidate.confidence,
    spans: candidate.spans
  )
  await emitter.emitFinal(event, at: timestamp)
  helperDebugLog(
    "transcript-final stream=\(stream.rawValue) language=\(candidate.language) segment=\(candidate.segmentId) chars=\(candidate.text.count)"
  )

  guard
    let translationTarget = oppositeLanguage(
      for: candidate.language,
      sourceLanguage: sourceLanguage,
      targetLanguage: targetLanguage
    ),
    let translator = translators[candidate.language]
  else {
    return
  }

  Task {
    guard let translated = await translator.translate(candidate.text) else {
      return
    }

    await emitter.emitTranslation(
      translationEvent(
        stream: stream,
        segmentId: candidate.segmentId,
        language: candidate.language,
        targetLanguage: translationTarget,
        translation: translated
      )
    )
  }
}
