import { describe, expect, it } from 'vitest';
import {
  decreaseMimiScale,
  DEFAULT_MIMI_SCALE,
  increaseMimiScale,
  MAX_MIMI_SCALE,
  MIN_MIMI_SCALE,
  parseMimiScale
} from './mimiDisplay';

describe('mimi display scale', () => {
  it('steps up and down from the default', () => {
    expect(increaseMimiScale(1)).toBeCloseTo(1.15);
    expect(decreaseMimiScale(1)).toBeCloseTo(0.85);
  });

  it('clamps to the allowed range', () => {
    expect(increaseMimiScale(MAX_MIMI_SCALE)).toBe(MAX_MIMI_SCALE);
    expect(decreaseMimiScale(MIN_MIMI_SCALE)).toBe(MIN_MIMI_SCALE);
  });

  it('parses a stored preference and rejects junk', () => {
    expect(parseMimiScale('1.45')).toBeCloseTo(1.45);
    expect(parseMimiScale('not-a-number')).toBe(DEFAULT_MIMI_SCALE);
    expect(parseMimiScale(null)).toBe(DEFAULT_MIMI_SCALE);
    expect(parseMimiScale('99')).toBe(MAX_MIMI_SCALE);
    expect(parseMimiScale('0.1')).toBe(MIN_MIMI_SCALE);
  });
});
