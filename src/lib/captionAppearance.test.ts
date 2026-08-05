import { describe, expect, it } from 'vitest';
import {
  captionFontFamilyValue,
  captionLineHeightValue,
  DEFAULT_CAPTION_FONT_FAMILY,
  DEFAULT_CAPTION_LINE_HEIGHT,
  parseCaptionFontFamily,
  parseCaptionLineHeight
} from './captionAppearance';

describe('caption appearance', () => {
  it('parses known font family preferences and falls back to system', () => {
    expect(parseCaptionFontFamily('system')).toBe('system');
    expect(parseCaptionFontFamily('rounded')).toBe('rounded');
    expect(parseCaptionFontFamily('serif')).toBe('serif');
    expect(parseCaptionFontFamily('mono')).toBe(DEFAULT_CAPTION_FONT_FAMILY);
    expect(parseCaptionFontFamily(null)).toBe(DEFAULT_CAPTION_FONT_FAMILY);
  });

  it('maps font family preferences to concrete CSS stacks', () => {
    expect(captionFontFamilyValue('system')).toContain('-apple-system');
    expect(captionFontFamilyValue('rounded')).toContain('SF Pro Rounded');
    expect(captionFontFamilyValue('serif')).toContain('Hiragino Mincho ProN');
  });

  it('parses line-height preferences and falls back to normal', () => {
    expect(parseCaptionLineHeight('compact')).toBe('compact');
    expect(parseCaptionLineHeight('normal')).toBe('normal');
    expect(parseCaptionLineHeight('relaxed')).toBe('relaxed');
    expect(parseCaptionLineHeight('wide')).toBe(DEFAULT_CAPTION_LINE_HEIGHT);
    expect(parseCaptionLineHeight(null)).toBe(DEFAULT_CAPTION_LINE_HEIGHT);
  });

  it('maps line-height preferences to stable numeric CSS values', () => {
    expect(captionLineHeightValue('compact')).toBe(1.25);
    expect(captionLineHeightValue('normal')).toBe(1.45);
    expect(captionLineHeightValue('relaxed')).toBe(1.7);
  });
});
