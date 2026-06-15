public enum HelperCommand: Equatable, Sendable {
  case detectLanguages
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

  public static func parse(_ arguments: [String]) throws -> CommandLineOptions {
    if arguments.contains("--detect-languages") {
      return CommandLineOptions(command: .detectLanguages, sourceLanguage: nil, targetLanguage: nil)
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
      targetLanguage: value(after: "--target-language", in: arguments)
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
