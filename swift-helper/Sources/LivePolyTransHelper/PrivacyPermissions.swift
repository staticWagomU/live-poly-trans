@preconcurrency import AVFAudio
import CoreGraphics
import Foundation

/// The two privacy permissions that gate capture. Speech recognition is not
/// listed: SpeechAnalyzer runs on-device and never asks for it.
public enum PermissionKind: String, Equatable, Sendable, CaseIterable {
  case microphone
  case screenRecording = "screen-recording"
}

public enum PermissionState: String, Equatable, Sendable, Encodable {
  case granted
  case denied
  case notDetermined
}

public struct PermissionStatusPayload: Equatable, Sendable, Encodable {
  public let microphone: PermissionState
  public let screenRecording: PermissionState

  public init(microphone: PermissionState, screenRecording: PermissionState) {
    self.microphone = microphone
    self.screenRecording = screenRecording
  }
}

@available(macOS 14.0, *)
public func permissionState(
  forRecordPermission permission: AVAudioApplication.recordPermission
) -> PermissionState {
  switch permission {
  case .granted: .granted
  case .denied: .denied
  default: .notDetermined
  }
}

/// CGPreflightScreenCaptureAccess answers a plain yes/no, so a "no" cannot
/// distinguish a first launch from a refusal. Reporting it as `.denied` would
/// send people to System Settings to fix something they were never asked
/// about, so an ungranted preflight stays `.notDetermined`.
public func permissionState(forScreenCaptureGranted granted: Bool) -> PermissionState {
  granted ? .granted : .notDetermined
}

@available(macOS 14.0, *)
public func requestMicrophonePermission() async -> Bool {
  await withCheckedContinuation { continuation in
    AVAudioApplication.requestRecordPermission { granted in
      continuation.resume(returning: granted)
    }
  }
}

public func requestScreenCapturePermission() -> Bool {
  CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess()
}

/// Reads both permissions without prompting. The app calls this at launch so
/// it can decide whether to start capture instead of firing system dialogs.
@available(macOS 14.0, *)
public func currentPermissionStatus() -> PermissionStatusPayload {
  PermissionStatusPayload(
    microphone: permissionState(forRecordPermission: AVAudioApplication.shared.recordPermission),
    screenRecording: permissionState(forScreenCaptureGranted: CGPreflightScreenCaptureAccess())
  )
}

/// Asks for exactly one permission. macOS lists an app under Privacy &
/// Security only once it has asked at least once, so this is also what turns
/// a later denial into a checkbox the user can simply tick.
@available(macOS 14.0, *)
public func requestPermission(_ kind: PermissionKind) async -> PermissionStatusPayload {
  switch kind {
  case .microphone:
    _ = await requestMicrophonePermission()
  case .screenRecording:
    _ = requestScreenCapturePermission()
  }

  return currentPermissionStatus()
}
