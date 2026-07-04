export type RecordingFileInfo = {
  name: string;
  stream: string;
  path: string;
  sizeBytes: number;
};

export type RecordingSummary = {
  id: string;
  startedAt: string;
  endedAt: string | null;
  files: RecordingFileInfo[];
};

export type RecordingWaveform = {
  durationMs: number;
  peaks: number[];
};

export type RecordingTranscriptItem = {
  key: string;
  startMs: number;
  stream: 'mic' | 'speaker';
  speakerLabel: string;
  language: string;
  text: string;
  translation: string | null;
};

export function parseSegmentStartMs(segmentId: unknown): number {
  if (typeof segmentId !== 'string') {
    return 0;
  }

  const start = Number(segmentId.split('-')[0]);
  return Number.isFinite(start) ? start : 0;
}

/// Merges per-stream transcript JSONL events (final transcripts + follow-up
/// translation events) into a single timeline sorted by audio position.
export function buildRecordingTranscript(events: unknown[]): RecordingTranscriptItem[] {
  const items = new Map<string, RecordingTranscriptItem>();

  for (const raw of events) {
    if (!raw || typeof raw !== 'object') {
      continue;
    }

    const event = raw as Record<string, unknown>;
    const stream = event.stream === 'mic' ? 'mic' : event.stream === 'speaker' ? 'speaker' : null;
    if (!stream || typeof event.segmentId !== 'string') {
      continue;
    }

    const key = `${stream}-${event.segmentId}`;

    if (event.type === 'transcript' && event.isFinal === true && typeof event.text === 'string') {
      const existing = items.get(key);
      items.set(key, {
        key,
        startMs: parseSegmentStartMs(event.segmentId),
        stream,
        speakerLabel:
          typeof event.speakerLabel === 'string'
            ? event.speakerLabel
            : stream === 'mic'
              ? 'Speaker A'
              : 'Speaker B',
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
