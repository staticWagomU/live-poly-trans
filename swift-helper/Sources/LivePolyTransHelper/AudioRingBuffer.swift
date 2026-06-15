import AudioRingBuffer
import Foundation

public final class AudioByteRingBuffer: @unchecked Sendable {
  private let rawBuffer: OpaquePointer

  public init(capacity: Int) {
    self.rawBuffer = LPTAudioRingBufferCreate(capacity)
  }

  deinit {
    LPTAudioRingBufferDestroy(rawBuffer)
  }

  @discardableResult
  public func write(_ pointer: UnsafeRawPointer, byteCount: Int) -> Int {
    LPTAudioRingBufferWrite(rawBuffer, pointer, byteCount)
  }

  public var availableBytes: Int {
    LPTAudioRingBufferAvailable(rawBuffer)
  }

  public var capacity: Int {
    LPTAudioRingBufferCapacity(rawBuffer)
  }

  public func clear() {
    LPTAudioRingBufferClear(rawBuffer)
  }
}
