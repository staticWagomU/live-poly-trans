import { describe, expect, it } from 'vitest';
import { toPlainText } from './plainText';
import type { TranscriptEntry } from './types';

function makeEntry(overrides: Partial<TranscriptEntry> = {}): TranscriptEntry {
  return {
    timestamp: '2026-06-15T00:00:00Z',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'en-US',
    text: 'hello',
    translation: null,
    ...overrides
  };
}

describe('toPlainText', () => {
  it('formats an entry without translation as a single line', () => {
    expect(toPlainText([makeEntry()])).toBe('[2026-06-15T00:00:00Z] Speaker A / en-US: hello');
  });

  it('appends the translation on an indented arrow line', () => {
    expect(toPlainText([makeEntry({ translation: 'こんにちは' })])).toBe(
      '[2026-06-15T00:00:00Z] Speaker A / en-US: hello\n  => こんにちは'
    );
  });

  it('omits the translation line for an empty-string translation', () => {
    expect(toPlainText([makeEntry({ translation: '' })])).toBe(
      '[2026-06-15T00:00:00Z] Speaker A / en-US: hello'
    );
  });

  it('joins entries with a newline and no trailing newline', () => {
    const text = toPlainText([
      makeEntry({ text: 'first', translation: 'はじめ' }),
      makeEntry({
        timestamp: '2026-06-15T00:00:05Z',
        speakerId: 'system-audio',
        speakerLabel: 'Speaker B',
        language: 'ja-JP',
        text: 'second'
      })
    ]);

    expect(text).toBe(
      '[2026-06-15T00:00:00Z] Speaker A / en-US: first\n' +
        '  => はじめ\n' +
        '[2026-06-15T00:00:05Z] Speaker B / ja-JP: second'
    );
  });

  it('returns an empty string for no entries', () => {
    expect(toPlainText([])).toBe('');
  });
});
