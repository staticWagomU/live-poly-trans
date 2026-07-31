import { describe, expect, it } from 'vitest';
import {
  emptyStreamSessions,
  formatRecordingTimer,
  isCurrentSessionEvent,
  recordingElapsedSeconds,
  shouldRestartStream,
  withStreamSession,
  withoutStreamSessions
} from './captureState';

describe('withStreamSession', () => {
  it('sets the session for one stream without touching the other', () => {
    const sessions = withStreamSession(emptyStreamSessions(), 'mic', 'mic-1');

    expect(sessions).toEqual({ mic: 'mic-1', speaker: null });
  });

  it('returns a new object instead of mutating the input', () => {
    const before = emptyStreamSessions();
    const after = withStreamSession(before, 'speaker', 'spk-1');

    expect(before.speaker).toBeNull();
    expect(after).not.toBe(before);
  });
});

describe('withoutStreamSessions', () => {
  it('clears only the listed streams', () => {
    const sessions = { mic: 'mic-1', speaker: 'spk-1' };

    expect(withoutStreamSessions(sessions, ['mic'])).toEqual({
      mic: null,
      speaker: 'spk-1'
    });
  });

  it('clears every listed stream at once', () => {
    const sessions = { mic: 'mic-1', speaker: 'spk-1' };

    expect(withoutStreamSessions(sessions, ['mic', 'speaker'])).toEqual({
      mic: null,
      speaker: null
    });
  });
});

describe('isCurrentSessionEvent', () => {
  const sessions = { mic: 'mic-1', speaker: null };

  it('accepts events from the stream currently running session', () => {
    expect(isCurrentSessionEvent(sessions, { stream: 'mic', sessionId: 'mic-1' })).toBe(true);
  });

  it('rejects events from a stale session of the same stream', () => {
    expect(isCurrentSessionEvent(sessions, { stream: 'mic', sessionId: 'mic-0' })).toBe(false);
  });

  it('rejects events for a stream with no session', () => {
    expect(isCurrentSessionEvent(sessions, { stream: 'speaker', sessionId: 'spk-1' })).toBe(
      false
    );
  });
});

describe('recordingElapsedSeconds', () => {
  it('reports whole seconds since the session started', () => {
    expect(recordingElapsedSeconds({ startedAtMs: 10_000 }, 73_400)).toBe(63);
  });

  it('never goes negative when clocks skew', () => {
    expect(recordingElapsedSeconds({ startedAtMs: 10_000 }, 9_000)).toBe(0);
  });
});

describe('formatRecordingTimer', () => {
  it('formats minutes and zero-padded seconds', () => {
    expect(formatRecordingTimer(0)).toBe('0:00');
    expect(formatRecordingTimer(63)).toBe('1:03');
    expect(formatRecordingTimer(600)).toBe('10:00');
  });

  it('adds an hours segment past one hour', () => {
    expect(formatRecordingTimer(3_600)).toBe('1:00:00');
    expect(formatRecordingTimer(3_723)).toBe('1:02:03');
  });
});

describe('shouldRestartStream', () => {
  const base = {
    generationAtExit: 3,
    currentGeneration: 3,
    captureMode: 'both' as const,
    stream: 'mic' as const,
    sessions: { mic: null, speaker: 'spk-1' }
  };

  it('restarts a stream that is wanted, unstarted, and from the current generation', () => {
    expect(shouldRestartStream(base)).toBe(true);
  });

  it('does not restart after the user stopped or restarted capture (generation bumped)', () => {
    expect(shouldRestartStream({ ...base, currentGeneration: 4 })).toBe(false);
  });

  it('does not restart a stream the current capture mode no longer includes', () => {
    expect(shouldRestartStream({ ...base, captureMode: 'speaker' })).toBe(false);
  });

  it('does not restart a stream that already got a new session in the meantime', () => {
    expect(
      shouldRestartStream({ ...base, sessions: { mic: 'mic-2', speaker: 'spk-1' } })
    ).toBe(false);
  });
});
