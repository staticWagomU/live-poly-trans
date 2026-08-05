import { describe, expect, it } from 'vitest';
import { formatSrtTimestamp, toSrt } from './srt';
import type { TranscriptEntry } from './types';

function makeEntry(overrides: Partial<TranscriptEntry> = {}): TranscriptEntry {
  return {
    timestamp: '2026-06-15T00:00:00Z',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'en-US',
    text: 'hello',
    translation: 'こんにちは',
    ...overrides
  };
}

describe('formatSrtTimestamp', () => {
  it('zero-pads and rolls milliseconds over into hours', () => {
    expect(formatSrtTimestamp(0)).toBe('00:00:00,000');
    expect(formatSrtTimestamp(3_661_234)).toBe('01:01:01,234');
    expect(formatSrtTimestamp(59_999)).toBe('00:00:59,999');
  });
});

describe('toSrt', () => {
  it('returns an empty string for no entries', () => {
    expect(toSrt([], { speakerPrefix: false, translation: 'none' })).toBe('');
  });

  it('numbers cues from 1 and separates them with a blank line', () => {
    const srt = toSrt(
      [
        makeEntry({ startMs: 0, endMs: 1_200, text: 'first' }),
        makeEntry({ startMs: 1_500, endMs: 2_000, text: 'second' })
      ],
      { speakerPrefix: false, translation: 'none' }
    );

    expect(srt).toBe(
      '1\n' +
        '00:00:00,000 --> 00:00:01,200\n' +
        'first\n' +
        '\n' +
        '2\n' +
        '00:00:01,500 --> 00:00:02,000\n' +
        'second\n'
    );
  });

  it('renders speaker prefix and both translation lines inside one cue', () => {
    const srt = toSrt([makeEntry({ startMs: 0, endMs: 1_000 })], {
      speakerPrefix: true,
      translation: 'both'
    });

    expect(srt).toBe('1\n00:00:00,000 --> 00:00:01,000\nSpeaker A: hello\nこんにちは\n');
  });

  it('fills missing end times through timing resolution', () => {
    const srt = toSrt(
      [makeEntry({ startMs: 0, text: 'first' }), makeEntry({ startMs: 1_000, text: 'second' })],
      { speakerPrefix: false, translation: 'none' }
    );

    expect(srt).toContain('00:00:00,000 --> 00:00:01,000');
    expect(srt).toContain('00:00:01,000 --> 00:00:04,000');
  });

  it('collapses embedded newline runs in text so a cue never contains a blank line', () => {
    const srt = toSrt([makeEntry({ startMs: 0, endMs: 1_000, text: 'first\n\nsecond' })], {
      speakerPrefix: false,
      translation: 'none'
    });

    expect(srt).toBe('1\n00:00:00,000 --> 00:00:01,000\nfirst second\n');
  });

  it('synthesizes sequential cues for entries with no timing at all', () => {
    const srt = toSrt(
      [makeEntry({ timestamp: 'not-a-date' }), makeEntry({ timestamp: 'not-a-date' })],
      { speakerPrefix: false, translation: 'none' }
    );

    expect(srt).toContain('00:00:00,000 --> 00:00:03,000');
    expect(srt).toContain('00:00:03,000 --> 00:00:06,000');
  });
});
