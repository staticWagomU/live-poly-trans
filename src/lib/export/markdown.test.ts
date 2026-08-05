import { describe, expect, it } from 'vitest';
import { toMarkdown } from './markdown';
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

describe('toMarkdown', () => {
  it('renders heading, meta lines, and timestamped entries', () => {
    const markdown = toMarkdown(
      [
        makeEntry({ startMs: 0 }),
        makeEntry({ startMs: 3_725_000, text: 'second', translation: null, speakerLabel: 'Speaker B' })
      ],
      {
        title: '定例ミーティング',
        dateLabel: '2026-06-15',
        participants: ['Speaker A', 'Speaker B']
      },
      { speakerPrefix: true, translation: 'both' }
    );

    expect(markdown).toBe(
      '# 定例ミーティング\n' +
        '\n' +
        '- 日時: 2026-06-15\n' +
        '- 参加者: Speaker A, Speaker B\n' +
        '\n' +
        '- [0:00] Speaker A: hello\n' +
        '  こんにちは\n' +
        '- [1:02:05] Speaker B: second\n'
    );
  });

  it('falls back to a generic title and skips absent meta lines', () => {
    const markdown = toMarkdown(
      [makeEntry({ startMs: 1_000 })],
      {},
      { speakerPrefix: false, translation: 'none' }
    );

    expect(markdown).toBe('# 文字起こし\n\n- [0:01] hello\n');
  });

  it('renders just the heading and meta for no entries', () => {
    expect(toMarkdown([], { dateLabel: '2026-06-15' }, { speakerPrefix: true, translation: 'none' })).toBe(
      '# 文字起こし\n\n- 日時: 2026-06-15\n'
    );
  });

  it('appends summary and action item sections only when provided', () => {
    const markdown = toMarkdown(
      [makeEntry({ startMs: 0, translation: null })],
      {
        summary: '来週リリースする。',
        actionItems: ['告知文を書く', 'QA を依頼する']
      },
      { speakerPrefix: false, translation: 'none' }
    );

    expect(markdown).toBe(
      '# 文字起こし\n' +
        '\n' +
        '- [0:00] hello\n' +
        '\n' +
        '## 要約\n' +
        '\n' +
        '来週リリースする。\n' +
        '\n' +
        '## アクションアイテム\n' +
        '\n' +
        '- 告知文を書く\n' +
        '- QA を依頼する\n'
    );
  });

  it('synthesizes sequential timestamps for entries with no timing', () => {
    const markdown = toMarkdown(
      [makeEntry({ timestamp: 'not-a-date' }), makeEntry({ timestamp: 'not-a-date' })],
      {},
      { speakerPrefix: false, translation: 'none' }
    );

    expect(markdown).toContain('- [0:00] hello');
    expect(markdown).toContain('- [0:03] hello');
  });
});
