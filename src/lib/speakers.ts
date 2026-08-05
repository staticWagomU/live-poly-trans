/// Shared speaker identity helpers: the one place that knows how speakers
/// are identified (`speaker-N` for diarized speakers, the stream name
/// otherwise), labeled, and colored. The ID scheme here is the key system
/// for the `speakers` map in meta.json.
import type { RecordingSpeaker, RecordingTranscriptItem } from './recordings';

/// Distinct dot colors for diarized speakers (話者1, 話者2, ...).
export const SPEAKER_PALETTE = ['#0066cc', '#ff9500', '#34c759', '#af52de', '#ff2d55', '#5ac8fa'];

export function speakerColor(index: number): string {
  return SPEAKER_PALETTE[index % SPEAKER_PALETTE.length];
}

/// Stable speaker id for a transcript item: diarized speakers get
/// `speaker-<index>`, everything else falls back to the capture stream
/// name ('mic' / 'speaker' / 'unknown').
export function transcriptItemSpeakerId(item: { speakerIndex?: number; stream: string }): string {
  return item.speakerIndex !== undefined ? `speaker-${item.speakerIndex}` : item.stream;
}

/// Default display label when a transcript event carries no explicit
/// speaker label: the mic is "Speaker A", everything else "Speaker B".
export function defaultStreamSpeakerLabel(stream: string): string {
  return stream === 'mic' ? 'Speaker A' : 'Speaker B';
}

/// Display label for a diarized (0-based) speaker index: 話者1, 話者2, ...
export function diarizedSpeakerLabel(index: number): string {
  return `話者${index + 1}`;
}

/// Resolves an entry's display name against the recording's `speakers` map
/// (meta.json): the custom name wins when present and non-blank, otherwise
/// the entry keeps its original label. Pure — the transcript itself is
/// never rewritten.
export function resolveSpeakerName(
  entry: { speakerId?: string; speakerLabel: string },
  speakers: Record<string, RecordingSpeaker> | null | undefined
): string {
  const name = entry.speakerId !== undefined ? speakers?.[entry.speakerId]?.name : undefined;
  return name !== undefined && name.trim() !== '' ? name : entry.speakerLabel;
}

/// Dot color for an entry: a custom color from the `speakers` map wins,
/// diarized entries fall back to the shared palette, and live-stream
/// entries return undefined so callers keep their stream-based styling.
export function resolveSpeakerColor(
  entry: { speakerId?: string; speakerIndex?: number },
  speakers: Record<string, RecordingSpeaker> | null | undefined
): string | undefined {
  const custom = entry.speakerId !== undefined ? speakers?.[entry.speakerId]?.color : undefined;
  if (custom !== undefined && custom !== null && custom !== '') {
    return custom;
  }

  return entry.speakerIndex !== undefined ? speakerColor(entry.speakerIndex) : undefined;
}

export type SpeakerStat = {
  id: string;
  label: string;
  count: number;
  speakerIndex?: number;
};

/// Unique speakers of a transcript in first-appearance order with their
/// utterance counts — the data behind the participant chips row.
export function speakerStats(items: RecordingTranscriptItem[]): SpeakerStat[] {
  const stats = new Map<string, SpeakerStat>();

  for (const item of items) {
    const id = transcriptItemSpeakerId(item);
    const existing = stats.get(id);
    if (existing) {
      existing.count += 1;
    } else {
      stats.set(id, {
        id,
        label: item.speakerLabel,
        count: 1,
        ...(item.speakerIndex !== undefined ? { speakerIndex: item.speakerIndex } : {})
      });
    }
  }

  return [...stats.values()];
}
