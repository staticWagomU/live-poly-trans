import type { ActionItem } from './actionItems';
import { defaultStreamSpeakerLabel, diarizedSpeakerLabel } from './speakers';

export type RecordingFileInfo = {
  name: string;
  stream: string;
  path: string;
  sizeBytes: number;
};

export type RecordingTrimRange = {
  sourceFile: string;
  startMs: number;
  endMs: number;
};

/// Custom speaker identity stored in meta.json, keyed by the stable speaker
/// id (`speaker-N` / stream name). Transcript jsonl is never rewritten;
/// names resolve at display/export time.
export type RecordingSpeaker = {
  name: string;
  color?: string | null;
};

export type RecordingActionItem = ActionItem;

export type RecordingSummary = {
  id: string;
  startedAt: string;
  endedAt: string | null;
  /// "external" for imported audio files.
  source?: string | null;
  /// Trim ranges keyed by trimmed output file name.
  trims?: Record<string, RecordingTrimRange> | null;
  /// Custom speaker names/colors keyed by speaker id.
  speakers?: Record<string, RecordingSpeaker> | null;
  /// AI action items persisted in meta.json.
  actions?: RecordingActionItem[] | null;
  files: RecordingFileInfo[];
};

export type RecordingWaveform = {
  durationMs: number;
  peaks: number[];
};

export type RecordingTranscriptItem = {
  key: string;
  startMs: number;
  /// Audio position where the segment ends; missing for legacy jsonl lines
  /// that never carried a duration.
  endMs?: number;
  stream: 'mic' | 'speaker' | 'unknown';
  speakerLabel: string;
  language: string;
  text: string;
  translation: string | null;
  /// Diarized speaker number (0-based) for WhisperX results; drives the
  /// per-speaker color in the transcript view.
  speakerIndex?: number;
};

export type RecordingSearchHit = {
  recordingId: string;
  entryIndex: number;
  snippet: string;
  timestampMs: number;
};

export function parseSegmentStartMs(segmentId: unknown): number {
  if (typeof segmentId !== 'string') {
    return 0;
  }

  const start = Number(segmentId.split('-')[0]);
  return Number.isFinite(start) ? start : 0;
}

/// Reads both ends of a `"<startMs>-<durationMs>"` segment id.
/// Unlike parseSegmentStartMs this rejects malformed ids instead of
/// defaulting to 0, so callers can leave timings undefined.
export function parseSegmentTiming(segmentId: unknown): { startMs: number; endMs: number } | null {
  if (typeof segmentId !== 'string') {
    return null;
  }

  const [startPart, durationPart] = segmentId.split('-');
  const startMs = Number(startPart);
  const durationMs = Number(durationPart);
  if (startPart === '' || durationPart === undefined || durationPart === '') {
    return null;
  }
  if (!Number.isFinite(startMs) || !Number.isFinite(durationMs)) {
    return null;
  }

  return { startMs, endMs: startMs + durationMs };
}

/// Merges per-stream transcript JSONL events (final transcripts + follow-up
/// translation events) into a single timeline sorted by audio position.
///
/// A helper restarted mid-session appends to the same jsonl with its capture
/// clock reset to 0, so segment ids repeat across runs. The rewind of a
/// stream's monotonic startMs marks the run boundary; keys are namespaced by
/// run so a later run never overwrites earlier lines.
export function buildRecordingTranscript(events: unknown[]): RecordingTranscriptItem[] {
  const items = new Map<string, RecordingTranscriptItem>();
  const runByStream: Record<string, number> = {};
  const lastStartByStream: Record<string, number> = {};

  for (const raw of events) {
    if (!raw || typeof raw !== 'object') {
      continue;
    }

    const event = raw as Record<string, unknown>;
    const stream = event.stream === 'mic' ? 'mic' : event.stream === 'speaker' ? 'speaker' : null;
    if (!stream || typeof event.segmentId !== 'string') {
      continue;
    }

    if (event.type === 'transcript' && event.isFinal === true) {
      const startMs = parseSegmentStartMs(event.segmentId);
      const lastStartMs = lastStartByStream[stream];
      if (lastStartMs !== undefined && startMs < lastStartMs) {
        runByStream[stream] = (runByStream[stream] ?? 0) + 1;
      }
      lastStartByStream[stream] = startMs;
    }

    const key = `${runByStream[stream] ?? 0}-${stream}-${event.segmentId}`;

    if (event.type === 'transcript' && event.isFinal === true && typeof event.text === 'string') {
      const existing = items.get(key);
      const timing = parseSegmentTiming(event.segmentId);
      items.set(key, {
        key,
        startMs: parseSegmentStartMs(event.segmentId),
        ...(timing !== null ? { endMs: timing.endMs } : {}),
        stream,
        speakerLabel:
          typeof event.speakerLabel === 'string'
            ? event.speakerLabel
            : defaultStreamSpeakerLabel(stream),
        language: typeof event.lang === 'string' ? event.lang : 'und',
        text: event.text,
        translation:
          typeof event.trans === 'string' && event.trans.length > 0
            ? event.trans
            : (existing?.translation ?? null)
      });
    } else if (event.type === 'translation' && typeof event.trans === 'string') {
      const existing = items.get(key);
      if (existing) {
        existing.translation = event.trans;
      }
    }
  }

  return [...items.values()].sort((left, right) => left.startMs - right.startMs);
}

/// "SPEAKER_07" -> 7; anything unparseable maps to the first slot.
export function whisperxSpeakerIndex(speaker: string): number {
  const match = speaker.match(/(\d+)\s*$/);
  const parsed = match ? Number.parseInt(match[1], 10) : Number.NaN;
  return Number.isFinite(parsed) ? parsed : 0;
}

/// Builds the transcript view of a WhisperX re-process from the mixed jsonl
/// event stream (whisperx lines coexist with the live transcript lines).
export function buildWhisperxTranscript(events: unknown[]): RecordingTranscriptItem[] {
  const items: RecordingTranscriptItem[] = [];

  for (const raw of events) {
    if (!raw || typeof raw !== 'object') {
      continue;
    }

    const event = raw as Record<string, unknown>;
    if (event.type !== 'whisperx' || typeof event.text !== 'string') {
      continue;
    }

    const startMs = typeof event.startMs === 'number' ? event.startMs : 0;
    const endMs = typeof event.endMs === 'number' ? event.endMs : undefined;
    const speaker = typeof event.speaker === 'string' ? event.speaker : null;
    const speakerIndex = speaker !== null ? whisperxSpeakerIndex(speaker) : undefined;

    items.push({
      key: `wx-${startMs}-${items.length}`,
      startMs,
      ...(endMs !== undefined ? { endMs } : {}),
      stream: 'unknown',
      speakerLabel: speakerIndex !== undefined ? diarizedSpeakerLabel(speakerIndex) : '話者',
      language: typeof event.lang === 'string' ? event.lang : 'und',
      text: event.text,
      translation: null,
      ...(speakerIndex !== undefined ? { speakerIndex } : {})
    });
  }

  return items.sort((left, right) => left.startMs - right.startMs);
}

export type RecordingGroup = {
  label: string;
  recordings: RecordingSummary[];
};

/// Sidebar buckets, mockup order: 今日 / 昨日 / 過去30日 / それ以前.
/// Buckets are calendar-based (local time), not rolling 24h windows.
export function groupRecordingsByDate(
  recordings: RecordingSummary[],
  now: Date
): RecordingGroup[] {
  const startOfDay = (date: Date) =>
    new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const todayStart = startOfDay(now);
  const yesterdayStart = todayStart - 24 * 60 * 60 * 1000;
  const thirtyDaysStart = todayStart - 30 * 24 * 60 * 60 * 1000;

  const buckets: RecordingGroup[] = [
    { label: '今日', recordings: [] },
    { label: '昨日', recordings: [] },
    { label: '過去30日', recordings: [] },
    { label: 'それ以前', recordings: [] }
  ];

  const sorted = [...recordings].sort(
    (left, right) => Date.parse(right.startedAt) - Date.parse(left.startedAt)
  );

  for (const recording of sorted) {
    const startedAt = Date.parse(recording.startedAt);
    if (!Number.isFinite(startedAt) || startedAt < thirtyDaysStart) {
      buckets[3].recordings.push(recording);
    } else if (startedAt >= todayStart) {
      buckets[0].recordings.push(recording);
    } else if (startedAt >= yesterdayStart) {
      buckets[1].recordings.push(recording);
    } else {
      buckets[2].recordings.push(recording);
    }
  }

  return buckets.filter((bucket) => bucket.recordings.length > 0);
}

/// Transcript counterpart of trimming the audio to [startMs, endMs]:
/// items outside the kept range disappear and the survivors' timestamps
/// shift so the range start becomes the new zero.
export function trimTranscript(
  items: RecordingTranscriptItem[],
  startMs: number,
  endMs: number
): RecordingTranscriptItem[] {
  return items
    .filter((item) => item.startMs >= startMs && item.startMs <= endMs)
    .map((item) => ({
      ...item,
      startMs: item.startMs - startMs,
      ...(item.endMs !== undefined ? { endMs: item.endMs - startMs } : {})
    }));
}

/// Scales peaks against the loudest bucket of the same file. Mic input often
/// peaks around 0.01, which is a 1px line on an absolute scale; the waveform is
/// a navigation aid, not a level meter, so shape matters more than loudness.
export function waveformDisplayPeaks(peaks: number[]): number[] {
  const loudest = peaks.reduce((max, peak) => Math.max(max, peak), 0);
  if (loudest <= 0) {
    return peaks.map(() => 0);
  }

  return peaks.map((peak) => Math.min(1, peak / loudest));
}

export function formatTimestampMs(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const paddedSeconds = String(seconds).padStart(2, '0');

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${paddedSeconds}`;
  }

  return `${minutes}:${paddedSeconds}`;
}

export function formatFileSize(sizeBytes: number): string {
  if (sizeBytes >= 1024 * 1024) {
    return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (sizeBytes >= 1024) {
    return `${Math.round(sizeBytes / 1024)} KB`;
  }

  return `${sizeBytes} B`;
}
