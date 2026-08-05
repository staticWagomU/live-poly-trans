export type ThemePreference = 'auto' | 'light' | 'dark';
export type EffectiveTheme = 'light' | 'dark';

export const DEFAULT_THEME_PREFERENCE: ThemePreference = 'auto';

export function effectiveTheme(
  preference: ThemePreference,
  systemTheme: EffectiveTheme
): EffectiveTheme {
  return preference === 'auto' ? systemTheme : preference;
}

export function parseThemePreference(value: string | null): ThemePreference {
  return value === 'light' || value === 'dark' ? value : DEFAULT_THEME_PREFERENCE;
}
