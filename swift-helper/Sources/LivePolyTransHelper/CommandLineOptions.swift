public enum HelperCommand: Equatable, Sendable {
  case detectLanguages
  case installLanguage(String)
  case uninstallLanguage(String)
  case aiServer
  case waveform(path: String, buckets: Int)
  case mix(inputs: [String], output: String)
  case trim(input: String, output: String, startMs: Int64, endMs: Int64)
  case checkPermissions
  case requestPermission(PermissionKind)
  case stream(AudioStream)
}

public enum AudioStream: String, Equatable, Sendable {
  case mic
  case speaker
}

public enum TranscriptionEngine: String, Equatable, Sendable {
  case builtin
  case whisper
}

public struct CommandLineOptions: Equatable, Sendable {
  public let command: HelperCommand
  public let sourceLanguage: String?
  public let targetLanguage: String?
  public let languages: [String]
  public let segmentDirectory: String?
  public let recordFile: String?
  public let transcriptFile: String?
  public let transcriptionEngine: TranscriptionEngine
  public let whisperModel: String?
  public let whisperCli: String?
  public let whisperEngine: String?

  public init(
    command: HelperCommand,
    sourceLanguage: String? = nil,
    targetLanguage: String? = nil,
    languages: [String] = [],
    segmentDirectory: String? = nil,
    recordFile: String? = nil,
    transcriptFile: String? = nil,
    transcriptionEngine: TranscriptionEngine = .builtin,
    whisperModel: String? = nil,
    whisperCli: String? = nil,
    whisperEngine: String? = nil
  ) {
    self.command = command
    self.sourceLanguage = sourceLanguage
    self.targetLanguage = targetLanguage
    self.languages = languages
    self.segmentDirectory = segmentDirectory
    self.recordFile = recordFile
    self.transcriptFile = transcriptFile
    self.transcriptionEngine = transcriptionEngine
    self.whisperModel = whisperModel
    self.whisperCli = whisperCli
    self.whisperEngine = whisperEngine
  }

  public static func parse(_ arguments: [String]) throws -> CommandLineOptions {
    if arguments.contains("--detect-languages") {
      return CommandLineOptions(command: .detectLanguages)
    }

    if let language = value(after: "--install-language", in: arguments) {
      return CommandLineOptions(command: .installLanguage(language))
    }

    if let language = value(after: "--uninstall-language", in: arguments) {
      return CommandLineOptions(command: .uninstallLanguage(language))
    }

    if arguments.contains("--ai-server") {
      return CommandLineOptions(command: .aiServer)
    }

    if let waveformPath = value(after: "--waveform", in: arguments) {
      let buckets = value(after: "--buckets", in: arguments).flatMap(Int.init) ?? defaultWaveformBuckets
      return CommandLineOptions(command: .waveform(path: waveformPath, buckets: buckets))
    }

    if arguments.contains("--trim") {
      guard
        let input = value(after: "--input", in: arguments),
        let output = value(after: "--output", in: arguments),
        let startMs = value(after: "--start-ms", in: arguments).flatMap(Int64.init),
        let endMs = value(after: "--end-ms", in: arguments).flatMap(Int64.init)
      else {
        throw CommandLineOptionsError.unsupportedArguments(arguments)
      }

      return CommandLineOptions(
        command: .trim(input: input, output: output, startMs: startMs, endMs: endMs)
      )
    }

    if arguments.contains("--check-permissions") {
      return CommandLineOptions(command: .checkPermissions)
    }

    if let kindName = value(after: "--request-permission", in: arguments) {
      guard let kind = PermissionKind(rawValue: kindName) else {
        throw CommandLineOptionsError.unknownPermissionKind(kindName)
      }

      return CommandLineOptions(command: .requestPermission(kind))
    }

    if arguments.contains("--mix") {
      let inputs = values(after: "--input", in: arguments)
      guard inputs.count == 2, let output = value(after: "--output", in: arguments) else {
        throw CommandLineOptionsError.unsupportedArguments(arguments)
      }

      return CommandLineOptions(command: .mix(inputs: inputs, output: output))
    }

    guard
      let streamName = value(after: "--stream", in: arguments),
      let stream = AudioStream(rawValue: streamName)
    else {
      throw CommandLineOptionsError.unsupportedArguments(arguments)
    }

    let engine = try parsedTranscriptionEngine(in: arguments)
    let whisperModel = value(after: "--whisper-model", in: arguments)
    let whisperCli = value(after: "--whisper-cli", in: arguments)
    let whisperEngine = value(after: "--whisper-engine", in: arguments)
    if engine == .whisper, whisperModel == nil || (whisperCli == nil && whisperEngine == nil) {
      throw CommandLineOptionsError.missingWhisperConfiguration
    }

    return CommandLineOptions(
      command: .stream(stream),
      sourceLanguage: nonEmptyValue(after: "--source-language", in: arguments),
      targetLanguage: nonEmptyValue(after: "--target-language", in: arguments),
      languages: values(after: "--language", in: arguments),
      segmentDirectory: value(after: "--segment-directory", in: arguments),
      recordFile: value(after: "--record-file", in: arguments),
      transcriptFile: value(after: "--transcript-file", in: arguments),
      transcriptionEngine: engine,
      whisperModel: whisperModel,
      whisperCli: whisperCli,
      whisperEngine: whisperEngine
    )
  }
}

private func parsedTranscriptionEngine(in arguments: [String]) throws -> TranscriptionEngine {
  guard let engineName = value(after: "--transcription-engine", in: arguments) else {
    return .builtin
  }

  guard let engine = TranscriptionEngine(rawValue: engineName) else {
    throw CommandLineOptionsError.unknownTranscriptionEngine(engineName)
  }

  return engine
}

public enum CommandLineOptionsError: Error, Equatable, Sendable {
  case unsupportedArguments([String])
  case unknownTranscriptionEngine(String)
  case unknownPermissionKind(String)
  case missingWhisperConfiguration
}

/// The app passes "" to mean "no language selected" (e.g. 翻訳しない);
/// an empty identifier must never masquerade as a real language downstream.
public func nonEmptyValue(after flag: String, in arguments: [String]) -> String? {
  value(after: flag, in: arguments).flatMap { $0.isEmpty ? nil : $0 }
}

public func value(after flag: String, in arguments: [String]) -> String? {
  guard let index = arguments.firstIndex(of: flag) else {
    return nil
  }

  let valueIndex = arguments.index(after: index)
  guard valueIndex < arguments.endIndex else {
    return nil
  }

  return arguments[valueIndex]
}

public func values(after flag: String, in arguments: [String]) -> [String] {
  arguments.indices.compactMap { index in
    guard arguments[index] == flag else {
      return nil
    }

    let valueIndex = arguments.index(after: index)
    guard valueIndex < arguments.endIndex else {
      return nil
    }

    return arguments[valueIndex]
  }
}
