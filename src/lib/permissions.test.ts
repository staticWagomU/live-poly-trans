import { describe, expect, it } from 'vitest';
import {
  missingPermissions,
  permissionActionFor,
  permissionLabel,
  permissionStartupNotice,
  requiredPermissions,
  type PermissionStatus
} from './permissions';

const allGranted: PermissionStatus = { microphone: 'granted', screenRecording: 'granted' };

describe('requiredPermissions', () => {
  it('asks only for what the capture mode actually uses', () => {
    expect(requiredPermissions('mic')).toEqual(['microphone']);
    expect(requiredPermissions('speaker')).toEqual(['screen-recording']);
    expect(requiredPermissions('both')).toEqual(['microphone', 'screen-recording']);
  });
});

describe('missingPermissions', () => {
  it('reports the required permissions that are not granted yet', () => {
    expect(missingPermissions(allGranted, ['microphone', 'screen-recording'])).toEqual([]);
    expect(
      missingPermissions({ microphone: 'denied', screenRecording: 'granted' }, ['microphone'])
    ).toEqual(['microphone']);
    expect(
      missingPermissions({ microphone: 'granted', screenRecording: 'notDetermined' }, [
        'microphone',
        'screen-recording'
      ])
    ).toEqual(['screen-recording']);
  });

  it('ignores a permission the current mode does not need', () => {
    expect(
      missingPermissions({ microphone: 'granted', screenRecording: 'denied' }, ['microphone'])
    ).toEqual([]);
  });

  it('treats an unreadable status as nothing missing so capture is never blocked by a probe failure', () => {
    expect(missingPermissions(null, ['microphone', 'screen-recording'])).toEqual([]);
  });
});

describe('permissionActionFor', () => {
  it('asks in-app while undecided and hands off to System Settings after a refusal', () => {
    expect(permissionActionFor('notDetermined')).toBe('request');
    expect(permissionActionFor('denied')).toBe('open-settings');
    expect(permissionActionFor('granted')).toBe('none');
  });
});

describe('permissionLabel', () => {
  it('names each permission the way System Settings does', () => {
    expect(permissionLabel('microphone')).toBe('マイク');
    expect(permissionLabel('screen-recording')).toBe('画面収録とシステムオーディオ');
  });
});

describe('permissionStartupNotice', () => {
  it('stays silent when nothing is missing', () => {
    expect(permissionStartupNotice([])).toBeNull();
  });

  it('names the missing permissions instead of showing a raw helper error', () => {
    expect(permissionStartupNotice(['microphone'])).toContain('マイク');
    const both = permissionStartupNotice(['microphone', 'screen-recording']);
    expect(both).toContain('マイク');
    expect(both).toContain('画面収録とシステムオーディオ');
  });
});
