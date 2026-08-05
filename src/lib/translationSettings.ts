export type TranslationEngine = 'apple' | 'deepl' | 'ollama';

export const DEFAULT_TRANSLATION_ENGINE: TranslationEngine = 'apple';
export const DEFAULT_OLLAMA_ENDPOINT = 'http://127.0.0.1:11434';
export const DEFAULT_OLLAMA_MODEL = 'llama3.1';

const TRANSLATION_ENGINES = ['apple', 'deepl', 'ollama'] as const;

export function parseTranslationEngine(raw: string | null): TranslationEngine {
  return (
    TRANSLATION_ENGINES.find((candidate) => candidate === raw) ?? DEFAULT_TRANSLATION_ENGINE
  );
}

export function translationEngineLabel(engine: TranslationEngine): string {
  if (engine === 'deepl') {
    return 'DeepL API';
  }

  if (engine === 'ollama') {
    return 'Ollama';
  }

  return 'Apple 翻訳';
}

export function translationEngineDescription(engine: TranslationEngine): string {
  if (engine === 'deepl') {
    return '高品質 · APIキーが必要';
  }

  if (engine === 'ollama') {
    return 'ローカルLLM · Apple Intelligence 不要';
  }

  return 'オフライン・無料・macOS 内蔵';
}

export function normalizeOllamaEndpoint(raw: string | null): string {
  const trimmed = raw?.trim() ?? '';
  return trimmed || DEFAULT_OLLAMA_ENDPOINT;
}

export function normalizeOllamaModel(raw: string | null): string {
  const trimmed = raw?.trim() ?? '';
  return trimmed || DEFAULT_OLLAMA_MODEL;
}
