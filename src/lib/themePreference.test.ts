import { describe, expect, it } from 'vitest';
import { effectiveTheme, themeDataAttribute } from './themePreference';

describe('effectiveTheme', () => {
  it('uses the system theme only when the preference is auto', () => {
    expect(effectiveTheme('auto', 'dark')).toBe('dark');
    expect(effectiveTheme('auto', 'light')).toBe('light');
    expect(effectiveTheme('dark', 'light')).toBe('dark');
    expect(effectiveTheme('light', 'dark')).toBe('light');
  });
});

describe('themeDataAttribute', () => {
  it('removes the data-theme override when the preference is auto', () => {
    expect(themeDataAttribute('auto')).toBeNull();
    expect(themeDataAttribute('light')).toBe('light');
    expect(themeDataAttribute('dark')).toBe('dark');
  });
});
