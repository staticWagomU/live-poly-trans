import { describe, expect, it } from 'vitest';
import { chooseDefaultLanguagePair, languageControlLabel, transcriptionLanguagesForStream } from './languages';

describe('chooseDefaultLanguagePair', () => {
  it('prefers Japanese and English when both installed', () => {
    expect(
      chooseDefaultLanguagePair([
        { id: 'zh-CN', label: 'Chinese' },
        { id: 'en-US', label: 'English' },
        { id: 'ja-JP', label: 'Japanese' }
      ])
    ).toEqual({ source: 'en-US', target: 'ja-JP' });
  });
});

describe('languageControlLabel', () => {
  it('uses a compact locale suffix for long region names', () => {
    expect(languageControlLabel({ id: 'en-AU', label: 'English (Australia)' })).toBe('English AU');
    expect(languageControlLabel({ id: 'ja-JP', label: 'Japanese (Japan)' })).toBe('Japanese JP');
  });
});

describe('transcriptionLanguagesForStream', () => {
  it('uses only the fixed mic language for mic and both languages for speaker', () => {
    expect(transcriptionLanguagesForStream('mic', 'en-US', 'ja-JP')).toEqual(['ja-JP']);
    expect(transcriptionLanguagesForStream('speaker', 'en-US', 'ja-JP')).toEqual([
      'en-US',
      'ja-JP'
    ]);
  });
});
