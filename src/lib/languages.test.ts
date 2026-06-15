import { describe, expect, it } from 'vitest';
import { chooseDefaultLanguagePair } from './languages';

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
