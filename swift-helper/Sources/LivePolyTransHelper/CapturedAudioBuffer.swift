import AVFAudio
import CoreMedia

/// One converted capture buffer, engine-agnostic: the builtin engine wraps it
/// into `AnalyzerInput`, the whisper engine consumes the samples directly.
/// `startTime` is only tracked where gated gaps must stay on the clock
/// (speaker capture); mic capture leaves it nil.
public struct CapturedAudioBuffer: @unchecked Sendable {
  public let buffer: AVAudioPCMBuffer
  public let startTime: CMTime?

  public init(buffer: AVAudioPCMBuffer, startTime: CMTime? = nil) {
    self.buffer = buffer
    self.startTime = startTime
  }
}
