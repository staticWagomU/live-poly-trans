import { describe, expect, it } from 'vitest';
import { entryLines } from './entryLines';
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

describe('entryLines', () => {
  it('emits the original text only for translation "none"', () => {
    expect(entryLines(makeEntry(), { speakerPrefix: false, translation: 'none' })).toEqual([
      'hello'
    ]);
  });

  it('emits original then translation for translation "both"', () => {
    expect(entryLines(makeEntry(), { speakerPrefix: false, translation: 'both' })).toEqual([
      'hello',
      'こんにちは'
    ]);
  });

  it('omits the translation line for "both" when there is none', () => {
    expect(
      entryLines(makeEntry({ translation: null }), { speakerPrefix: false, translation: 'both' })
    ).toEqual(['hello']);
  });

  it('emits the translation only for "translationOnly"', () => {
    expect(
      entryLines(makeEntry(), { speakerPrefix: false, translation: 'translationOnly' })
    ).toEqual(['こんにちは']);
  });

  it('falls back to the original for "translationOnly" without a translation', () => {
    expect(
      entryLines(makeEntry({ translation: null }), {
        speakerPrefix: false,
        translation: 'translationOnly'
      })
    ).toEqual(['hello']);
  });

  it('prefixes the speaker label on the first line only', () => {
    expect(entryLines(makeEntry(), { speakerPrefix: true, translation: 'both' })).toEqual([
      'Speaker A: hello',
      'こんにちは'
    ]);
  });
});
