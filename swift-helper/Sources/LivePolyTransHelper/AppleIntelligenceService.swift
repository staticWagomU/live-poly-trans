import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

public enum AppleIntelligenceError: Error, CustomStringConvertible {
  case unavailable(String)

  public var description: String {
    switch self {
    case let .unavailable(reason):
      "Apple Intelligence is unavailable: \(reason)"
    }
  }
}

public struct MeetingTranscriptRecord: Sendable, Equatable {
  public let timestamp: Date
  public let speakerId: String
  public let speakerLabel: String
  public let text: String

  public init(timestamp: Date = Date(), speakerId: String, speakerLabel: String, text: String) {
    self.timestamp = timestamp
    self.speakerId = speakerId
    self.speakerLabel = speakerLabel
    self.text = text
  }
}

public actor AppleIntelligenceService {
  public static let shared = AppleIntelligenceService()

  public var records: [MeetingTranscriptRecord] = []

  public init() {}

  public func appendFinalTranscript(_ text: String, speakerId: String) {
    appendFinalTranscript(text, speakerId: speakerId, speakerLabel: aiSpeakerLabel(for: speakerId))
  }

  public func appendFinalTranscript(_ text: String, speakerId: String, speakerLabel: String) {
    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
      return
    }

    records.append(
      MeetingTranscriptRecord(speakerId: speakerId, speakerLabel: speakerLabel, text: trimmed)
    )
  }

  public func transcriptContext() -> String {
    records
      .map { "[\(iso8601Timestamp($0.timestamp))] \($0.speakerLabel): \($0.text)" }
      .joined(separator: "\n")
  }

  public func generateSummary(transcript: String) async throws -> String {
    try await meetingAiResponse(
      prompt: meetingSummaryPrompt(transcript: transcript),
      emptyTranscriptResponse: "まだ確定済みの文字起こしがありません。"
    )
  }

  public func suggestQuestions(transcript: String) async throws -> String {
    try await meetingAiResponse(
      prompt: suggestedQuestionsPrompt(transcript: transcript),
      emptyTranscriptResponse: "質問候補を作るための文字起こしがまだありません。"
    )
  }

  public func ask(_ question: String, transcript: String) async throws -> String {
    let trimmedQuestion = question.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmedQuestion.isEmpty else {
      return "質問を入力してください。"
    }

    return try await meetingAiResponse(
      prompt: meetingQuestionPrompt(question: trimmedQuestion, transcript: transcript),
      emptyTranscriptResponse: "回答するための文字起こしがまだありません。"
    )
  }
}

public func aiSpeakerLabel(for speakerId: String) -> String {
  switch speakerId {
  case "self", "mic":
    "Speaker A"
  default:
    "Speaker B"
  }
}

public func meetingAiResponse(prompt: String, emptyTranscriptResponse: String) async throws -> String {
  guard !meetingTranscriptIsEmpty(in: prompt) else {
    return emptyTranscriptResponse
  }

  return try await appleIntelligenceResponse(prompt: prompt)
}

public func meetingTranscriptIsEmpty(in prompt: String) -> Bool {
  guard let transcriptRange = prompt.range(of: "Transcript:") else {
    return prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  return prompt[transcriptRange.upperBound...].trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
}

public func appleIntelligenceResponse(prompt: String) async throws -> String {
  #if canImport(FoundationModels)
  if #available(macOS 26.0, *) {
    let model = SystemLanguageModel.default
    guard model.isAvailable else {
      throw AppleIntelligenceError.unavailable(String(describing: model.availability))
    }

    let session = LanguageModelSession(
      model: model,
      instructions: "You help with live meeting transcripts. Be concise, factual, and do not invent details."
    )
    let response = try await session.respond(to: prompt)
    return response.content.trimmingCharacters(in: .whitespacesAndNewlines)
  }
  #endif

  throw AppleIntelligenceError.unavailable("FoundationModels framework is not available.")
}

public func readStandardInputText() -> String {
  let data = FileHandle.standardInput.readDataToEndOfFile()
  return String(decoding: data, as: UTF8.self)
}

public func meetingSummaryPrompt(transcript: String) -> String {
  """
  You are helping summarize a live meeting transcript.
  Produce a concise Japanese meeting overview with:
  - one paragraph overview
  - decisions
  - action items
  - open questions

  Transcript:
  \(transcript)
  """
}

public func suggestedQuestionsPrompt(transcript: String) -> String {
  """
  会議の文字起こしを読んで、参加者が次に確認すると有用な質問を日本語で5件以内に提案してください。
  各質問は短く、会議内容に直接関係するものだけにしてください。

  Transcript:
  \(transcript)
  """
}

public func meetingQuestionPrompt(question: String, transcript: String) -> String {
  """
  Answer the user's question using only the meeting transcript.
  If the transcript does not contain enough evidence, say that the transcript does not confirm it.
  Answer in Japanese unless the question asks for another language.

  Question:
  \(question)

  Transcript:
  \(transcript)
  """
}
