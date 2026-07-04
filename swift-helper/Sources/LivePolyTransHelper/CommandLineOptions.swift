public enum HelperCommand: Equatable, Sendable {
  case detectLanguages
  case installLanguage(String)
  case uninstallLanguage(String)
  case aiServer
  case waveform(path: String, buckets: Int)
  case mix(inputs: [String], output: String)
  case stream(AudioStream)
}

public enum AudioStream: String, Equatable, Sendable {
  case mic
  case speaker
}

public struct CommandLineOptions: Equatable, Sendable {
  public let command: HelperCommand
  public let sourceLanguage: String?
  public let targetLanguage: String?
  public let languages: [String]
  public let segmentDirectory: String?
  public let recordFile: String?
  public let transcriptFile: String?

  public init(
    command: HelperCommand,
    sourceLanguage: String? = nil,
    targetLanguage: String? = nil,
    languages: [String] = [],
    segmentDirectory: String? = nil,
    recordFile: String? = nil,
    transcriptFile: String? = nil
  ) {
    self.command = command
    self.sourceLanguage = sourceLanguage
    self.targetLanguage = targetLanguage
    self.languages = languages
    self.segmentDirectory = segmentDirectory
    self.recordFile = recordFile
    self.transcriptFile = transcriptFile
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

    return CommandLineOptions(
      command: .stream(stream),
      sourceLanguage: value(after: "--source-language", in: arguments),
      targetLanguage: value(after: "--target-language", in: arguments),
      languages: values(after: "--language", in: arguments),
      segmentDirectory: value(after: "--segment-directory", in: arguments),
      recordFile: value(after: "--record-file", in: arguments),
      transcriptFile: value(after: "--transcript-file", in: arguments)
    )
  }
}

public enum CommandLineOptionsError: Error, Equatable, Sendable {
  case unsupportedArguments([String])
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
