import { describe, expect, it } from 'vitest';
import {
  buildRecordingTranscript,
  formatFileSize,
  formatTimestampMs,
  parseSegmentStartMs
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
