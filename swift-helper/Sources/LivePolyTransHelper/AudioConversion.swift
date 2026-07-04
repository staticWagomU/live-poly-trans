import AVFAudio
import Foundation

public enum HelperRuntimeError: Error, CustomStringConvertible {
  case microphonePermissionDenied
  case unsupportedOperatingSystem
  case missingAnalyzerAudioFormat
  case missingConvertedAudioBuffer
  case audioConversionFailed(String)
  case speechLanguageNotInstalled(String)

  public var description: String {
    switch self {
    case .microphonePermissionDenied:
      "Microphone permission was denied."
    case .unsupportedOperatingSystem:
      "LivePolyTrans requires macOS 26 or later."
    case .missingAnalyzerAudioFormat:
      "SpeechAnalyzer did not provide a compatible audio format."
    case .missingConvertedAudioBuffer:
      "Could not allocate a converted audio buffer."
    case let .audioConversionFailed(message):
      "Audio conversion failed: \(message)"
    case let .speechLanguageNotInstalled(language):
      "Speech language is not installed and could not be downloaded: \(language)."
    }
  }
}

func audioConverter(from sourceFormat: AVAudioFormat, to targetFormat: AVAudioFormat) -> AVAudioConverter? {
  if sourceFormat == targetFormat {
    return nil
  }

  return AVAudioConverter(from: sourceFormat, to: targetFormat)
}

func convertBuffer(
  _ buffer: AVAudioPCMBuffer,
  to targetFormat: AVAudioFormat,
  using converter: AVAudioConverter?
) throws -> AVAudioPCMBuffer {
  guard let converter else {
    return buffer
  }

  let ratio = targetFormat.sampleRate / buffer.format.sampleRate
  let frameCapacity = AVAudioFrameCount(max(1, ceil(Double(buffer.frameLength) * ratio)))
  guard let converted = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: frameCapacity) else {
    throw HelperRuntimeError.missingConvertedAudioBuffer
  }

  let inputProvider = SingleUseAudioInput(buffer: buffer)
  var conversionError: NSError?
  let status = converter.convert(to: converted, error: &conversionError) { _, outStatus in
    inputProvider.next(outStatus: outStatus)
  }

  switch status {
  case .haveData, .inputRanDry, .endOfStream:
    return converted
  case .error:
    throw HelperRuntimeError.audioConversionFailed(conversionError?.localizedDescription ?? "unknown error")
  @unknown default:
    throw HelperRuntimeError.audioConversionFailed("unknown converter status")
  }
}

public final class SingleUseAudioInput: @unchecked Sendable {
  private let buffer: AVAudioPCMBuffer
  private let lock = NSLock()
  private var didProvideInput = false

  public init(buffer: AVAudioPCMBuffer) {
    self.buffer = buffer
  }

  public func next(outStatus: UnsafeMutablePointer<AVAudioConverterInputStatus>) -> AVAudioBuffer? {
    lock.lock()
    defer { lock.unlock() }

    if didProvideInput {
      outStatus.pointee = .noDataNow
      return nil
    }

    didProvideInput = true
    outStatus.pointee = .haveData
    return buffer
  }
}
