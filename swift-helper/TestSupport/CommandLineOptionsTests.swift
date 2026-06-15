import Foundation

@main
struct CommandLineOptionsTests {
  static func main() throws {
    try parsesDetectLanguagesCommand()
    try parsesMicStreamCommandWithLocales()
    try readsValueAfterFlag()
    try formatsLanguageInfoWithBcp47Identifier()
    try encodesJsonLine()
    try formatsTranscriptEvent()
  }

  static func parsesDetectLanguagesCommand() throws {
    let options = try CommandLineOptions.parse(["helper", "--detect-languages"])
    try expectEqual(options.command, .detectLanguages)
  }

  static func parsesMicStreamCommandWithLocales() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "mic",
      "--source-language",
      "en-US",
      "--target-language",
      "ja-JP"
    ])

    try expectEqual(options.command, .stream(.mic))
    try expectEqual(options.sourceLanguage, "en-US")
    try expectEqual(options.targetLanguage, "ja-JP")
  }

  static func expectEqual<T: Equatable>(_ actual: T, _ expected: T) throws {
    if actual != expected {
      throw TestFailure(message: "Expected \(expected), got \(actual)")
    }
  }

  static func readsValueAfterFlag() throws {
    try expectEqual(value(after: "--stream", in: ["helper", "--stream", "mic"]), "mic")
    try expectEqual(value(after: "--missing", in: ["helper", "--stream", "mic"]), nil)
  }

  static func formatsLanguageInfoWithBcp47Identifier() throws {
    let language = languageInfo(from: Locale(identifier: "ja_JP"))
    try expectEqual(language.id, "ja-JP")
    try expectEqual(language.label, "Japanese (Japan)")
  }

  static func encodesJsonLine() throws {
    let line = try jsonLine(for: LanguageInfo(id: "en-US", label: "English"))
    try expectEqual(line, #"{"id":"en-US","label":"English"}"#)
  }

  static func formatsTranscriptEvent() throws {
    let event = transcriptEvent(
      stream: .mic,
      language: "en-US",
      text: "hello",
      translation: nil,
      isFinal: true,
      timestamp: Date(timeIntervalSince1970: 0)
    )

    try expectEqual(event.type, "transcript")
    try expectEqual(event.stream, "mic")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
  }
}

struct TestFailure: Error, CustomStringConvertible {
  let message: String

  var description: String {
    message
  }
}
