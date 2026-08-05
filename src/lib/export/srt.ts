import { entryLines } from './entryLines';
import { withResolvedTimings } from './timing';
import type { ExportOptions, TranscriptEntry } from './types';

export function formatSrtTimestamp(ms: number): string {
  return formatCueTimestamp(ms, ',');
}

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

/// SubRip document: cues numbered from 1, `,`-separated millisecond
/// timestamps, one blank line between cues, and a single trailing newline.
/// No entries yields an empty string. Timing gaps are filled by
/// withResolvedTimings so every cue has a non-empty window.
export function toSrt(entries: TranscriptEntry[], opts: ExportOptions): string {
  const resolved = withResolvedTimings(entries);
  if (resolved.length === 0) {
    return '';
  }

  const cues = resolved.map((entry, index) =>
    [
      `${index + 1}`,
      `${formatSrtTimestamp(entry.startMs)} --> ${formatSrtTimestamp(entry.endMs)}`,
      ...entryLines(entry, opts)
    ].join('\n')
  );

  return `${cues.join('\n\n')}\n`;
}
