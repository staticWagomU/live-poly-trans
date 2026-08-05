import { entryLines } from './entryLines';
import { formatCueTimestamp, withResolvedTimings } from './timing';
import type { ExportOptions, TranscriptEntry } from './types';

export function formatSrtTimestamp(ms: number): string {
  return formatCueTimestamp(ms, ',');
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
