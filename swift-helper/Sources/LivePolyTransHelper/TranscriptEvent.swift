import Foundation

public struct TranscriptSpan: Codable, Equatable {
  public let text: String
  public let confidence: Double?
  public let startMs: Int64?
  public let endMs: Int64?

  public init(text: String, confidence: Double?, startMs: Int64?, endMs: Int64?) {
    self.text = text
    self.confidence = confidence
    self.startMs = startMs
    self.endMs = endMs
  }
}

public struct TranscriptEvent: Codable, Equatable {
  public let type: String
  public let stream: String
  public let speakerId: String
  public let speakerLabel: String
  public let lang: String
  public let text: String
  public let trans: String?
  public let isFinal: Bool
  public let time: String
  public let timestamp: String
  public let segmentId: String
  public let confidence: Double?
  public let detectedLang: String?
  public let detectedLangConfidence: Double?
  public let spans: [TranscriptSpan]
}

public func transcriptEvent(
  stream: AudioStream,
  language: String,
  text: String,
  translation: String?,
  isFinal: Bool,
  timestamp: Date,
  segmentId: String,
  confidence: Double? = nil,
  spans: [TranscriptSpan] = []
) -> TranscriptEvent {
  let formattedTimestamp = iso8601Timestamp(timestamp)
  let languageDetection = detectedTranscriptLanguage(text)
  return TranscriptEvent(
    type: "transcript",
    stream: stream.rawValue,
    speakerId: speakerIdentifier(for: stream),
    speakerLabel: speakerLabel(for: stream),
    lang: language,
    text: text,
    trans: translation,
    isFinal: isFinal,
    time: formattedTimestamp,
    timestamp: formattedTimestamp,
    segmentId: segmentId,
    confidence: confidence,
    detectedLang: languageDetection?.language,
    detectedLangConfidence: languageDetection?.confidence,
    spans: spans
  )
}

public func speakerIdentifier(for stream: AudioStream) -> String {
  switch stream {
  case .mic:
    "self"
  case .speaker:
    "system-audio"
  }
}

public func speakerLabel(for stream: AudioStream) -> String {
  switch stream {
  case .mic:
    "Speaker A"
  case .speaker:
    "Speaker B"
  }
}
