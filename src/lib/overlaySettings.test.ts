import { describe, expect, it } from 'vitest';
import {
  DEFAULT_OVERLAY_SETTINGS,
  parseOverlayFadeSeconds,
  parseOverlayFontScale,
  parseOverlayLineCount
} from './overlaySettings';

describe('overlay settings parsers', () => {
  it('clamps line count to 1 through 3', () => {
    expect(parseOverlayLineCount('1')).toBe(1);
    expect(parseOverlayLineCount('3')).toBe(3);
    expect(parseOverlayLineCount('9')).toBe(3);
    expect(parseOverlayLineCount('0')).toBe(1);
    expect(parseOverlayLineCount('junk')).toBe(DEFAULT_OVERLAY_SETTINGS.lineCount);
  });

  it('clamps fade seconds and allows zero for no fade', () => {
    expect(parseOverlayFadeSeconds('0')).toBe(0);
    expect(parseOverlayFadeSeconds('8')).toBe(8);
    expect(parseOverlayFadeSeconds('99')).toBe(10);
    expect(parseOverlayFadeSeconds('-1')).toBe(0);
    expect(parseOverlayFadeSeconds('junk')).toBe(DEFAULT_OVERLAY_SETTINGS.fadeSeconds);
  });

  it('clamps font scale', () => {
    expect(parseOverlayFontScale('0.8')).toBeCloseTo(0.8);
    expect(parseOverlayFontScale('2')).toBe(1.8);
    expect(parseOverlayFontScale('0.1')).toBe(0.7);
    expect(parseOverlayFontScale('junk')).toBe(DEFAULT_OVERLAY_SETTINGS.fontScale);
  });
});
