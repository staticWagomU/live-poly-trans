/// The language catalogue, labelled as each language labels itself — no
/// flags: a language is not a country (English is not only the US, and
/// representing Arabic with a Saudi flag is worse than lazy).
///
/// `jp` and `en` exist for the search box: a Japanese user types「スペイン」,
/// an English one types "spanish", and both should find Español.
export type Language = { code: string; name: string; jp: string; en: string };

export const LANGUAGES: Language[] = [
  { code: 'ja', name: '日本語', jp: '日本語', en: 'Japanese' },
  { code: 'en', name: 'English', jp: '英語', en: 'English' },
  { code: 'zh', name: '中文', jp: '中国語', en: 'Chinese' },
  { code: 'ko', name: '한국어', jp: '韓国語', en: 'Korean' },
  { code: 'es', name: 'Español', jp: 'スペイン語', en: 'Spanish' },
  { code: 'fr', name: 'Français', jp: 'フランス語', en: 'French' },
  { code: 'de', name: 'Deutsch', jp: 'ドイツ語', en: 'German' },
  { code: 'pt', name: 'Português', jp: 'ポルトガル語', en: 'Portuguese' },
  { code: 'it', name: 'Italiano', jp: 'イタリア語', en: 'Italian' },
  { code: 'ru', name: 'Русский', jp: 'ロシア語', en: 'Russian' },
  { code: 'id', name: 'Bahasa Indonesia', jp: 'インドネシア語', en: 'Indonesian' },
  { code: 'vi', name: 'Tiếng Việt', jp: 'ベトナム語', en: 'Vietnamese' },
  { code: 'th', name: 'ไทย', jp: 'タイ語', en: 'Thai' },
  { code: 'hi', name: 'हिन्दी', jp: 'ヒンディー語', en: 'Hindi' },
  { code: 'ar', name: 'العربية', jp: 'アラビア語', en: 'Arabic' }
];

export const labelOf = (code: string) => LANGUAGES.find((l) => l.code === code)?.name ?? code;

/// How many languages may be spoken at once. Beyond two, detection on a
/// one-second window is a guess rather than a decision (docs/step0-results.md),
/// and the backend rejects it — so the control does not offer it.
export const MAX_SPOKEN = 2;

export type Languages = { spoken: string[]; target: string | null; mutual: boolean };

/// What the pill says. The symbol carries the mode so the languages
/// themselves stay readable: none = one language, untranslated;
/// → = one way; ⇄ = mutual; · = several languages, untranslated.
export function pillText(languages: Languages): string {
  const spoken = languages.spoken.map(labelOf);
  if (!languages.target) return spoken.join(' · ');
  const target = labelOf(languages.target);
  const other = languages.spoken.find((code) => code !== languages.target);
  if (languages.mutual && other) return `${target} ⇄ ${labelOf(other)}`;
  return `${other ? labelOf(other) : spoken[0]} → ${target}`;
}

/// Mutual only means something with a second language to translate back into.
export const canBeMutual = (languages: Languages) =>
  languages.spoken.length === 2 &&
  !!languages.target &&
  languages.spoken.includes(languages.target);

export function matches(language: Language, term: string): boolean {
  const needle = term.trim().toLocaleLowerCase();
  if (!needle) return true;
  return `${language.code} ${language.name} ${language.jp} ${language.en}`
    .toLocaleLowerCase()
    .includes(needle);
}
