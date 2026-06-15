import AVFAudio
import CoreAudio
import Darwin
import Foundation
import Speech

public enum SpeakerTapError: Error, CustomStringConvertible {
  case missingTap
  case missingAggregateDevice
  case missingAudioFormat
  case missingDefaultOutputDevice
  case audioHardwareCall(String, OSStatus)

  public var description: String {
    switch self {
    case .missingTap:
      "Core Audio did not create a process tap."
    case .missingAggregateDevice:
      "Core Audio did not create an aggregate tap device."
    case .missingAudioFormat:
      "Core Audio did not provide a readable tap audio format."
    case .missingDefaultOutputDevice:
      "Core Audio did not provide a default output device."
    case let .audioHardwareCall(name, status):
      "\(name) failed with OSStatus \(status)."
    }
  }
}

@available(macOS 26.0, *)
public final class SpeakerTapInput: @unchecked Sendable {
  private let tap: AudioHardwareTap
  private let aggregateDevice: AudioHardwareAggregateDevice
  private let format: AVAudioFormat
  private let ringBuffer = AudioByteRingBuffer(capacity: 32 * 1024 * 1024)
  private var ioProcID: AudioDeviceIOProcID?

  public init() throws {
    let outputDevice = try defaultOutputDevice()
    let outputDeviceUID = try outputDevice.uid
    let description = CATapDescription(excludingProcesses: [], deviceUID: outputDeviceUID, stream: 0)
    description.name = "LivePolyTrans Speaker Tap"
    description.isPrivate = true
    description.muteBehavior = CATapMuteBehavior.unmuted
    helperDebugLog(
      "speaker-tap-create name=\"\(description.name)\" private=\(description.isPrivate) outputDevice=\(outputDevice.id) outputUID=\(outputDeviceUID) stream=0"
    )

    guard let tap = try AudioHardwareSystem.shared.makeProcessTap(description: description) else {
      throw SpeakerTapError.missingTap
    }

    self.tap = tap
    let tapUID = try tap.uid
    helperDebugLog("speaker-tap-created uid=\(tapUID)")

    let aggregateDescription: [String: Any] = [
      kAudioAggregateDeviceNameKey: "LivePolyTrans Speaker Capture",
      kAudioAggregateDeviceUIDKey: "com.staticwagomu.live-poly-trans.speaker.\(UUID().uuidString)",
      kAudioAggregateDeviceIsPrivateKey: true,
      kAudioAggregateDeviceTapAutoStartKey: true,
      kAudioAggregateDeviceTapListKey: [
        [
          kAudioSubTapUIDKey: tapUID,
          kAudioSubTapDriftCompensationKey: true
        ]
      ]
    ]

    guard let aggregateDevice = try AudioHardwareSystem.shared.makeAggregateDevice(description: aggregateDescription) else {
      try AudioHardwareSystem.shared.destroyProcessTap(tap)
      throw SpeakerTapError.missingAggregateDevice
    }

    self.aggregateDevice = aggregateDevice
    helperDebugLog("speaker-aggregate-created id=\(aggregateDevice.id)")

    var streamDescription = try tap.format
    guard let format = AVAudioFormat(streamDescription: &streamDescription) else {
      try AudioHardwareSystem.shared.destroyAggregateDevice(aggregateDevice)
      try AudioHardwareSystem.shared.destroyProcessTap(tap)
      throw SpeakerTapError.missingAudioFormat
    }

    self.format = format
    helperDebugLog("speaker-tap-format {\(audioFormatDescription(format))}")
  }

  deinit {
    stop()
  }

  public func makeInputSequence(analyzerFormat: AVAudioFormat) throws -> AsyncThrowingStream<AnalyzerInput, Error> {
    AsyncThrowingStream { continuation in
      do {
        try start(analyzerFormat: analyzerFormat, continuation: continuation)
      } catch {
        continuation.finish(throwing: error)
      }

      continuation.onTermination = { [weak self] _ in
        self?.stop()
      }
    }
  }

  private func start(
    analyzerFormat: AVAudioFormat,
    continuation: AsyncThrowingStream<AnalyzerInput, Error>.Continuation
  ) throws {
    var localIOProcID: AudioDeviceIOProcID?
    let audioFormat = format
    let converter = audioConverter(from: audioFormat, to: analyzerFormat)
    let audioRingBuffer = ringBuffer
    let audioCounter = AudioDebugCounter(label: "speaker")
    let convertedAudioCounter = AudioDebugCounter(label: "speaker-converted")
    let callbackCounter = AudioCallbackDebugCounter(label: "speaker-callback", format: audioFormat)

    helperDebugLog("speaker-start aggregateDevice=\(aggregateDevice.id) converter=\(converter == nil ? "none" : "enabled")")

    let status = AudioDeviceCreateIOProcIDWithBlock(&localIOProcID, aggregateDevice.id, nil) {
      _, inputData, _, outputData, _ in
      callbackCounter.record(inputData: inputData, outputData: outputData)
      if let buffer = copyAudioBufferList(inputData, format: audioFormat, ringBuffer: audioRingBuffer) {
        do {
          audioCounter.record(buffer)
          let analyzerBuffer = try convertBuffer(buffer, to: analyzerFormat, using: converter)
          convertedAudioCounter.record(analyzerBuffer)
          continuation.yield(AnalyzerInput(buffer: analyzerBuffer))
        } catch {
          continuation.finish(throwing: error)
        }
      }
    }
    try throwIfAudioError(status, "AudioDeviceCreateIOProcIDWithBlock")

    guard let localIOProcID else {
      throw SpeakerTapError.audioHardwareCall("AudioDeviceCreateIOProcIDWithBlock", -1)
    }

    ioProcID = localIOProcID
    try throwIfAudioError(AudioDeviceStart(aggregateDevice.id, localIOProcID), "AudioDeviceStart")
    helperDebugLog("speaker-audio-device-started aggregateDevice=\(aggregateDevice.id)")
  }

  public func stop() {
    if let ioProcID {
      helperDebugLog("speaker-stop aggregateDevice=\(aggregateDevice.id)")
      _ = AudioDeviceStop(aggregateDevice.id, ioProcID)
      _ = AudioDeviceDestroyIOProcID(aggregateDevice.id, ioProcID)
      self.ioProcID = nil
    }

    try? AudioHardwareSystem.shared.destroyAggregateDevice(aggregateDevice)
    try? AudioHardwareSystem.shared.destroyProcessTap(tap)
  }

  public var audioFormat: AVAudioFormat {
    format
  }
}

private final class AudioCallbackDebugCounter: @unchecked Sendable {
  private let label: String
  private let format: AVAudioFormat
  private let lock = NSLock()
  private var count = 0

  init(label: String, format: AVAudioFormat) {
    self.label = label
    self.format = format
  }

  func record(inputData: UnsafePointer<AudioBufferList>, outputData: UnsafeMutablePointer<AudioBufferList>) {
    lock.lock()
    count += 1
    let currentCount = count
    lock.unlock()

    guard currentCount == 1 || currentCount % 100 == 0 else {
      return
    }

    helperDebugLog(
      "\(label) count=\(currentCount) input={\(audioLevelDescription(inputData, format: format))} output={\(audioLevelDescription(outputData, format: format))}"
    )
  }
}

@available(macOS 26.0, *)
private func defaultOutputDevice() throws -> AudioHardwareDevice {
  var address = AudioObjectPropertyAddress(
    mSelector: kAudioHardwarePropertyDefaultOutputDevice,
    mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain
  )
  var deviceID = AudioObjectID(kAudioObjectUnknown)
  var dataSize = UInt32(MemoryLayout<AudioObjectID>.size)

  let status = AudioObjectGetPropertyData(
    AudioObjectID(kAudioObjectSystemObject),
    &address,
    0,
    nil,
    &dataSize,
    &deviceID
  )
  try throwIfAudioError(status, "AudioObjectGetPropertyData(default output)")

  guard deviceID != kAudioObjectUnknown else {
    throw SpeakerTapError.missingDefaultOutputDevice
  }

  return AudioHardwareDevice(id: deviceID)
}

private func throwIfAudioError(_ status: OSStatus, _ name: String) throws {
  guard status == noErr else {
    throw SpeakerTapError.audioHardwareCall(name, status)
  }
}

private func copyAudioBufferList(
  _ inputData: UnsafePointer<AudioBufferList>,
  format: AVAudioFormat,
  ringBuffer: AudioByteRingBuffer
) -> AVAudioPCMBuffer? {
  let sourceBuffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: inputData))
  guard let firstBuffer = sourceBuffers.first else {
    return nil
  }

  let bytesPerFrame = format.streamDescription.pointee.mBytesPerFrame
  guard bytesPerFrame > 0 else {
    return nil
  }

  let frameLength = AVAudioFrameCount(firstBuffer.mDataByteSize / bytesPerFrame)
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
    ringBuffer.write(source, byteCount: Int(byteCount))
    destinationBuffers[index].mDataByteSize = byteCount
  }

  return copy
}
