import { describe, expect, it } from 'vitest';
import { effectiveTheme } from './themePreference';

describe('effectiveTheme', () => {
  it('uses the system theme only when the preference is auto', () => {
    expect(effectiveTheme('auto', 'dark')).toBe('dark');
    expect(effectiveTheme('auto', 'light')).toBe('light');
    expect(effectiveTheme('dark', 'light')).toBe('dark');
    expect(effectiveTheme('light', 'dark')).toBe('light');
  });
});
