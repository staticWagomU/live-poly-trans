import { describe, expect, it } from 'vitest';
import { chooseDefaultLanguagePair, languageControlLabel, transcriptionCandidateLanguages } from './languages';

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

  it('uses the system language as main when installed', () => {
    expect(
      chooseDefaultLanguagePair(
        [
          { id: 'en-US', label: 'English' },
          { id: 'ja-JP', label: 'Japanese' }
        ],
        'ja-JP'
      )
    ).toEqual({ source: 'ja-JP', target: 'en-US' });
  });

  it('falls back to English main when the system language is not installed', () => {
    expect(
      chooseDefaultLanguagePair(
        [
          { id: 'en-US', label: 'English' },
          { id: 'ja-JP', label: 'Japanese' }
        ],
        'fr-FR'
      )
    ).toEqual({ source: 'en-US', target: 'ja-JP' });
  });
});

describe('languageControlLabel', () => {
  it('uses a compact locale suffix for long region names', () => {
    expect(languageControlLabel({ id: 'en-AU', label: 'English (Australia)' })).toBe('English AU');
    expect(languageControlLabel({ id: 'ja-JP', label: 'Japanese (Japan)' })).toBe('Japanese JP');
  });
});

describe('transcriptionCandidateLanguages', () => {
  it('uses both display languages as transcription candidates', () => {
    expect(transcriptionCandidateLanguages('en-US', 'ja-JP')).toEqual(['en-US', 'ja-JP']);
  });

  it('deduplicates matching main and sub languages', () => {
    expect(transcriptionCandidateLanguages('ja-JP', 'ja-JP')).toEqual(['ja-JP']);
  });
});
