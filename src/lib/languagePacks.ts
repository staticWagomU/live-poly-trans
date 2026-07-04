import type { LanguageInfo } from '$lib/languages';

export type LanguagePackGroups = {
  installed: LanguageInfo[];
  available: LanguageInfo[];
};

export function partitionLanguagePacks(
  installed: LanguageInfo[],
  supported: LanguageInfo[]
): LanguagePackGroups {
  const installedIds = new Set(installed.map((language) => language.id));
  const byLabel = (left: LanguageInfo, right: LanguageInfo) =>
    left.label.localeCompare(right.label);

  return {
    installed: [...installed].sort(byLabel),
    available: supported.filter((language) => !installedIds.has(language.id)).sort(byLabel)
  };
}

export function filterLanguagePacks(languages: LanguageInfo[], query: string): LanguageInfo[] {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return languages;
  }

  return languages.filter(
    (language) =>
      language.label.toLowerCase().includes(needle) || language.id.toLowerCase().includes(needle)
  );
}
