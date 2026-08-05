/// Shared speaker identity helpers: the one place that knows how speakers
/// are identified (`speaker-N` for diarized speakers, the stream name
/// otherwise), labeled, and colored. The ID scheme here is the key system
/// for the upcoming `speakers` map in meta.json.

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
