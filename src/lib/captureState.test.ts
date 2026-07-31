import { describe, expect, it } from 'vitest';
import {
  emptyStreamSessions,
  isCurrentSessionEvent,
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
