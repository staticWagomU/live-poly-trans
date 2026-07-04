export type LanguageInfo = {
  id: string;
  label: string;
};

export type LanguagePair = {
  source: string;
  target: string;
};

export function chooseDefaultLanguagePair(
  languages: LanguageInfo[],
  preferredLanguage?: string
): LanguagePair {
  const english = languages.find((language) => language.id.toLowerCase().startsWith('en'));
  const japanese = languages.find((language) => language.id.toLowerCase().startsWith('ja'));

  const preferredPrimary = (preferredLanguage ?? '').split('-')[0]?.toLowerCase() ?? '';
  const preferred = preferredPrimary
    ? languages.find((language) => language.id.toLowerCase().startsWith(preferredPrimary))
    : undefined;

  if (preferred) {
    const partner = [japanese, english, ...languages].find(
      (language) =>
        language && language.id.split('-')[0]?.toLowerCase() !== preferredPrimary
    );

    if (partner) {
      return { source: preferred.id, target: partner.id };
    }
  }

  if (english && japanese) {
    return { source: english.id, target: japanese.id };
  }

  return {
    source: languages[0]?.id ?? 'en-US',
    target: languages[1]?.id ?? languages[0]?.id ?? 'ja-JP'
  };
}

export function languageControlLabel(language: LanguageInfo): string {
  const baseName = language.label.replace(/\s*\(.+\)\s*$/, '');
  const region = language.id.split('-')[1];

  return region ? `${baseName} ${region.toUpperCase()}` : baseName;
}

export function transcriptionCandidateLanguages(mainLanguage: string, subLanguage: string) {
  return [mainLanguage, subLanguage].filter(
    (language, index, languages) => language && languages.indexOf(language) === index
  );
}
