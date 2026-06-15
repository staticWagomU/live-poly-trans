import Foundation

public struct TranscriptEvent: Codable, Equatable {
  public let type: String
  public let stream: String
  public let lang: String
  public let text: String
  public let trans: String?
  public let isFinal: Bool
  public let timestamp: String
}

public func transcriptEvent(
  stream: AudioStream,
  language: String,
  text: String,
  translation: String?,
  isFinal: Bool,
  timestamp: Date
) -> TranscriptEvent {
  TranscriptEvent(
    type: "transcript",
    stream: stream.rawValue,
    lang: language,
    text: text,
    trans: translation,
    isFinal: isFinal,
    timestamp: ISO8601DateFormatter().string(from: timestamp)
  )
}
