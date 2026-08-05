import { entryLines } from './entryLines';
import { formatCueTimestamp } from './srt';
import { withResolvedTimings } from './timing';
import type { ExportOptions, TranscriptEntry } from './types';

export function formatVttTimestamp(ms: number): string {
  return formatCueTimestamp(ms, '.');
}

/// WebVTT document: `WEBVTT` header, then cues mirroring the SRT structure
/// except timestamps use `.` for milliseconds and cues carry no identifier
/// lines. No entries yields just the header; otherwise cues are separated by
/// blank lines with a single trailing newline.
export function toVtt(entries: TranscriptEntry[], opts: ExportOptions): string {
  const resolved = withResolvedTimings(entries);
  if (resolved.length === 0) {
    return 'WEBVTT\n';
  }

  const cues = resolved.map((entry) =>
    [
      `${formatVttTimestamp(entry.startMs)} --> ${formatVttTimestamp(entry.endMs)}`,
      ...entryLines(entry, opts)
    ].join('\n')
  );

  return `WEBVTT\n\n${cues.join('\n\n')}\n`;
}
