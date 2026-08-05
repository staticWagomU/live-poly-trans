import type { ChatMessage } from '../transcripts';
import {
  formatTimestampMs,
  parseSegmentTiming,
  type RecordingTranscriptItem
} from '../recordings';

/// Normalized export-facing transcript line: what every export format
/// (clipboard text, JSON, TXT) needs, decoupled from UI-only ChatMessage
/// state such as interim flags and rendering ids.
export type TranscriptEntry = {
  timestamp: string;
  speakerId: string;
  speakerLabel: string;
  language: string;
  text: string;
  translation: string | null;
  startMs?: number;
  endMs?: number;
};

/// How exporters render each entry's text.
/// - speakerPrefix: prepend `"<speakerLabel>: "` to the first text line.
/// - translation: 'none' emits the original only, 'both' emits the original
///   followed by the translation, 'translationOnly' emits the translation but
///   falls back to the original when none exists (a cue is never empty).
export type ExportOptions = {
  speakerPrefix: boolean;
  translation: 'none' | 'both' | 'translationOnly';
};

export function chatMessageToTranscriptEntry(message: ChatMessage): TranscriptEntry {
  const timing = parseSegmentTiming(message.segmentId);
  return {
    timestamp: message.timestamp,
    speakerId: message.speakerId,
    speakerLabel: message.speakerLabel,
    language: message.language,
    text: message.text,
    translation: message.translation,
    ...(timing !== null ? timing : {})
  };
}

export function chatMessagesToTranscriptEntries(messages: ChatMessage[]): TranscriptEntry[] {
  return messages.map(chatMessageToTranscriptEntry);
}

/// Recording items only carry capture-relative positions, so the timestamp
/// becomes absolute (ISO) only when the recording's start time is supplied;
/// otherwise it falls back to a relative `M:SS` label.
export function recordingItemToTranscriptEntry(
  item: RecordingTranscriptItem,
  opts: { baseTimestamp?: string } = {}
): TranscriptEntry {
  const baseMs = opts.baseTimestamp !== undefined ? Date.parse(opts.baseTimestamp) : Number.NaN;
  return {
    timestamp: Number.isFinite(baseMs)
      ? new Date(baseMs + item.startMs).toISOString()
      : formatTimestampMs(item.startMs),
    speakerId: item.speakerIndex !== undefined ? `speaker-${item.speakerIndex}` : item.stream,
    speakerLabel: item.speakerLabel,
    language: item.language,
    text: item.text,
    translation: item.translation,
    startMs: item.startMs,
    ...(item.endMs !== undefined ? { endMs: item.endMs } : {})
  };
}
