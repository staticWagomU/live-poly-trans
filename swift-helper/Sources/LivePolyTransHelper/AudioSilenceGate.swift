import AVFAudio
import Foundation

public final class AudioSilenceGate: @unchecked Sendable {
  public let silenceThresholdRMS: Double
  public let maxTrailingSilentFrames: Int
  public private(set) var trailingSilentFrames = 0
  public private(set) var hasEmittedAudio = false

  public init(silenceThresholdRMS: Double, maxTrailingSilentFrames: Int) {
    self.silenceThresholdRMS = silenceThresholdRMS
    self.maxTrailingSilentFrames = max(0, maxTrailingSilentFrames)
  }

  public func shouldEmit(_ buffer: AVAudioPCMBuffer) -> Bool {
    shouldEmit(level: audioSignalLevel(buffer), frameLength: Int(buffer.frameLength))
  }

  public func shouldEmit(level: AudioSignalLevel, frameLength: Int) -> Bool {
    let isSilent = level.sampleCount == 0 || level.rms <= silenceThresholdRMS
    if !isSilent {
      hasEmittedAudio = true
      trailingSilentFrames = 0
      return true
    }

    guard hasEmittedAudio else {
      return false
    }

    trailingSilentFrames += max(0, frameLength)
    return trailingSilentFrames <= maxTrailingSilentFrames
  }
}
