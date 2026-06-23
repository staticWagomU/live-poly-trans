import Darwin
import Foundation
import Speech

@main
public struct LivePolyTransHelper {
  public static func main() async {
    do {
      let options = try CommandLineOptions.parse(CommandLine.arguments)

      if #available(macOS 26.0, *) {
        try await run(options: options)
      } else {
        throw HelperRuntimeError.unsupportedOperatingSystem
      }
    } catch {
      fputs("live-poly-trans-helper: \(diagnosticDescription(for: error))\n", stderr)
      exit(1)
    }
  }

  @available(macOS 26.0, *)
  public static func run(options: CommandLineOptions) async throws {
    helperDebugLog("command=\(options.command) source=\(options.sourceLanguage ?? "-") target=\(options.targetLanguage ?? "-") languages=\(options.languages.joined(separator: ",")) segmentDirectory=\(options.segmentDirectory ?? "-")")

    switch options.command {
    case .detectLanguages:
      let payload = await languageDetectionPayload(
        installed: SpeechTranscriber.installedLocales,
        supported: SpeechTranscriber.supportedLocales
      )
      helperDebugLog("detect-languages installed=\(payload.installed.count) supported=\(payload.supported.count)")
      print(try jsonLine(for: payload))
      fflush(stdout)
    case .aiGenerateSummary:
      let response = try await AppleIntelligenceService.shared.generateSummary(
        transcript: readStandardInputText(),
        responseLanguage: options.sourceLanguage
      )
      print(response)
      fflush(stdout)
    case .aiSuggestQuestions:
      let response = try await AppleIntelligenceService.shared.suggestQuestions(
        transcript: readStandardInputText()
      )
      print(response)
      fflush(stdout)
    case let .aiAsk(question):
      let response = try await AppleIntelligenceService.shared.ask(
        question,
        transcript: readStandardInputText()
      )
      print(response)
      fflush(stdout)
    case let .stream(stream):
      try await runMicrophoneTranscription(
        stream: stream,
        sourceLanguage: options.sourceLanguage,
        targetLanguage: options.targetLanguage,
        languages: options.languages,
        segmentDirectory: options.segmentDirectory
      )
    }
  }
}

func diagnosticDescription(for error: Error) -> String {
  let nsError = error as NSError
  var parts = [
    String(describing: error),
    "domain=\(nsError.domain)",
    "code=\(nsError.code)"
  ]

  if !nsError.localizedDescription.isEmpty {
    parts.append("description=\(nsError.localizedDescription)")
  }

  if !nsError.userInfo.isEmpty {
    let userInfo = nsError.userInfo
      .map { key, value in "\(key)=\(value)" }
      .sorted()
      .joined(separator: ", ")
    parts.append("userInfo={\(userInfo)}")
  }

  return parts.joined(separator: " | ")
}
