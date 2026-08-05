import AVFAudio
import CoreGraphics
import CoreMedia
import Foundation
import ScreenCaptureKit
import Speech

public enum SpeakerTapError: Error, CustomStringConvertible {
  case missingDisplay
  case screenCapturePermissionDenied
  case missingAudioFormat
  case missingAudioBufferListSize
  case audioBufferList(OSStatus)
  case addStreamOutput(String)
  case screenCaptureKit(String)

  public var description: String {
    switch self {
    case .missingDisplay:
      "ScreenCaptureKit did not provide a display to bind the system audio stream."
    case .screenCapturePermissionDenied:
      screenCapturePermissionRecoveryMessage
    case .missingAudioFormat:
      "ScreenCaptureKit did not provide a readable audio format."
    case .missingAudioBufferListSize:
      "ScreenCaptureKit did not provide an audio buffer list size."
    case let .audioBufferList(status):
      "CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer failed with OSStatus \(status)."
    case let .addStreamOutput(message):
      "ScreenCaptureKit could not add an audio stream output: \(message)"
    case let .screenCaptureKit(message):
      "ScreenCaptureKit speaker capture failed: \(message)"
    }
  }
}

@available(macOS 13.0, *)
public final class SpeakerTapInput: @unchecked Sendable {
  private let stream: SCStream
  private let streamOutput = SpeakerStreamOutput()
  private let sampleQueue = DispatchQueue(label: "com.staticwagomu.live-poly-trans.speaker.screencapturekit")
  private let stateQueue = DispatchQueue(label: "com.staticwagomu.live-poly-trans.speaker.state")
  private let format: AVAudioFormat
  private var isCapturing = false
  private var isOutputAdded = false
  private var finishContinuation: (@Sendable () -> Void)?

  public init() async throws {
    let hasScreenCapturePermission = requestScreenCapturePermission()
    helperDebugLog("speaker-screen-capture-permission granted=\(hasScreenCapturePermission)")
    guard hasScreenCapturePermission else {
      throw SpeakerTapError.screenCapturePermissionDenied
    }

    let content = try await screenCaptureKitShareableContent()
    guard let display = content.displays.first else {
      throw SpeakerTapError.screenCapturePermissionDenied
    }

    let filter = SCContentFilter(display: display, excludingWindows: [])
    let configuration = screenCaptureKitSpeakerStreamConfiguration()
    self.stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
    self.format = try screenCaptureKitSpeakerAudioFormat()

    helperDebugLog(
      "speaker-screencapturekit-create display=\(display.displayID) size=\(display.width)x\(display.height) audioSampleRate=\(configuration.sampleRate) audioChannels=\(configuration.channelCount) excludesCurrentProcessAudio=\(configuration.excludesCurrentProcessAudio)"
    )
    helperDebugLog("speaker-screencapturekit-format {\(audioFormatDescription(format))}")
  }

  deinit {
    stop()
  }

  public func makeInputSequence(
    analyzerFormat: AVAudioFormat,
    recordingSink: RecordingSink? = nil,
    onAudioLevel: (@Sendable (AudioSignalLevel) -> Void)? = nil
  ) async throws -> AsyncThrowingStream<AnalyzerInput, Error> {
    // Trailing silence must reach the analyzer long enough to trigger
    // finalization of the pending segment; only prolonged silence is dropped
    // to save CPU. Dropped stretches stay on the capture clock, so the
    // analyzer sees an explicit gap instead of compressed time.
    let silenceGate = AudioSilenceGate(
      silenceThresholdRMS: 0.0001,
      maxTrailingSilentFrames: Int(format.sampleRate * speakerTrailingSilenceSeconds)
    )
    return try await makeCaptureSequence(
      targetFormat: analyzerFormat,
      recordingSink: recordingSink,
      onAudioLevel: onAudioLevel,
      shouldInclude: { silenceGate.shouldEmit($0) },
      transform: { AnalyzerInput(buffer: $0.buffer, bufferStartTime: $0.startTime) }
    )
  }

  /// Engine-agnostic capture: converts every non-gated buffer to
  /// `targetFormat` and yields whatever `transform` makes of it.
  /// `shouldInclude` decides on the raw (pre-conversion) buffer so skipped
  /// stretches also skip conversion; recording and the capture clock run
  /// before it so gated gaps stay on the file/clock timeline.
  public func makeCaptureSequence<Element: Sendable>(
    targetFormat: AVAudioFormat,
    recordingSink: RecordingSink? = nil,
    onAudioLevel: (@Sendable (AudioSignalLevel) -> Void)? = nil,
    shouldInclude: @escaping @Sendable (AVAudioPCMBuffer) -> Bool = { _ in true },
    transform: @escaping @Sendable (CapturedAudioBuffer) -> Element
  ) async throws -> AsyncThrowingStream<Element, Error> {
    stop()

    let audioFormat = format
    let converter = audioConverter(from: audioFormat, to: targetFormat)
    let audioCounter = AudioDebugCounter(label: "speaker")
    let convertedAudioCounter = AudioDebugCounter(label: "speaker-converted")
    let captureClock = AudioCaptureClock(sampleRate: audioFormat.sampleRate)
    let inputSequence = AsyncThrowingStream<Element, Error> { continuation in
      stateQueue.sync {
        self.finishContinuation = { continuation.finish() }
      }
      streamOutput.setHandler { sampleBuffer in
        do {
          guard let buffer = try screenCaptureKitAudioBuffer(
            from: sampleBuffer,
            format: audioFormat
          ) else {
            return
          }

          // Recording taps the raw capture BEFORE silence gating so the file
          // timeline matches transcript timestamps sample for sample.
          let bufferStartTime = captureClock.advance(by: buffer.frameLength)
          recordingSink?.write(buffer)
          onAudioLevel?(audioSignalLevel(buffer))

          guard shouldInclude(buffer) else {
            return
          }

          audioCounter.record(buffer)
          let convertedBuffer = try convertBuffer(buffer, to: targetFormat, using: converter)
          convertedAudioCounter.record(convertedBuffer)
          continuation.yield(
            transform(CapturedAudioBuffer(buffer: convertedBuffer, startTime: bufferStartTime))
          )
        } catch {
          continuation.finish(throwing: error)
        }
      }

      continuation.onTermination = { [weak self] _ in
        self?.stop()
      }
    }

    do {
      do {
        try stream.addStreamOutput(streamOutput, type: .audio, sampleHandlerQueue: sampleQueue)
      } catch {
        throw SpeakerTapError.addStreamOutput(error.localizedDescription)
      }
      stateQueue.sync {
        isOutputAdded = true
      }

      helperDebugLog("speaker-screencapturekit-start converter=\(converter == nil ? "none" : "enabled")")
      try await stream.startCapture()

      stateQueue.sync {
        isCapturing = true
      }

      helperDebugLog("speaker-screencapturekit-started")
      return inputSequence
    } catch {
      stop()
      throw error
    }
  }

  public func stop() {
    let state = stateQueue.sync {
      let wasCapturing = isCapturing
      let hadOutput = isOutputAdded
      isCapturing = false
      isOutputAdded = false
      return (wasCapturing, hadOutput)
    }

    let pendingFinish = stateQueue.sync { () -> (@Sendable () -> Void)? in
      let current = finishContinuation
      finishContinuation = nil
      return current
    }
    pendingFinish?()

    streamOutput.reset()

    if state.1 {
      do {
        try stream.removeStreamOutput(streamOutput, type: .audio)
      } catch {
        helperDebugLog("speaker-screencapturekit-remove-output-error error=\(error.localizedDescription)")
      }
    }

    guard state.0 else {
      return
    }

    helperDebugLog("speaker-screencapturekit-stop")
    stream.stopCapture { error in
      if let error {
        helperDebugLog("speaker-screencapturekit-stop-error error=\(error.localizedDescription)")
      }
    }
  }

  public var audioFormat: AVAudioFormat {
    format
  }
}

@available(macOS 13.0, *)
public func screenCaptureKitShareableContent() async throws -> SCShareableContent {
  let currentContent = try await SCShareableContent.current
  if !currentContent.displays.isEmpty {
    return currentContent
  }

  return try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
}

@available(macOS 13.0, *)
final class SpeakerStreamOutput: NSObject, SCStreamOutput, @unchecked Sendable {
  let lock = NSLock()
  var handler: ((CMSampleBuffer) -> Void)?

  func setHandler(_ handler: @escaping (CMSampleBuffer) -> Void) {
    lock.lock()
    self.handler = handler
    lock.unlock()
  }

  func reset() {
    lock.lock()
    handler = nil
    lock.unlock()
  }

  func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
    guard type == .audio else {
      return
    }

    lock.lock()
    let currentHandler = handler
    lock.unlock()
    currentHandler?(sampleBuffer)
  }
}

public let speakerTrailingSilenceSeconds = 2.5

/// Frame-accurate clock over everything captured (including gated silence);
/// feeding explicit buffer start times keeps analyzer segment timestamps
/// aligned with the recording file.
public final class AudioCaptureClock: @unchecked Sendable {
  private let sampleRate: Double
  private let lock = NSLock()
  private var frames: Int64 = 0

  public init(sampleRate: Double) {
    self.sampleRate = sampleRate
  }

  public func advance(by frameLength: AVAudioFrameCount) -> CMTime {
    lock.lock()
    defer { lock.unlock() }

    let start = frames
    frames += Int64(frameLength)
    return CMTime(value: start, timescale: CMTimeScale(sampleRate))
  }
}

func screenCaptureKitAudioBuffer(
  from sampleBuffer: CMSampleBuffer,
  format: AVAudioFormat
) throws -> AVAudioPCMBuffer? {
  try withScreenCaptureKitAudioBufferList(from: sampleBuffer) { audioBufferList in
    copyAudioBufferList(audioBufferList, format: format)
  }
}

func withScreenCaptureKitAudioBufferList<T>(
  from sampleBuffer: CMSampleBuffer,
  _ body: (UnsafePointer<AudioBufferList>) throws -> T
) throws -> T {
  var sizeNeeded = 0
  var blockBuffer: CMBlockBuffer?

  let sizeStatus = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
    sampleBuffer,
    bufferListSizeNeededOut: &sizeNeeded,
    bufferListOut: nil,
    bufferListSize: 0,
    blockBufferAllocator: kCFAllocatorDefault,
    blockBufferMemoryAllocator: kCFAllocatorDefault,
    flags: 0,
    blockBufferOut: &blockBuffer
  )
  guard sizeStatus == noErr else {
    throw SpeakerTapError.audioBufferList(sizeStatus)
  }
  guard sizeNeeded > 0 else {
    throw SpeakerTapError.missingAudioBufferListSize
  }

  let rawBufferList = UnsafeMutableRawPointer.allocate(
    byteCount: sizeNeeded,
    alignment: MemoryLayout<AudioBufferList>.alignment
  )
  defer {
    rawBufferList.deallocate()
  }

  let audioBufferList = rawBufferList.bindMemory(to: AudioBufferList.self, capacity: 1)
  let fillStatus = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
    sampleBuffer,
    bufferListSizeNeededOut: nil,
    bufferListOut: audioBufferList,
    bufferListSize: sizeNeeded,
    blockBufferAllocator: kCFAllocatorDefault,
    blockBufferMemoryAllocator: kCFAllocatorDefault,
    flags: 0,
    blockBufferOut: &blockBuffer
  )
  guard fillStatus == noErr else {
    throw SpeakerTapError.audioBufferList(fillStatus)
  }

  return try body(UnsafePointer(audioBufferList))
}

func screenCaptureKitAudioFormat(from sampleBuffer: CMSampleBuffer) throws -> AVAudioFormat {
  guard
    let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer),
    let streamDescription = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)
  else {
    throw SpeakerTapError.missingAudioFormat
  }

  var description = streamDescription.pointee
  guard let format = AVAudioFormat(streamDescription: &description) else {
    throw SpeakerTapError.missingAudioFormat
  }

  return format
}

func copyAudioBufferList(
  _ inputData: UnsafePointer<AudioBufferList>,
  format: AVAudioFormat
) -> AVAudioPCMBuffer? {
  let sourceBuffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: inputData))
  let frameLength = audioFrameLength(inputData, format: format)
  guard frameLength > 0 else {
    return nil
  }

  guard let copy = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameLength) else {
    return nil
  }

  copy.frameLength = frameLength
  let destinationBuffers = UnsafeMutableAudioBufferListPointer(copy.mutableAudioBufferList)

  for index in 0..<min(sourceBuffers.count, destinationBuffers.count) {
    guard
      let source = sourceBuffers[index].mData,
      let destination = destinationBuffers[index].mData
    else {
      continue
    }

    let byteCount = min(sourceBuffers[index].mDataByteSize, destinationBuffers[index].mDataByteSize)
    memcpy(destination, source, Int(byteCount))
    destinationBuffers[index].mDataByteSize = byteCount
  }

  return copy
}
