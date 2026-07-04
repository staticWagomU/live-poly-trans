import Foundation

/// One recognition result from a single-language transcriber, normalized for arbitration.
public struct TranscriptCandidate: Equatable, Sendable {
  public let language: String
  public let text: String
  public let isFinal: Bool
  public let startMs: Int64
  public let durationMs: Int64
  public let confidence: Double?
  public let detectedLanguage: String?
  public let detectedLanguageConfidence: Double?
  public let spans: [TranscriptSpan]

  public init(
    language: String,
    text: String,
    isFinal: Bool,
    startMs: Int64,
    durationMs: Int64,
    confidence: Double?,
    detectedLanguage: String?,
    detectedLanguageConfidence: Double?,
    spans: [TranscriptSpan] = []
  ) {
    self.language = language
    self.text = text
    self.isFinal = isFinal
    self.startMs = startMs
    self.durationMs = durationMs
    self.confidence = confidence
    self.detectedLanguage = detectedLanguage
    self.detectedLanguageConfidence = detectedLanguageConfidence
    self.spans = spans
  }

  public var segmentId: String {
    "\(startMs)-\(durationMs)"
  }
}

public let transcriptArbitrationHoldMilliseconds: Int64 = 1_200

private let confidenceDecisiveGap = 0.12
private let densityPenaltyThreshold = 24.0
private let lengthRatioThreshold = 1.8
private let pointOverlapToleranceMs: Int64 = 300
private let minimumOverlapRatio = 0.3

public func primaryLanguageCode(_ language: String) -> String {
  language.split(separator: "-", maxSplits: 1).first?.lowercased() ?? ""
}

/// The two language transcribers segment the same audio differently, so
/// candidates are paired by time-range overlap instead of start proximity.
public func candidateRangesOverlap(_ left: TranscriptCandidate, _ right: TranscriptCandidate) -> Bool {
  timeRangesOverlap(
    startA: left.startMs, durationA: left.durationMs,
    startB: right.startMs, durationB: right.durationMs
  )
}

public func timeRangesOverlap(
  startA: Int64, durationA: Int64,
  startB: Int64, durationB: Int64
) -> Bool {
  if abs(startA - startB) <= pointOverlapToleranceMs {
    return true
  }

  let overlap = min(startA + durationA, startB + durationB) - max(startA, startB)
  guard overlap > 0 else {
    return false
  }

  let shorterDuration = max(1, min(durationA, durationB))
  return Double(overlap) / Double(shorterDuration) >= minimumOverlapRatio
}

/// Positive result means `left` wins, negative means `right` wins.
/// Ties keep `left` so the earlier/current candidate is stable.
public func compareTranscriptCandidates(_ left: TranscriptCandidate, _ right: TranscriptCandidate) -> Double {
  let confidenceGap = (left.confidence ?? 0) - (right.confidence ?? 0)
  if abs(confidenceGap) >= confidenceDecisiveGap {
    return confidenceGap
  }

  let detectionGap = detectedLanguageAgreement(left) - detectedLanguageAgreement(right)
  if abs(detectionGap) >= 0.5 {
    return detectionGap
  }

  let fitnessGap = languageFitness(text: left.text, language: left.language)
    - languageFitness(text: right.text, language: right.language)
  if abs(fitnessGap) >= 0.2 {
    return fitnessGap
  }

  let densityGap = transcriptDensityPenalty(right) - transcriptDensityPenalty(left)
  if abs(densityGap) >= 0.2 {
    return densityGap
  }

  let leftLength = Double(normalizedTextLength(left.text))
  let rightLength = Double(normalizedTextLength(right.text))
  let longer = max(leftLength, rightLength)
  let shorter = max(1, min(leftLength, rightLength))
  if longer / shorter >= lengthRatioThreshold {
    return leftLength < rightLength ? 1 : -1
  }

  return 1
}

/// A wrong-language transcriber tends to hear phantom syllables, producing
/// implausibly many characters per second of audio.
public func transcriptDensityPenalty(_ candidate: TranscriptCandidate) -> Double {
  guard candidate.durationMs > 0 else {
    return 0
  }

  let charactersPerSecond =
    Double(normalizedTextLength(candidate.text)) / (Double(candidate.durationMs) / 1000)
  guard charactersPerSecond > densityPenaltyThreshold else {
    return 0
  }

  return min(1, (charactersPerSecond - densityPenaltyThreshold) / densityPenaltyThreshold)
}

public func detectedLanguageAgreement(_ candidate: TranscriptCandidate) -> Double {
  guard
    let detected = candidate.detectedLanguage.map(primaryLanguageCode),
    !detected.isEmpty,
    (candidate.detectedLanguageConfidence ?? 0) >= 0.6
  else {
    return 0
  }

  let expected = primaryLanguageCode(candidate.language)
  guard !expected.isEmpty else {
    return 0
  }

  return detected == expected ? 1 : -1
}

/// Ratio of the text's letters/digits that belong to the script the given
/// language is expected to use. 0.5 for scripts we do not classify.
public func languageFitness(text: String, language: String) -> Double {
  let primary = primaryLanguageCode(language)
  let significant = text.unicodeScalars.filter { scalar in
    scalar.properties.isAlphabetic || scalar.properties.numericType != nil
  }

  guard !significant.isEmpty else {
    return 0
  }

  if primary == "ja" {
    let matching = significant.filter(isJapaneseScalar).count
    return Double(matching) / Double(significant.count)
  }

  if latinScriptLanguages.contains(primary) {
    let matching = significant.filter(isLatinScalar).count
    return Double(matching) / Double(significant.count)
  }

  return 0.5
}

/// Collects every pending candidate transitively connected to `seed` by
/// time-range overlap, across both languages.
public func overlappingGroup(
  in candidates: [TranscriptCandidate],
  seed: TranscriptCandidate
) -> [TranscriptCandidate] {
  var group: [TranscriptCandidate] = [seed]
  var remaining = candidates.filter { $0 != seed }
  var changed = true

  while changed {
    changed = false
    for candidate in remaining where group.contains(where: { candidateRangesOverlap($0, candidate) }) {
      group.append(candidate)
      remaining.removeAll { $0 == candidate }
      changed = true
    }
  }

  return group
}

/// Joins same-language finals of one utterance group in time order into a
/// single candidate covering their union range.
public func combinedCandidate(_ finals: [TranscriptCandidate]) -> TranscriptCandidate? {
  guard let first = finals.first else {
    return nil
  }

  if finals.count == 1 {
    return first
  }

  let sorted = finals.sorted { $0.startMs < $1.startMs }
  let separator = primaryLanguageCode(first.language) == "ja" ? "" : " "
  let text = sorted.map(\.text).joined(separator: separator)
  let spans = sorted.flatMap(\.spans)
  let startMs = sorted.map(\.startMs).min() ?? first.startMs
  let endMs = sorted.map { $0.startMs + $0.durationMs }.max() ?? first.startMs
  let detection = detectedTranscriptLanguage(text)

  return TranscriptCandidate(
    language: first.language,
    text: text,
    isFinal: true,
    startMs: startMs,
    durationMs: max(0, endMs - startMs),
    confidence: transcriptConfidence(spans: spans) ?? first.confidence,
    detectedLanguage: detection?.language,
    detectedLanguageConfidence: detection?.confidence,
    spans: spans
  )
}

/// Reduces one utterance group to the winning single-language candidate.
public func arbitrateGroup(_ group: [TranscriptCandidate]) -> TranscriptCandidate? {
  var languagesInOrder: [String] = []
  for candidate in group where !languagesInOrder.contains(candidate.language) {
    languagesInOrder.append(candidate.language)
  }

  let combined = languagesInOrder.compactMap { language in
    combinedCandidate(group.filter { $0.language == language })
  }

  guard var winner = combined.first else {
    return nil
  }

  for challenger in combined.dropFirst() where compareTranscriptCandidates(challenger, winner) > 0 {
    winner = challenger
  }

  return winner
}

private let latinScriptLanguages: Set<String> = ["en", "fr", "de", "es", "it", "pt", "nl"]

private func isJapaneseScalar(_ scalar: Unicode.Scalar) -> Bool {
  switch scalar.value {
  case 0x3040...0x309F, // Hiragana
    0x30A0...0x30FF, // Katakana
    0x31F0...0x31FF, // Katakana phonetic extensions
    0x3400...0x4DBF, // CJK extension A
    0x4E00...0x9FFF: // CJK unified ideographs
    return true
  default:
    return false
  }
}

private func isLatinScalar(_ scalar: Unicode.Scalar) -> Bool {
  switch scalar.value {
  case 0x0041...0x005A, 0x0061...0x007A, 0x0030...0x0039,
    0x00C0...0x024F: // Latin-1 supplement + extended A/B
    return true
  default:
    return false
  }
}

private func normalizedTextLength(_ text: String) -> Int {
  text.trimmingCharacters(in: .whitespacesAndNewlines).count
}

/// Serializes results from the per-language transcribers into a single lane:
/// at most one interim stream, and exactly one final per utterance.
public actor TranscriptArbiter {
  public enum Output: Equatable, Sendable {
    case interim(TranscriptCandidate)
    case final(TranscriptCandidate)
  }

  private let languageCount: Int
  private let holdMilliseconds: Int64
  private let output: @Sendable (Output) async -> Void
  private var pendingFinals: [TranscriptCandidate] = []
  private var lastInterim: TranscriptCandidate?
  private var latestVolatiles: [String: TranscriptCandidate] = [:]
  private var flushedRanges: [(startMs: Int64, endMs: Int64)] = []

  public init(
    languageCount: Int,
    holdMilliseconds: Int64 = transcriptArbitrationHoldMilliseconds,
    output: @escaping @Sendable (Output) async -> Void
  ) {
    self.languageCount = max(1, languageCount)
    self.holdMilliseconds = holdMilliseconds
    self.output = output
  }

  public func receive(_ candidate: TranscriptCandidate) async {
    if candidate.isFinal {
      await receiveFinal(candidate)
    } else {
      await receiveVolatile(candidate)
    }
  }

  /// Flushes everything still pending; call after all result streams end.
  public func flushAll() async {
    while let seed = pendingFinals.first {
      await flushGroup(containing: seed)
    }
  }

  private func receiveVolatile(_ candidate: TranscriptCandidate) async {
    guard !coversFlushedUtterance(candidate) else {
      return
    }

    latestVolatiles[candidate.language] = candidate

    var winner = candidate
    for competitor in latestVolatiles.values
    where competitor.language != winner.language && candidateRangesOverlap(competitor, winner) {
      if compareTranscriptCandidates(competitor, winner) > 0 {
        winner = competitor
      }
    }

    if let lastInterim, lastInterim.language == winner.language, lastInterim.text == winner.text {
      return
    }

    lastInterim = winner
    await output(.interim(winner))
  }

  private func receiveFinal(_ candidate: TranscriptCandidate) async {
    latestVolatiles[candidate.language] = nil

    // The other language's transcriber already finalized this stretch of
    // audio and its group was flushed; a second bubble would duplicate it.
    guard !coversFlushedUtterance(candidate) else {
      return
    }

    pendingFinals.append(candidate)

    let hasCounterpart = pendingFinals.contains {
      $0.language != candidate.language && candidateRangesOverlap($0, candidate)
    }

    if languageCount <= 1 || hasCounterpart {
      await flushGroup(containing: candidate)
      return
    }

    Task { [holdMilliseconds] in
      try? await Task.sleep(nanoseconds: UInt64(holdMilliseconds) * 1_000_000)
      await self.flushIfStillPending(candidate)
    }
  }

  private func flushIfStillPending(_ candidate: TranscriptCandidate) async {
    guard pendingFinals.contains(candidate) else {
      return
    }

    await flushGroup(containing: candidate)
  }

  private func flushGroup(containing seed: TranscriptCandidate) async {
    let group = overlappingGroup(in: pendingFinals, seed: seed)
    pendingFinals.removeAll { group.contains($0) }
    rememberFlushedRange(of: group)

    for candidate in group {
      if let volatileCandidate = latestVolatiles[candidate.language],
        candidateRangesOverlap(volatileCandidate, candidate) {
        latestVolatiles[candidate.language] = nil
      }
    }

    guard let winner = arbitrateGroup(group) else {
      return
    }

    lastInterim = nil
    await output(.final(winner))
  }

  private func rememberFlushedRange(of group: [TranscriptCandidate]) {
    guard let startMs = group.map(\.startMs).min(),
      let endMs = group.map({ $0.startMs + $0.durationMs }).max()
    else {
      return
    }

    flushedRanges.append((startMs: startMs, endMs: endMs))
    if flushedRanges.count > flushedRangeHistoryLimit {
      flushedRanges.removeFirst(flushedRanges.count - flushedRangeHistoryLimit)
    }
  }

  private func coversFlushedUtterance(_ candidate: TranscriptCandidate) -> Bool {
    flushedRanges.contains { range in
      timeRangesOverlap(
        startA: candidate.startMs, durationA: candidate.durationMs,
        startB: range.startMs, durationB: max(0, range.endMs - range.startMs)
      )
    }
  }
}

private let flushedRangeHistoryLimit = 8
