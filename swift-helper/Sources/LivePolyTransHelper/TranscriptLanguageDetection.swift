import Foundation
import NaturalLanguage

public struct TranscriptLanguageDetection: Codable, Equatable {
  public let language: String
  public let confidence: Double
}

public func detectedTranscriptLanguage(_ text: String) -> TranscriptLanguageDetection? {
  let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
  guard !trimmed.isEmpty else {
    return nil
  }

  let recognizer = NLLanguageRecognizer()
  recognizer.processString(trimmed)
  let hypotheses = recognizer.languageHypotheses(withMaximum: 1)
  guard let hypothesis = hypotheses.first else {
    return nil
  }

  return TranscriptLanguageDetection(
    language: hypothesis.key.rawValue,
    confidence: hypothesis.value
  )
}
