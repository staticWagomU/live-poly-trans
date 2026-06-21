import AVFAudio
import Foundation

public let helperDebugLogPrefix = "live-poly-trans-helper debug:"

public func helperDebugLog(_ message: @autoclosure () -> String) {
  let line = "\(helperDebugLogPrefix) \(iso8601Timestamp(Date())) \(message())\n"
  fputs(line, stderr)
  fflush(stderr)
}

public func audioFormatDescription(_ format: AVAudioFormat) -> String {
  let description = format.streamDescription.pointee
  return [
    "sampleRate=\(format.sampleRate)",
    "channels=\(format.channelCount)",
    "commonFormat=\(format.commonFormat.rawValue)",
    "interleaved=\(format.isInterleaved)",
    "bytesPerFrame=\(description.mBytesPerFrame)",
    "framesPerPacket=\(description.mFramesPerPacket)"
  ].joined(separator: " ")
}

public final class AudioDebugCounter: @unchecked Sendable {
  private let label: String
  private let lock = NSLock()
  private var count = 0

  public init(label: String) {
    self.label = label
  }

  public func record(_ buffer: AVAudioPCMBuffer) {
    lock.lock()
    count += 1
    let currentCount = count
    lock.unlock()

    guard currentCount == 1 || currentCount % 100 == 0 else {
      return
    }

    helperDebugLog(
      "\(label) audio-buffer count=\(currentCount) frames=\(buffer.frameLength) level={\(audioLevelDescription(buffer))} format={\(audioFormatDescription(buffer.format))}"
    )
  }
}

public func audioLevelDescription(_ buffer: AVAudioPCMBuffer) -> String {
  audioSignalLevel(buffer).description
}

public func audioLevelDescription(_ bufferList: UnsafePointer<AudioBufferList>, format: AVAudioFormat) -> String {
  audioSignalLevel(bufferList: bufferList, format: format).description
}

public func audioLevelDescription(_ bufferList: UnsafeMutablePointer<AudioBufferList>, format: AVAudioFormat) -> String {
  audioSignalLevel(bufferList: bufferList, format: format).description
}
