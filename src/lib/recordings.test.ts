import { describe, expect, it } from 'vitest';
import {
  buildRecordingTranscript,
  formatFileSize,
  formatTimestampMs,
  groupRecordingsByDate,
  parseSegmentStartMs,
  trimTranscript,
  type RecordingTranscriptItem
} from './recordings';

describe('parseSegmentStartMs', () => {
  it('reads the millisecond offset from a segment id', () => {
    expect(parseSegmentStartMs('12500-1800')).toBe(12_500);
    expect(parseSegmentStartMs('broken')).toBe(0);
    expect(parseSegmentStartMs(undefined)).toBe(0);
  });
});

describe('buildRecordingTranscript', () => {
  it('merges finals and follow-up translations sorted by audio position', () => {
    const events = [
      {
        type: 'transcript',
        stream: 'speaker',
        segmentId: '5000-2000',
        isFinal: true,
        lang: 'en-US',
        speakerLabel: 'Speaker B',
        text: 'the release is Friday',
        trans: null
      },
      {
        type: 'translation',
        stream: 'speaker',
        segmentId: '5000-2000',
        trans: 'リリースは金曜日です'
      },
      {
        type: 'transcript',
        stream: 'mic',
        segmentId: '1000-1500',
        isFinal: true,
        lang: 'ja-JP',
        speakerLabel: 'Speaker A',
        text: 'リリースはいつですか',
        trans: null
      }
    ];

    const transcript = buildRecordingTranscript(events);

    expect(transcript.map((item) => item.startMs)).toEqual([1_000, 5_000]);
    expect(transcript[1].translation).toBe('リリースは金曜日です');
    expect(transcript[0].speakerLabel).toBe('Speaker A');
  });

  it('keeps entries from before and after a helper restart apart', () => {
    // A restarted helper appends to the same jsonl with its clock reset to
    // 0, so segment ids repeat. The second run must not overwrite the first
    // run's lines; the rewind of startMs marks the run boundary.
    const events = [
      {
        type: 'transcript',
        stream: 'mic',
        segmentId: '0-1000',
        isFinal: true,
        lang: 'ja-JP',
        text: '一時間目の発話',
        trans: null
      },
      {
        type: 'transcript',
        stream: 'mic',
        segmentId: '5000-2000',
        isFinal: true,
        lang: 'ja-JP',
        text: 'クラッシュ直前の発話',
        trans: null
      },
      {
        type: 'transcript',
        stream: 'mic',
        segmentId: '0-1000',
        isFinal: true,
        lang: 'ja-JP',
        text: '再起動後の発話',
        trans: null
      },
      {
        type: 'translation',
        stream: 'mic',
        segmentId: '0-1000',
        trans: 'After-restart translation'
      }
    ];

    const transcript = buildRecordingTranscript(events);

    expect(transcript.map((item) => item.text)).toContain('一時間目の発話');
    expect(transcript.map((item) => item.text)).toContain('再起動後の発話');
    expect(transcript).toHaveLength(3);
    // The follow-up translation belongs to the post-restart run's segment.
    expect(transcript.find((item) => item.text === '再起動後の発話')?.translation).toBe(
      'After-restart translation'
    );
    expect(transcript.find((item) => item.text === '一時間目の発話')?.translation).toBeNull();
  });

  it('ignores interim events and unknown payloads', () => {
    const events = [
      { type: 'transcript', stream: 'mic', segmentId: '0-100', isFinal: false, text: 'partial' },
      { type: 'status', stream: 'mic', state: 'downloading-language' },
      null
    ];

    expect(buildRecordingTranscript(events)).toEqual([]);
  });
});

describe('formatTimestampMs', () => {
  it('formats minutes and seconds', () => {
    expect(formatTimestampMs(0)).toBe('0:00');
    expect(formatTimestampMs(65_000)).toBe('1:05');
    expect(formatTimestampMs(3_725_000)).toBe('1:02:05');
  });
});

describe('formatFileSize', () => {
  it('scales bytes to readable units', () => {
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(2_048)).toBe('2 KB');
    expect(formatFileSize(5_242_880)).toBe('5.0 MB');
  });
});

describe('groupRecordingsByDate', () => {
  const recording = (id: string, startedAt: string) => ({
    id,
    startedAt,
    endedAt: startedAt,
    files: []
  });
  // Fixed "now": 2026-07-31T12:00 local time.
  const now = new Date(2026, 6, 31, 12, 0, 0);

  it('buckets recordings into today / yesterday / last 30 days / older', () => {
    const groups = groupRecordingsByDate(
      [
        recording('today', new Date(2026, 6, 31, 9, 0).toISOString()),
        recording('yesterday', new Date(2026, 6, 30, 23, 30).toISOString()),
        recording('recent', new Date(2026, 6, 10, 8, 0).toISOString()),
        recording('old', new Date(2026, 4, 1, 8, 0).toISOString())
      ],
      now
    );

    expect(
      groups.map((group) => [group.label, group.recordings.map((entry) => entry.id)])
    ).toEqual([
      ['今日', ['today']],
      ['昨日', ['yesterday']],
      ['過去30日', ['recent']],
      ['それ以前', ['old']]
    ]);
  });

  it('omits empty buckets and sorts newest first inside each', () => {
    const groups = groupRecordingsByDate(
      [
        recording('older-today', new Date(2026, 6, 31, 8, 0).toISOString()),
        recording('newer-today', new Date(2026, 6, 31, 10, 0).toISOString())
      ],
      now
    );

    expect(groups).toHaveLength(1);
    expect(groups[0].label).toBe('今日');
    expect(groups[0].recordings.map((entry) => entry.id)).toEqual([
      'newer-today',
      'older-today'
    ]);
  });
});

describe('trimTranscript', () => {
  const item = (key: string, startMs: number): RecordingTranscriptItem => ({
    key,
    startMs,
    stream: 'mic',
    speakerLabel: 'Speaker A',
    language: 'ja-JP',
    text: key,
    translation: null
  });

  it('keeps only items inside the range and shifts them to the new origin', () => {
    const items = [item('before', 1_000), item('kept', 6_000), item('late', 12_000)];

    expect(trimTranscript(items, 5_000, 10_000)).toEqual([
      { ...item('kept', 6_000), startMs: 1_000 }
    ]);
  });

  it('keeps an item starting exactly at the range edges', () => {
    const items = [item('at-start', 5_000), item('at-end', 10_000)];

    expect(trimTranscript(items, 5_000, 10_000).map((entry) => entry.key)).toEqual([
      'at-start',
      'at-end'
    ]);
  });
});
