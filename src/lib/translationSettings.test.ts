import { describe, expect, it } from 'vitest';
import {
  DEFAULT_OLLAMA_ENDPOINT,
  DEFAULT_OLLAMA_MODEL,
  DEFAULT_TRANSLATION_ENGINE,
  normalizeOllamaEndpoint,
  normalizeOllamaModel,
  parseTranslationEngine,
  translationEngineDescription,
  translationEngineLabel
} from './translationSettings';

describe('translation settings', () => {
  it('parses supported engine ids and falls back to Apple translation', () => {
    expect(parseTranslationEngine('apple')).toBe('apple');
    expect(parseTranslationEngine('deepl')).toBe('deepl');
    expect(parseTranslationEngine('ollama')).toBe('ollama');
    expect(parseTranslationEngine('google')).toBe(DEFAULT_TRANSLATION_ENGINE);
    expect(parseTranslationEngine(null)).toBe(DEFAULT_TRANSLATION_ENGINE);
  });

  it('labels each engine for the settings pane', () => {
    expect(translationEngineLabel('apple')).toBe('Apple 翻訳');
    expect(translationEngineLabel('deepl')).toBe('DeepL API');
    expect(translationEngineLabel('ollama')).toBe('Ollama');
  });

  it('describes engine requirements without implying backend availability', () => {
    expect(translationEngineDescription('apple')).toContain('macOS 内蔵');
    expect(translationEngineDescription('deepl')).toContain('APIキー');
    expect(translationEngineDescription('ollama')).toContain('ローカルLLM');
  });

  it('normalizes blank Ollama fields to defaults', () => {
    expect(normalizeOllamaEndpoint('  http://localhost:11434  ')).toBe('http://localhost:11434');
    expect(normalizeOllamaEndpoint('')).toBe(DEFAULT_OLLAMA_ENDPOINT);
    expect(normalizeOllamaModel('  llama3.1:8b  ')).toBe('llama3.1:8b');
    expect(normalizeOllamaModel(null)).toBe(DEFAULT_OLLAMA_MODEL);
  });
});
