import CoreMedia
import Foundation
import Speech

@available(macOS 26.0, *)
public func transcriptAttributeOptions() -> Set<SpeechTranscriber.ResultAttributeOption> {
  [.transcriptionConfidence, .audioTimeRange]
}

@available(macOS 26.0, *)
public func transcriptSpans(from attributedText: AttributedString) -> [TranscriptSpan] {
  attributedText.runs.map { run in
    let text = String(attributedText[run.range].characters)
    let confidence = run[AttributeScopes.SpeechAttributes.ConfidenceAttribute.self]
    let timeRange = run[AttributeScopes.SpeechAttributes.TimeRangeAttribute.self]
    let startMs: Int64? = timeRange.flatMap { milliseconds(from: $0.start) }
    let endMs: Int64? = timeRange.flatMap { range in
      guard
        let startMs = milliseconds(from: range.start),
        let durationMs = milliseconds(from: range.duration)
      else {
        return nil
      }

      return startMs + durationMs
    }

    return TranscriptSpan(text: text, confidence: confidence, startMs: startMs, endMs: endMs)
  }
}

public func transcriptConfidence(spans: [TranscriptSpan]) -> Double? {
  var weightedConfidence = 0.0
  var totalWeight = 0

  for span in spans {
    guard let confidence = span.confidence else {
      continue
    }

    let weight = max(span.text.count, 1)
    weightedConfidence += confidence * Double(weight)
    totalWeight += weight
  }

  guard totalWeight > 0 else {
    return nil
  }

  return weightedConfidence / Double(totalWeight)
}

public func milliseconds(from time: CMTime) -> Int64? {
  let seconds = time.seconds
  guard seconds.isFinite else {
    return nil
  }

  return Int64((seconds * 1000).rounded())
}
