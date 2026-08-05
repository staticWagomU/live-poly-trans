import { describe, expect, it } from 'vitest';
import { toVtt } from './vtt';
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

describe('toVtt', () => {
  it('returns only the WEBVTT header for no entries', () => {
    expect(toVtt([], { speakerPrefix: false, translation: 'none' })).toBe('WEBVTT\n');
  });

  it('emits header, period timestamps, and unnumbered cues separated by blank lines', () => {
    const vtt = toVtt(
      [
        makeEntry({ startMs: 0, endMs: 1_200, text: 'first' }),
        makeEntry({ startMs: 3_661_234, endMs: 3_662_000, text: 'second' })
      ],
      { speakerPrefix: false, translation: 'none' }
    );

    expect(vtt).toBe(
      'WEBVTT\n' +
        '\n' +
        '00:00:00.000 --> 00:00:01.200\n' +
        'first\n' +
        '\n' +
        '01:01:01.234 --> 01:01:02.000\n' +
        'second\n'
    );
  });

  it('renders speaker prefix and both translation lines inside one cue', () => {
    const vtt = toVtt([makeEntry({ startMs: 0, endMs: 1_000 })], {
      speakerPrefix: true,
      translation: 'both'
    });

    expect(vtt).toBe('WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nSpeaker A: hello\nこんにちは\n');
  });

  it('fills missing end times through timing resolution', () => {
    const vtt = toVtt([makeEntry({ startMs: 500 })], {
      speakerPrefix: false,
      translation: 'none'
    });

    expect(vtt).toContain('00:00:00.500 --> 00:00:03.500');
  });
});
