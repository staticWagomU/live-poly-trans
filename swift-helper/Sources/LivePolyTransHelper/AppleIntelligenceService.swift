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

// MARK: - AI server protocol (JSONL over stdin/stdout)

public struct AiChatTurn: Codable, Equatable, Sendable {
  public let question: String
  public let answer: String

  public init(question: String, answer: String) {
    self.question = question
    self.answer = answer
  }
}

public struct AiServerRequest: Codable, Equatable, Sendable {
  public let id: String
  public let command: String
  public let question: String?
  public let language: String?
  public let previousSummary: String?
  public let history: [AiChatTurn]?
  public let transcript: String

  public init(
    id: String,
    command: String,
    question: String? = nil,
    language: String? = nil,
    previousSummary: String? = nil,
    history: [AiChatTurn]? = nil,
    transcript: String
  ) {
    self.id = id
    self.command = command
    self.question = question
    self.language = language
    self.previousSummary = previousSummary
    self.history = history
    self.transcript = transcript
  }
}

public struct AiServerResponse: Codable, Equatable, Sendable {
  public let id: String
  public let ok: Bool
  public let response: String?
  public let error: String?

  public init(id: String, ok: Bool, response: String? = nil, error: String? = nil) {
    self.id = id
    self.ok = ok
    self.response = response
    self.error = error
  }
}

public func aiServerResponse(for request: AiServerRequest) async -> AiServerResponse {
  let transcript = request.transcript.trimmingCharacters(in: .whitespacesAndNewlines)

  if transcript.isEmpty {
    return AiServerResponse(
      id: request.id,
      ok: true,
      response: emptyTranscriptResponse(command: request.command, responseLanguage: request.language)
    )
  }

  let prompt: String
  switch request.command {
  case "summary":
    prompt = meetingSummaryPrompt(
      transcript: transcript,
      previousSummary: request.previousSummary,
      responseLanguage: request.language
    )
  case "ask":
    let question = request.question?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    guard !question.isEmpty else {
      return AiServerResponse(id: request.id, ok: false, error: "question is empty")
    }

    prompt = meetingQuestionPrompt(
      question: question,
      transcript: transcript,
      history: request.history ?? [],
      responseLanguage: request.language
    )
  case "suggest":
    prompt = suggestedQuestionsPrompt(transcript: transcript, responseLanguage: request.language)
  default:
    return AiServerResponse(id: request.id, ok: false, error: "unknown command: \(request.command)")
  }

  do {
    let response = try await appleIntelligenceResponse(prompt: prompt)
    return AiServerResponse(id: request.id, ok: true, response: response)
  } catch {
    return AiServerResponse(id: request.id, ok: false, error: String(describing: error))
  }
}

/// Long-lived request loop. Keeping the process (and the system language
/// model) warm avoids paying model startup latency on every summary refresh.
public func runAiServer() async {
  prewarmAppleIntelligence()

  while let line = readLine(strippingNewline: true) {
    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
      continue
    }

    let response: AiServerResponse
    do {
      let request = try JSONDecoder().decode(AiServerRequest.self, from: Data(trimmed.utf8))
      response = await aiServerResponse(for: request)
    } catch {
      response = AiServerResponse(id: "", ok: false, error: "invalid request JSON: \(error)")
    }

    if let encoded = try? jsonLine(for: response) {
      print(encoded)
      fflush(stdout)
    }
  }
}

public let appleIntelligenceInstructions =
  "You help with live meeting transcripts. Be concise, factual, and do not invent details."

public func prewarmAppleIntelligence() {
  #if canImport(FoundationModels)
  if #available(macOS 26.0, *) {
    let model = SystemLanguageModel.default
    guard model.isAvailable else {
      helperDebugLog("ai-server-prewarm-skipped availability=\(String(describing: model.availability))")
      return
    }

    let session = LanguageModelSession(model: model, instructions: appleIntelligenceInstructions)
    session.prewarm()
    helperDebugLog("ai-server-prewarmed")
  }
  #endif
}

public func appleIntelligenceResponse(prompt: String) async throws -> String {
  #if canImport(FoundationModels)
  if #available(macOS 26.0, *) {
    let model = SystemLanguageModel.default
    guard model.isAvailable else {
      throw AppleIntelligenceError.unavailable(String(describing: model.availability))
    }

    let session = LanguageModelSession(model: model, instructions: appleIntelligenceInstructions)
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

// MARK: - Prompts

public func meetingSummaryPrompt(
  transcript: String,
  previousSummary: String?,
  responseLanguage: String?
) -> String {
  let language = responseLanguageInstruction(responseLanguage)
  let languageGuard = responseLanguageGuard(responseLanguage)
  let sections = meetingSummarySections(responseLanguage: responseLanguage)
  let trimmedPrevious = previousSummary?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

  if trimmedPrevious.isEmpty {
    return """
    You are helping summarize a live meeting transcript.
    Write the entire response in \(language).
    \(languageGuard)
    Structure it with these sections:
    \(sections)

    Transcript:
    \(transcript)
    """
  }

  return """
  You maintain a running summary of a live meeting.
  Write the entire response in \(language).
  \(languageGuard)
  Update the existing summary below using the new transcript portion.
  Keep everything still true, merge in the new information, and keep these sections:
  \(sections)

  Existing summary:
  \(trimmedPrevious)

  New transcript portion:
  \(transcript)
  """
}

public func meetingQuestionPrompt(
  question: String,
  transcript: String,
  history: [AiChatTurn],
  responseLanguage: String?
) -> String {
  let language = responseLanguageInstruction(responseLanguage)
  let historySection = chatHistorySection(history)

  return """
  Answer the user's question using only the meeting transcript.
  If the transcript does not contain enough evidence, say that the transcript does not confirm it.
  Write the answer in \(language) unless the question asks for another language.
  \(historySection)
  Question:
  \(question)

  Transcript:
  \(transcript)
  """
}

public func chatHistorySection(_ history: [AiChatTurn]) -> String {
  guard !history.isEmpty else {
    return ""
  }

  let turns = history
    .map { "Q: \($0.question)\nA: \($0.answer)" }
    .joined(separator: "\n")

  return """

  Previous conversation for context:
  \(turns)

  """
}

public func suggestedQuestionsPrompt(transcript: String, responseLanguage: String?) -> String {
  let language = responseLanguageInstruction(responseLanguage)

  return """
  Read the meeting transcript and suggest at most 5 short questions, in \(language), \
  that participants would find useful to confirm next.
  Only include questions directly related to the meeting content.

  Transcript:
  \(transcript)
  """
}

public func meetingSummarySections(responseLanguage: String?) -> String {
  if responseLanguagePrimaryCode(responseLanguage) == "ja" {
    return """
    - 概要
    - 決定事項
    - アクション項目
    - 未解決の質問
    """
  }

  return """
  - Overview
  - Decisions
  - Action items
  - Open questions
  """
}

public func responseLanguageInstruction(_ responseLanguage: String?) -> String {
  let trimmed = responseLanguage?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
  guard !trimmed.isEmpty else {
    return "the user's main language"
  }

  return "\(summaryLanguageName(for: trimmed)) (\(trimmed))"
}

public func responseLanguageGuard(_ responseLanguage: String?) -> String {
  let primaryLanguage = responseLanguagePrimaryCode(responseLanguage)
  if primaryLanguage == "ja" {
    return "Do not write the summary in English."
  }

  return "Do not switch to another language unless quoting exact transcript text."
}

public func summaryLanguageName(for responseLanguage: String) -> String {
  switch responseLanguagePrimaryCode(responseLanguage) {
  case "ja":
    return "Japanese"
  case "en":
    return "English"
  case "fr":
    return "French"
  case "de":
    return "German"
  case "es":
    return "Spanish"
  case "it":
    return "Italian"
  case "pt":
    return "Portuguese"
  case "ko":
    return "Korean"
  case "zh":
    return "Chinese"
  default:
    return responseLanguage
  }
}

public func responseLanguagePrimaryCode(_ responseLanguage: String?) -> String {
  let trimmed = responseLanguage?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
  return trimmed
    .split(separator: "-", maxSplits: 1)
    .first?
    .lowercased() ?? ""
}

public func emptyTranscriptResponse(command: String, responseLanguage: String?) -> String {
  let isJapanese = responseLanguagePrimaryCode(responseLanguage) == "ja"

  switch command {
  case "ask":
    return isJapanese
      ? "回答するための文字起こしがまだありません。"
      : "There is no transcript to answer from yet."
  case "suggest":
    return isJapanese
      ? "質問候補を作るための文字起こしがまだありません。"
      : "There is no transcript to suggest questions from yet."
  default:
    return isJapanese
      ? "まだ確定済みの文字起こしがありません。"
      : "There is no finalized transcript yet."
  }
}
