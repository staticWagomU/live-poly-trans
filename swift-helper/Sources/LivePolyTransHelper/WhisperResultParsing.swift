import Foundation

/// Subset of whisper-cli --output-json-full we consume. Unknown fields
/// (systeminfo, params, token ids/timestamps) are ignored by Codable.
public struct WhisperCliResult: Codable, Equatable, Sendable {
  public struct LanguageResult: Codable, Equatable, Sendable {
    public let language: String

    public init(language: String) {
      self.language = language
    }
  }

  public struct Offsets: Codable, Equatable, Sendable {
    public let from: Int64
    public let to: Int64

    public init(from: Int64, to: Int64) {
      self.from = from
      self.to = to
    }
  }

  public struct Token: Codable, Equatable, Sendable {
    public let text: String
    public let p: Double

    public init(text: String, p: Double) {
      self.text = text
      self.p = p
    }
  }

  public struct Segment: Codable, Equatable, Sendable {
    public let text: String
    public let offsets: Offsets
    public let tokens: [Token]?

    public init(text: String, offsets: Offsets, tokens: [Token]?) {
      self.text = text
      self.offsets = offsets
      self.tokens = tokens
    }
  }

  public let result: LanguageResult
  public let transcription: [Segment]

  public init(result: LanguageResult, transcription: [Segment]) {
    self.result = result
    self.transcription = transcription
  }
}

public func parsedWhisperResult(_ jsonData: Data) throws -> WhisperCliResult {
  try JSONDecoder().decode(WhisperCliResult.self, from: jsonData)
}

/// Mean token probability across all segments, excluding decoder
/// bookkeeping tokens like [_BEG_]; whisper's stand-in for the per-result
/// confidence the Apple transcribers report natively.
public func whisperConfidence(_ result: WhisperCliResult) -> Double? {
  let probabilities = result.transcription
    .flatMap { $0.tokens ?? [] }
    .filter { !isWhisperSpecialToken($0.text) }
    .map(\.p)

  guard !probabilities.isEmpty else {
    return nil
  }

  return probabilities.reduce(0, +) / Double(probabilities.count)
}

public func isWhisperSpecialToken(_ text: String) -> Bool {
  text.hasPrefix("[_") && text.hasSuffix("_]")
}

/// Whisper fills silence-only audio with annotations ([BLANK_AUDIO],
/// (music), ♪) or stock hallucinated phrases; strip them so downstream
/// meaningfulness checks see only actual speech.
public func sanitizedWhisperText(_ text: String) -> String {
  var sanitized = text
  for pattern in [#"\[[^\]]*\]"#, #"\([^)]*\)"#, #"（[^）]*）"#] {
    sanitized = sanitized.replacingOccurrences(of: pattern, with: "", options: .regularExpression)
  }
  sanitized = sanitized.replacingOccurrences(of: "♪", with: "")
  sanitized = sanitized.trimmingCharacters(in: .whitespacesAndNewlines)

  if knownWhisperHallucinations.contains(sanitized) {
    return ""
  }

  return sanitized
}

private let knownWhisperHallucinations: Set<String> = [
  "ご視聴ありがとうございました",
  "ご視聴ありがとうございました。",
  "おしまい",
  "Thank you for watching.",
  "Thanks for watching.",
]

/// Maps whisper's detected code ("ja", "en") onto the session language pair
/// so translation direction keeps working. Unmatched codes (noise chunks
/// misdetected as a third language) fall back to whichever session language
/// the text's script fits better, preferring the source on a tie.
public func sessionLanguage(
  forWhisperLanguage code: String,
  sourceLanguage: String?,
  targetLanguage: String?,
  text: String
) -> String {
  let sessionLanguages = [sourceLanguage, targetLanguage]
    .compactMap { $0 }
    .filter { !$0.isEmpty }

  let normalizedCode = code.lowercased()
  if let match = sessionLanguages.first(where: { primaryLanguageCode($0) == normalizedCode }) {
    return match
  }

  var best: (language: String, fitness: Double)?
  for language in sessionLanguages {
    let fitness = languageFitness(text: text, language: language)
    if best == nil || fitness > best!.fitness {
      best = (language, fitness)
    }
  }

  return best?.language ?? code
}

public func whisperCliArguments(
  modelPath: String,
  audioPath: String,
  outputBase: String
) -> [String] {
  [
    "-m", modelPath,
    "-f", audioPath,
    "-l", "auto",
    "--output-json-full",
    "-of", outputBase,
    "--no-prints",
  ]
}

/// Reduces one whisper chunk result to a final TranscriptCandidate, or nil
/// when the chunk carried no meaningful speech. Timing comes from the chunk
/// (stream clock), not the whisper-internal offsets, so segment ids stay
/// monotonic across the session.
public func whisperTranscriptCandidate(
  result: WhisperCliResult,
  chunkStartMs: Int64,
  chunkDurationMs: Int64,
  sourceLanguage: String?,
  targetLanguage: String?
) -> TranscriptCandidate? {
  let text = sanitizedWhisperText(result.transcription.map(\.text).joined())
  guard isMeaningfulTranscript(text, isFinal: true) else {
    return nil
  }

  return TranscriptCandidate(
    language: sessionLanguage(
      forWhisperLanguage: result.result.language,
      sourceLanguage: sourceLanguage,
      targetLanguage: targetLanguage,
      text: text
    ),
    text: text,
    isFinal: true,
    startMs: chunkStartMs,
    durationMs: chunkDurationMs,
    confidence: whisperConfidence(result),
    detectedLanguage: result.result.language,
    detectedLanguageConfidence: nil
  )
}

public func isMeaningfulTranscript(_ text: String, isFinal: Bool) -> Bool {
  let hasSpeechLikeContent = text.contains { character in
    character.isLetter || character.isNumber
  }

  guard hasSpeechLikeContent else {
    return false
  }

  let minimumLength = isFinal ? 2 : 3
  return text.count >= minimumLength
}
