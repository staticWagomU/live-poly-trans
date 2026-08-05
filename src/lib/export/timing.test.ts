import { describe, expect, it } from 'vitest';
import { withResolvedTimings } from './timing';
import type { TranscriptEntry } from './types';

function makeEntry(overrides: Partial<TranscriptEntry> = {}): TranscriptEntry {
  return {
    timestamp: '2026-06-15T00:00:00Z',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'ja-JP',
    text: 'こんにちは',
    translation: null,
    ...overrides
  };
}

describe('withResolvedTimings', () => {
  it('returns an empty array for no entries', () => {
    expect(withResolvedTimings([])).toEqual([]);
  });

  it('keeps complete timings as-is', () => {
    const resolved = withResolvedTimings([
      makeEntry({ startMs: 1_000, endMs: 2_500 }),
      makeEntry({ startMs: 3_000, endMs: 4_000 })
    ]);

    expect(resolved.map((entry) => [entry.startMs, entry.endMs])).toEqual([
      [1_000, 2_500],
      [3_000, 4_000]
    ]);
  });

  it('caps a missing endMs at the next entry start', () => {
    const resolved = withResolvedTimings([
      makeEntry({ startMs: 1_000 }),
      makeEntry({ startMs: 2_000, endMs: 3_000 })
    ]);

    expect(resolved[0].endMs).toBe(2_000);
  });

  it('gives the last entry a missing endMs the default duration', () => {
    const resolved = withResolvedTimings([makeEntry({ startMs: 1_000 })]);

    expect(resolved[0].endMs).toBe(4_000);
  });

  it('honors a custom defaultDurationMs', () => {
    const resolved = withResolvedTimings([makeEntry({ startMs: 1_000 })], {
      defaultDurationMs: 500
    });

    expect(resolved[0].endMs).toBe(1_500);
  });

  it('keeps end strictly after start even when the next entry starts at the same time', () => {
    const resolved = withResolvedTimings([
      makeEntry({ startMs: 1_000 }),
      makeEntry({ startMs: 1_000, endMs: 2_000 })
    ]);

    expect(resolved[0].endMs).toBe(1_001);
  });

  it('derives missing starts from ISO timestamp deltas against the first timed entry', () => {
    const resolved = withResolvedTimings([
      makeEntry({ startMs: 5_000, timestamp: '2026-06-15T00:00:10Z' }),
      makeEntry({ timestamp: '2026-06-15T00:00:12.500Z' })
    ]);

    expect(resolved[1].startMs).toBe(7_500);
  });

  it('derives starts from timestamp deltas when no entry has timings', () => {
    const resolved = withResolvedTimings([
      makeEntry({ timestamp: '2026-06-15T00:00:10Z' }),
      makeEntry({ timestamp: '2026-06-15T00:00:14Z' })
    ]);

    expect(resolved.map((entry) => entry.startMs)).toEqual([0, 4_000]);
  });

  it('falls back to sequential synthetic cues when nothing carries timing', () => {
    const resolved = withResolvedTimings([
      makeEntry({ timestamp: 'not-a-date' }),
      makeEntry({ timestamp: 'still-not-a-date' }),
      makeEntry({ timestamp: '' })
    ]);

    expect(resolved.map((entry) => [entry.startMs, entry.endMs])).toEqual([
      [0, 3_000],
      [3_000, 6_000],
      [6_000, 9_000]
    ]);
  });

  it('keeps the given order and clamps starts to be monotonically non-decreasing', () => {
    const resolved = withResolvedTimings([
      makeEntry({ text: 'later', startMs: 5_000, endMs: 6_000 }),
      makeEntry({ text: 'earlier', startMs: 2_000, endMs: 3_000 })
    ]);

    expect(resolved.map((entry) => entry.text)).toEqual(['later', 'earlier']);
    expect(resolved[1].startMs).toBe(5_000);
    expect(resolved[1].endMs).toBe(5_001);
  });

  it('clamps a derived start that would rewind behind the previous entry', () => {
    const resolved = withResolvedTimings([
      makeEntry({ startMs: 1_000, timestamp: '2026-06-15T00:00:10Z' }),
      makeEntry({ timestamp: '2026-06-15T00:00:05Z' })
    ]);

    expect(resolved[1].startMs).toBe(1_000);
  });
});
