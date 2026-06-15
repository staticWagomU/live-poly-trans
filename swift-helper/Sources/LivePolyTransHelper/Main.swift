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
      fputs("live-poly-trans-helper: \(error)\n", stderr)
      exit(1)
    }
  }

  @available(macOS 26.0, *)
  public static func run(options: CommandLineOptions) async throws {
    switch options.command {
    case .detectLanguages:
      let payload = await languageDetectionPayload(
        installed: SpeechTranscriber.installedLocales,
        supported: SpeechTranscriber.supportedLocales
      )
      print(try jsonLine(for: payload))
      fflush(stdout)
    case let .stream(stream):
      guard stream == .mic else {
        throw HelperRuntimeError.unsupportedStream(stream)
      }
      try await runMicrophoneTranscription(sourceLanguage: options.sourceLanguage)
    }
  }
}
