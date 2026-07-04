import { describe, expect, it } from 'vitest';
import { filterLanguagePacks, partitionLanguagePacks } from './languagePacks';

describe('partitionLanguagePacks', () => {
  it('splits supported languages into installed and available groups', () => {
    const installed = [
      { id: 'ja-JP', label: 'Japanese (Japan)' },
      { id: 'en-US', label: 'English (United States)' }
    ];
    const supported = [
      { id: 'en-US', label: 'English (United States)' },
      { id: 'fr-FR', label: 'French (France)' },
      { id: 'ja-JP', label: 'Japanese (Japan)' },
      { id: 'de-DE', label: 'German (Germany)' }
    ];

    expect(partitionLanguagePacks(installed, supported)).toEqual({
      installed: [
        { id: 'en-US', label: 'English (United States)' },
        { id: 'ja-JP', label: 'Japanese (Japan)' }
      ],
      available: [
        { id: 'fr-FR', label: 'French (France)' },
        { id: 'de-DE', label: 'German (Germany)' }
      ]
    });
  });

  it('sorts both groups by label', () => {
    const { installed, available } = partitionLanguagePacks(
      [
        { id: 'ja-JP', label: 'Japanese (Japan)' },
        { id: 'de-DE', label: 'German (Germany)' }
      ],
      [
        { id: 'ja-JP', label: 'Japanese (Japan)' },
        { id: 'de-DE', label: 'German (Germany)' },
        { id: 'zh-CN', label: 'Chinese (China mainland)' },
        { id: 'ar-SA', label: 'Arabic (Saudi Arabia)' }
      ]
    );

    expect(installed.map((language) => language.label)).toEqual([
      'German (Germany)',
      'Japanese (Japan)'
    ]);
    expect(available.map((language) => language.label)).toEqual([
      'Arabic (Saudi Arabia)',
      'Chinese (China mainland)'
    ]);
  });

  it('keeps installed languages missing from the supported list', () => {
    const { installed } = partitionLanguagePacks(
      [{ id: 'xx-XX', label: 'Legacy' }],
      [{ id: 'en-US', label: 'English (United States)' }]
    );

    expect(installed).toEqual([{ id: 'xx-XX', label: 'Legacy' }]);
  });
});

describe('filterLanguagePacks', () => {
  const languages = [
    { id: 'en-US', label: 'English (United States)' },
    { id: 'en-GB', label: 'English (United Kingdom)' },
    { id: 'ja-JP', label: 'Japanese (Japan)' }
  ];

  it('matches case-insensitively against the label', () => {
    expect(filterLanguagePacks(languages, 'english')).toEqual([
      { id: 'en-US', label: 'English (United States)' },
      { id: 'en-GB', label: 'English (United Kingdom)' }
    ]);
  });

  it('matches against the locale id', () => {
    expect(filterLanguagePacks(languages, 'ja-')).toEqual([
      { id: 'ja-JP', label: 'Japanese (Japan)' }
    ]);
  });

  it('returns everything for a blank query', () => {
    expect(filterLanguagePacks(languages, '  ')).toEqual(languages);
  });
});
