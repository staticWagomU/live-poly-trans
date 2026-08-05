import { describe, expect, it } from 'vitest';
import {
  buildTextExport,
  sanitizeFileName,
  timestampLabel,
  uniqueSpeakerLabels
} from './saveTextExport';
import type { TranscriptEntry } from './types';

function makeEntry(overrides: Partial<TranscriptEntry> = {}): TranscriptEntry {
  return {
    timestamp: '2026-06-15T00:00:00Z',
    speakerId: 'self',
    speakerLabel: '自分',
    language: 'en-US',
    text: 'hello',
    translation: 'こんにちは',
    startMs: 1000,
    endMs: 2500,
    ...overrides
  };
}

describe('buildTextExport', () => {
  it('builds a Markdown minutes file with meta and .md extension', () => {
    const file = buildTextExport('markdown', [makeEntry()], {
      baseName: 'meeting',
      markdownMeta: { title: '定例', dateLabel: '2026/06/15', participants: ['自分'] }
    });

    expect(file.extension).toBe('md');
    expect(file.defaultFileName).toBe('meeting.md');
    expect(file.contents).toContain('# 定例');
    expect(file.contents).toContain('- 参加者: 自分');
    expect(file.contents).toContain('自分: hello');
    expect(file.contents).toContain('こんにちは');
  });

  it('builds an SRT file with comma timestamps', () => {
    const file = buildTextExport('srt', [makeEntry()], { baseName: 'meeting' });

    expect(file.extension).toBe('srt');
    expect(file.defaultFileName).toBe('meeting.srt');
    expect(file.contents).toContain('00:00:01,000 --> 00:00:02,500');
    expect(file.contents).toContain('自分: hello');
  });

  it('builds a VTT file with the WEBVTT header', () => {
    const file = buildTextExport('vtt', [makeEntry()], { baseName: 'meeting' });

    expect(file.extension).toBe('vtt');
    expect(file.contents.startsWith('WEBVTT')).toBe(true);
    expect(file.contents).toContain('00:00:01.000 --> 00:00:02.500');
  });

  it('builds the legacy plain-text format for txt', () => {
    const file = buildTextExport('txt', [makeEntry()], { baseName: 'meeting' });

    expect(file.extension).toBe('txt');
    expect(file.contents).toBe('[2026-06-15T00:00:00Z] 自分 / en-US: hello\n  => こんにちは');
  });

  it('defaults to speaker prefix + both translation lanes', () => {
    const file = buildTextExport('srt', [makeEntry()], { baseName: 'meeting' });

    expect(file.contents).toContain('自分: hello\nこんにちは');
  });

  it('honors an explicit export options override', () => {
    const file = buildTextExport('srt', [makeEntry()], {
      baseName: 'meeting',
      exportOptions: { speakerPrefix: false, translation: 'none' }
    });

    expect(file.contents).toContain('hello');
    expect(file.contents).not.toContain('自分:');
    expect(file.contents).not.toContain('こんにちは');
  });

  it('sanitizes the base name in defaultFileName', () => {
    const file = buildTextExport('txt', [makeEntry()], { baseName: '2026/06/15 14:00' });

    expect(file.defaultFileName).toBe('2026-06-15 14-00.txt');
  });

  it('gives every format a non-empty dialog filter name', () => {
    for (const format of ['markdown', 'srt', 'vtt', 'txt'] as const) {
      expect(buildTextExport(format, [], { baseName: 'x' }).filterName).not.toBe('');
    }
  });
});

describe('sanitizeFileName', () => {
  it('replaces path separators and reserved characters with hyphens', () => {
    expect(sanitizeFileName('a/b\\c:d*e?f"g<h>i|j')).toBe('a-b-c-d-e-f-g-h-i-j');
  });

  it('falls back to "transcript" when nothing usable remains', () => {
    expect(sanitizeFileName('   ')).toBe('transcript');
    expect(sanitizeFileName('')).toBe('transcript');
  });
});

describe('uniqueSpeakerLabels', () => {
  it('keeps first-appearance order and drops duplicates and empties', () => {
    const labels = uniqueSpeakerLabels([
      makeEntry({ speakerLabel: '相手' }),
      makeEntry({ speakerLabel: '自分' }),
      makeEntry({ speakerLabel: '相手' }),
      makeEntry({ speakerLabel: '' })
    ]);

    expect(labels).toEqual(['相手', '自分']);
  });
});

describe('timestampLabel', () => {
  it('formats a local date as YYYYMMDD-HHMMSS with zero padding', () => {
    expect(timestampLabel(new Date(2026, 0, 5, 9, 3, 7))).toBe('20260105-090307');
  });
});
