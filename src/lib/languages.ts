export type LanguageInfo = {
  id: string;
  label: string;
};

export type LanguagePair = {
  source: string;
  target: string;
};

export function chooseDefaultLanguagePair(languages: LanguageInfo[]): LanguagePair {
  const english = languages.find((language) => language.id.toLowerCase().startsWith('en'));
  const japanese = languages.find((language) => language.id.toLowerCase().startsWith('ja'));

  if (english && japanese) {
    return { source: english.id, target: japanese.id };
  }

  return {
    source: languages[0]?.id ?? 'en-US',
    target: languages[1]?.id ?? languages[0]?.id ?? 'ja-JP'
  };
}
