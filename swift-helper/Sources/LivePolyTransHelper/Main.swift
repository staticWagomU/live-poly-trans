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
      try await printLanguageDetectionPayload()
    case let .installLanguage(language):
      try await installLanguageAsset(language)
      try await printLanguageDetectionPayload()
    case let .uninstallLanguage(language):
      try await uninstallLanguageAsset(language)
      try await printLanguageDetectionPayload()
    case .aiServer:
      await runAiServer()
    case let .waveform(path, buckets):
      let waveform = try computeWaveform(path: path, buckets: buckets)
      print(try jsonLine(for: waveform))
      fflush(stdout)
    case let .mix(inputs, output):
      try mixAudioFiles(inputs: inputs, outputPath: output)
      print(#"{"ok":true}"#)
      fflush(stdout)
    case let .stream(stream):
      switch options.transcriptionEngine {
      case .builtin:
        try await runMicrophoneTranscription(
          stream: stream,
          sourceLanguage: options.sourceLanguage,
          targetLanguage: options.targetLanguage,
          languages: options.languages,
          segmentDirectory: options.segmentDirectory,
          recordFile: options.recordFile,
          transcriptFile: options.transcriptFile
        )
      case .whisper:
        guard let modelPath = options.whisperModel, let cliPath = options.whisperCli else {
          throw CommandLineOptionsError.missingWhisperConfiguration
        }

        try await runWhisperTranscription(
          stream: stream,
          sourceLanguage: options.sourceLanguage,
          targetLanguage: options.targetLanguage,
          segmentDirectory: options.segmentDirectory,
          recordFile: options.recordFile,
          transcriptFile: options.transcriptFile,
          modelPath: modelPath,
          cliPath: cliPath
        )
      }
    }
  }

  @available(macOS 26.0, *)
  static func printLanguageDetectionPayload() async throws {
    let payload = await languageDetectionPayload(
      installed: SpeechTranscriber.installedLocales,
      supported: SpeechTranscriber.supportedLocales,
      reserved: AssetInventory.reservedLocales
    )
    helperDebugLog("detect-languages installed=\(payload.installed.count) supported=\(payload.supported.count) reserved=\(payload.reserved.count)")
    print(try jsonLine(for: payload))
    fflush(stdout)
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
