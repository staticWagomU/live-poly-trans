import { describe, expect, it } from 'vitest';
import {
  applyGlossary,
  applyGlossaryToEntries,
  importGlossaryJson,
  parseGlossaryRules,
  serializeGlossaryRules,
  type GlossaryRule
} from './glossary';
import type { TranscriptEntry } from './export/types';

function rule(overrides: Partial<GlossaryRule> = {}): GlossaryRule {
  return { from: 'Cloud', to: 'Claude', matchType: 'text', enabled: true, ...overrides };
}

describe('applyGlossary', () => {
  it('replaces every occurrence of a text rule literally', () => {
    expect(applyGlossary('Cloud と Cloud の話', [rule()])).toBe('Claude と Claude の話');
  });

  it('treats text rules as literal even when they contain regex metacharacters', () => {
    const rules = [rule({ from: 'C++ (v2)', to: 'CPlusPlus' })];
    expect(applyGlossary('C++ (v2) 入門', rules)).toBe('CPlusPlus 入門');
  });

  it('does not expand $-patterns in a text rule replacement', () => {
    const rules = [rule({ from: 'price', to: '$100' })];
    expect(applyGlossary('the price is right', rules)).toBe('the $100 is right');
  });

  it('applies regex rules with groups and backreferences', () => {
    const rules = [rule({ from: '(\\d+)円', to: '¥$1', matchType: 'regex' })];
    expect(applyGlossary('300円と500円', rules)).toBe('¥300と¥500');
  });

  it('silently skips invalid regex patterns', () => {
    const rules = [rule({ from: '[unclosed', to: 'x', matchType: 'regex' }), rule()];
    expect(applyGlossary('Cloud [unclosed', rules)).toBe('Claude [unclosed');
  });

  it('skips disabled rules', () => {
    expect(applyGlossary('Cloud', [rule({ enabled: false })])).toBe('Cloud');
  });

  it('skips rules with an empty from', () => {
    expect(applyGlossary('abc', [rule({ from: '', to: 'x' })])).toBe('abc');
  });

  it('applies rules in array order so later rules see earlier output', () => {
    const rules = [rule({ from: 'AI', to: 'A.I.' }), rule({ from: 'A.I.', to: '人工知能' })];
    expect(applyGlossary('AI の未来', rules)).toBe('人工知能 の未来');
  });

  it('returns the text unchanged when there are no rules', () => {
    expect(applyGlossary('そのまま', [])).toBe('そのまま');
  });
});

describe('applyGlossaryToEntries', () => {
  function entry(text: string): TranscriptEntry {
    return {
      timestamp: '2026-08-05T00:00:00.000Z',
      speakerId: 'self',
      speakerLabel: 'Speaker A',
      language: 'ja',
      text,
      translation: 'Cloud translation'
    };
  }

  it('rewrites entry text but leaves translations untouched', () => {
    const result = applyGlossaryToEntries([entry('Cloud の話')], [rule()]);
    expect(result[0].text).toBe('Claude の話');
    expect(result[0].translation).toBe('Cloud translation');
  });

  it('does not mutate the input entries', () => {
    const input = [entry('Cloud')];
    applyGlossaryToEntries(input, [rule()]);
    expect(input[0].text).toBe('Cloud');
  });

  it('returns the same array reference when no rule is enabled', () => {
    const input = [entry('Cloud')];
    expect(applyGlossaryToEntries(input, [rule({ enabled: false })])).toBe(input);
    expect(applyGlossaryToEntries(input, [])).toBe(input);
  });
});

describe('parseGlossaryRules', () => {
  it('parses a serialized rule list round-trip', () => {
    const rules = [rule(), rule({ from: '(\\d+)s', to: '$1 seconds', matchType: 'regex' })];
    expect(parseGlossaryRules(serializeGlossaryRules(rules))).toEqual(rules);
  });

  it('returns an empty list for null input', () => {
    expect(parseGlossaryRules(null)).toEqual([]);
  });

  it('returns an empty list for malformed JSON', () => {
    expect(parseGlossaryRules('{not json')).toEqual([]);
  });

  it('returns an empty list when the JSON is not an array', () => {
    expect(parseGlossaryRules('{"from":"a","to":"b"}')).toEqual([]);
  });

  it('filters out items without string from/to', () => {
    const raw = JSON.stringify([{ from: 'a', to: 'b' }, { from: 1, to: 'b' }, 'junk', null]);
    expect(parseGlossaryRules(raw)).toEqual([
      { from: 'a', to: 'b', matchType: 'text', enabled: true }
    ]);
  });

  it('coerces unknown matchType to text and missing enabled to true', () => {
    const raw = JSON.stringify([
      { from: 'a', to: 'b', matchType: 'glob' },
      { from: 'c', to: 'd', matchType: 'regex', enabled: false }
    ]);
    expect(parseGlossaryRules(raw)).toEqual([
      { from: 'a', to: 'b', matchType: 'text', enabled: true },
      { from: 'c', to: 'd', matchType: 'regex', enabled: false }
    ]);
  });
});

describe('importGlossaryJson', () => {
  it('imports a JSON rule list with compatibility defaults', () => {
    const result = importGlossaryJson(
      JSON.stringify([
        { from: 'クロード コード', to: 'Claude Code' },
        { from: '(\\d+)円', to: '¥$1', matchType: 'regex', enabled: false }
      ])
    );

    expect(result).toEqual({
      ok: true,
      rules: [
        { from: 'クロード コード', to: 'Claude Code', matchType: 'text', enabled: true },
        { from: '(\\d+)円', to: '¥$1', matchType: 'regex', enabled: false }
      ]
    });
  });

  it('rejects malformed glossary JSON instead of silently clearing rules', () => {
    expect(importGlossaryJson('{"from":"a","to":"b"}')).toEqual({
      ok: false,
      error: '用語集JSONは配列である必要があります。'
    });
  });
});
