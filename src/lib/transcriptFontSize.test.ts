import { describe, expect, it } from 'vitest';
import {
  DEFAULT_TRANSCRIPT_FONT_SCALE,
  TRANSCRIPT_FONT_SCALE_STEPS,
  canDecreaseTranscriptFontScale,
  canIncreaseTranscriptFontScale,
  decreaseTranscriptFontScale,
  increaseTranscriptFontScale,
  parseTranscriptFontScale
} from './transcriptFontSize';

describe('transcript font scale steps', () => {
  it('includes the default scale so the initial state is a valid step', () => {
    expect(TRANSCRIPT_FONT_SCALE_STEPS).toContain(DEFAULT_TRANSCRIPT_FONT_SCALE);
  });

  it('increases to the next larger step', () => {
    expect(increaseTranscriptFontScale(1)).toBeGreaterThan(1);
  });

  it('decreases to the next smaller step', () => {
    expect(decreaseTranscriptFontScale(1)).toBeLessThan(1);
  });

  it('stays at the largest step when increasing from the maximum', () => {
    const max = TRANSCRIPT_FONT_SCALE_STEPS[TRANSCRIPT_FONT_SCALE_STEPS.length - 1];
    expect(increaseTranscriptFontScale(max)).toBe(max);
  });

  it('stays at the smallest step when decreasing from the minimum', () => {
    const min = TRANSCRIPT_FONT_SCALE_STEPS[0];
    expect(decreaseTranscriptFontScale(min)).toBe(min);
  });

  it('snaps an off-step value to the nearest step before moving', () => {
    expect(increaseTranscriptFontScale(1.02)).toBe(increaseTranscriptFontScale(1));
  });

  it('reports whether further increase or decrease is possible', () => {
    const min = TRANSCRIPT_FONT_SCALE_STEPS[0];
    const max = TRANSCRIPT_FONT_SCALE_STEPS[TRANSCRIPT_FONT_SCALE_STEPS.length - 1];
    expect(canIncreaseTranscriptFontScale(max)).toBe(false);
    expect(canDecreaseTranscriptFontScale(min)).toBe(false);
    expect(canIncreaseTranscriptFontScale(DEFAULT_TRANSCRIPT_FONT_SCALE)).toBe(true);
    expect(canDecreaseTranscriptFontScale(DEFAULT_TRANSCRIPT_FONT_SCALE)).toBe(true);
  });
});

describe('parseTranscriptFontScale', () => {
  it('restores a persisted step value', () => {
    const max = TRANSCRIPT_FONT_SCALE_STEPS[TRANSCRIPT_FONT_SCALE_STEPS.length - 1];
    expect(parseTranscriptFontScale(String(max))).toBe(max);
  });

  const invalidCases: Array<{ name: string; raw: string | null }> = [
    { name: 'falls back to the default when nothing was persisted', raw: null },
    { name: 'falls back to the default for non-numeric input', raw: 'large' },
    { name: 'falls back to the default for values outside the step list', raw: '9' }
  ];

  it.each(invalidCases)('$name', ({ raw }) => {
    expect(parseTranscriptFontScale(raw)).toBe(DEFAULT_TRANSCRIPT_FONT_SCALE);
  });
});
