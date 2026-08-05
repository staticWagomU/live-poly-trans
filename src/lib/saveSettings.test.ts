import { describe, expect, it } from 'vitest';
import { markdownExportPath, renderFileName } from './saveSettings';

describe('renderFileName', () => {
  it('replaces date time title and language placeholders', () => {
    expect(
      renderFileName('{date} {time} {title} {lang}', {
        date: new Date('2026-08-05T05:00:00.000Z'),
        title: '定例ミーティング',
        lang: 'ja-en'
      })
    ).toBe('2026-08-05 1400 定例ミーティング ja-en');
  });

  it('sanitizes values that are unsafe in file names', () => {
    expect(
      renderFileName('{date}/{time}:{title}', {
        date: new Date('2026-08-05T05:00:00.000Z'),
        title: 'A/B:検討',
        lang: 'ja'
      })
    ).toBe('2026-08-05-1400-A-B-検討');
  });

  it('falls back to transcript when the rendered result is blank', () => {
    expect(
      renderFileName('   ', {
        date: new Date('2026-08-05T05:00:00.000Z'),
        title: '定例',
        lang: 'ja'
      })
    ).toBe('transcript');
  });
});

describe('markdownExportPath', () => {
  it('joins the export directory and rendered markdown file name', () => {
    expect(
      markdownExportPath('/Users/me/Meetings', '{date} {time} {title}', {
        date: new Date('2026-08-05T05:00:00.000Z'),
        title: '定例',
        lang: 'ja-en'
      })
    ).toBe('/Users/me/Meetings/2026-08-05 1400 定例.md');
  });

  it('does not duplicate the slash when the directory already ends with one', () => {
    expect(
      markdownExportPath('/Users/me/Meetings/', '{title}', {
        date: new Date('2026-08-05T05:00:00.000Z'),
        title: '定例',
        lang: 'ja-en'
      })
    ).toBe('/Users/me/Meetings/定例.md');
  });
});
