import type { TranscriptEntry } from './types';

export type ResolvedTranscriptEntry = TranscriptEntry & { startMs: number; endMs: number };

export const DEFAULT_CUE_DURATION_MS = 3000;

/// `HH:MM:SS<separator>mmm`, hours zero-padded to at least two digits and
/// growing beyond for very long sessions. SRT separates milliseconds with a
/// comma, WebVTT with a period.
export function formatCueTimestamp(ms: number, separator: ',' | '.'): string {
  const clamped = Math.max(0, Math.floor(ms));
  const millis = clamped % 1000;
  const totalSeconds = Math.floor(clamped / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const seconds = totalSeconds % 60;

  return (
    `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:` +
    `${String(seconds).padStart(2, '0')}${separator}${String(millis).padStart(3, '0')}`
  );
}

/// Guarantees every entry a usable [startMs, endMs) cue window while keeping
/// the given order. Resolution rules, in priority order:
///
/// startMs
///   1. an explicit startMs wins;
///   2. otherwise it is derived from the ISO `timestamp` delta against the
///      first entry that has an explicit startMs (the anchor);
///   3. with no anchor, deltas are taken against the first entry's timestamp
///      (the first entry becomes 0);
///   4. an unparseable timestamp falls back to a synthetic sequential cue at
///      `index * defaultDurationMs`.
///   Results are clamped to be monotonically non-decreasing and >= 0.
///
/// endMs
///   1. an explicit endMs wins;
///   2. otherwise `min(next entry's startMs, startMs + defaultDurationMs)`;
///   3. either way the end is floored to `startMs + 1` so a cue never has
///      zero or negative length.
export function withResolvedTimings(
  entries: TranscriptEntry[],
  opts: { defaultDurationMs?: number } = {}
): ResolvedTranscriptEntry[] {
  const defaultDurationMs = opts.defaultDurationMs ?? DEFAULT_CUE_DURATION_MS;

  const anchor = entries.find((entry) => entry.startMs !== undefined) ?? entries[0];
  const anchorStartMs = anchor?.startMs ?? 0;
  const anchorTimeMs = anchor !== undefined ? Date.parse(anchor.timestamp) : Number.NaN;

  const startTimes: number[] = [];
  entries.forEach((entry, index) => {
    let startMs: number;
    const timeMs = Date.parse(entry.timestamp);
    if (entry.startMs !== undefined) {
      startMs = entry.startMs;
    } else if (Number.isFinite(anchorTimeMs) && Number.isFinite(timeMs)) {
      startMs = anchorStartMs + (timeMs - anchorTimeMs);
    } else {
      startMs = index * defaultDurationMs;
    }

    const previous = index > 0 ? startTimes[index - 1] : 0;
    startTimes.push(Math.max(0, previous, startMs));
  });

  return entries.map((entry, index) => {
    const startMs = startTimes[index];
    let endMs = entry.endMs;
    if (endMs === undefined) {
      endMs = startMs + defaultDurationMs;
      if (index + 1 < startTimes.length) {
        endMs = Math.min(endMs, startTimes[index + 1]);
      }
    }

    return { ...entry, startMs, endMs: Math.max(endMs, startMs + 1) };
  });
}
