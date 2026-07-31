import type { CaptureMode } from '$lib/audioMode';

export type PermissionKind = 'microphone' | 'screen-recording';
export type PermissionState = 'granted' | 'denied' | 'notDetermined';

export type PermissionStatus = {
  microphone: PermissionState;
  screenRecording: PermissionState;
};

export const PERMISSION_KINDS: PermissionKind[] = ['microphone', 'screen-recording'];

const labels: Record<PermissionKind, string> = {
  microphone: 'マイク',
  'screen-recording': '画面収録とシステムオーディオ'
};

export function permissionLabel(kind: PermissionKind): string {
  return labels[kind];
}

export function permissionStateOf(status: PermissionStatus, kind: PermissionKind): PermissionState {
  return kind === 'microphone' ? status.microphone : status.screenRecording;
}

/// Speaker capture goes through ScreenCaptureKit, which is why system audio
/// costs a screen recording grant rather than a microphone one.
export function requiredPermissions(mode: CaptureMode): PermissionKind[] {
  switch (mode) {
    case 'mic':
      return ['microphone'];
    case 'speaker':
      return ['screen-recording'];
    case 'both':
      return ['microphone', 'screen-recording'];
  }
}

/// A null status means the probe itself failed. Capture then proceeds as
/// before and surfaces whatever real error it hits, rather than being blocked
/// by an inconclusive check.
export function missingPermissions(
  status: PermissionStatus | null,
  required: PermissionKind[]
): PermissionKind[] {
  if (!status) {
    return [];
  }

  return required.filter((kind) => permissionStateOf(status, kind) !== 'granted');
}

/// Once refused, only System Settings can undo it — macOS will not show the
/// dialog a second time, so asking again would silently do nothing.
export function permissionActionFor(state: PermissionState): 'request' | 'open-settings' | 'none' {
  switch (state) {
    case 'granted':
      return 'none';
    case 'denied':
      return 'open-settings';
    case 'notDetermined':
      return 'request';
  }
}

export function permissionStartupNotice(missing: PermissionKind[]): string | null {
  if (missing.length === 0) {
    return null;
  }

  const names = missing.map(permissionLabel).join('と');
  return `${names}の許可がまだありません。設定 > プライバシー から1つずつ許可してください。`;
}
