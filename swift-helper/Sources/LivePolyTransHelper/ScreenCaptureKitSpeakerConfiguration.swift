import AVFAudio
import Foundation
import ScreenCaptureKit

public let screenCaptureKitSpeakerSampleRate = 48_000
public let screenCaptureKitSpeakerChannelCount: UInt32 = 1
public let screenCapturePermissionRecoveryMessage =
  "Screen recording permission is required for speaker audio capture. Open System Settings > Privacy & Security > Screen & System Audio Recording, enable LivePolyTransHelper, then restart capture."

public enum SpeakerAudioConfigurationError: Error, CustomStringConvertible {
  case missingAudioFormat

  public var description: String {
    switch self {
    case .missingAudioFormat:
      "Could not create the ScreenCaptureKit speaker audio format."
    }
  }
}

@available(macOS 13.0, *)
public func screenCaptureKitSpeakerStreamConfiguration() -> SCStreamConfiguration {
  let configuration = SCStreamConfiguration()
  configuration.width = 2
  configuration.height = 2
  configuration.queueDepth = 3
  configuration.capturesAudio = true
  configuration.excludesCurrentProcessAudio = true
  configuration.sampleRate = screenCaptureKitSpeakerSampleRate
  configuration.channelCount = Int(screenCaptureKitSpeakerChannelCount)
  return configuration
}

public func screenCaptureKitSpeakerAudioFormat() throws -> AVAudioFormat {
  guard
    let format = AVAudioFormat(
      standardFormatWithSampleRate: Double(screenCaptureKitSpeakerSampleRate),
      channels: screenCaptureKitSpeakerChannelCount
    )
  else {
    throw SpeakerAudioConfigurationError.missingAudioFormat
  }

  return format
}
