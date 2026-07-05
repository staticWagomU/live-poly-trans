import { describe, expect, it } from 'vitest';
import {
  formatModelSize,
  parseSpeechModelPreference,
  resolveSpeechModelSelection,
  speechModelLabel,
  speechModelPreferenceValue,
  streamEnginePayload,
  type SpeechModelInfo
} from './speechModels';

const models: SpeechModelInfo[] = [
  {
    fileName: 'ggml-large-v3-turbo.bin',
    path: '/Users/me/Library/Application Support/superwhisper/ggml-large-v3-turbo.bin',
    sizeBytes: 1_624_555_275
  }
];

describe('speechModelLabel', () => {
  it('derives the label from a ggml file name', () => {
    expect(speechModelLabel('ggml-large-v3-turbo.bin')).toBe('large-v3-turbo');
    expect(speechModelLabel('ggml-base.en.bin')).toBe('base.en');
  });

  it('keeps unfamiliar file names as-is without the extension', () => {
    expect(speechModelLabel('custom-model.bin')).toBe('custom-model');
  });
});

describe('formatModelSize', () => {
  it('formats gigabyte and megabyte sizes', () => {
    expect(formatModelSize(1_624_555_275)).toBe('1.5 GB');
    expect(formatModelSize(494 * 1024 * 1024)).toBe('494 MB');
  });
});

describe('speech model preference round-trip', () => {
  it('treats missing or builtin values as the builtin engine', () => {
    expect(parseSpeechModelPreference(null)).toEqual({ engine: 'builtin' });
    expect(parseSpeechModelPreference('')).toEqual({ engine: 'builtin' });
    expect(parseSpeechModelPreference('builtin')).toEqual({ engine: 'builtin' });
  });

  it('treats any other stored value as a whisper model path', () => {
    expect(parseSpeechModelPreference(models[0].path)).toEqual({
      engine: 'whisper',
      modelPath: models[0].path
    });
  });

  it('serializes selections back to storable strings', () => {
    expect(speechModelPreferenceValue({ engine: 'builtin' })).toBe('builtin');
    expect(
      speechModelPreferenceValue({ engine: 'whisper', modelPath: models[0].path })
    ).toBe(models[0].path);
  });
});

describe('resolveSpeechModelSelection', () => {
  it('keeps a whisper selection whose model is still available', () => {
    expect(
      resolveSpeechModelSelection({ engine: 'whisper', modelPath: models[0].path }, models, true)
    ).toEqual({ engine: 'whisper', modelPath: models[0].path });
  });

  it('falls back to builtin when the stored model path disappeared', () => {
    expect(
      resolveSpeechModelSelection({ engine: 'whisper', modelPath: '/gone.bin' }, models, true)
    ).toEqual({ engine: 'builtin' });
  });

  it('falls back to builtin when whisper-cli is not installed', () => {
    expect(
      resolveSpeechModelSelection({ engine: 'whisper', modelPath: models[0].path }, models, false)
    ).toEqual({ engine: 'builtin' });
  });
});

describe('streamEnginePayload', () => {
  it('builds the builtin payload without a whisper model', () => {
    expect(streamEnginePayload({ engine: 'builtin' })).toEqual({
      engine: 'builtin',
      whisperModel: null
    });
  });

  it('builds the whisper payload with the model path', () => {
    expect(streamEnginePayload({ engine: 'whisper', modelPath: models[0].path })).toEqual({
      engine: 'whisper',
      whisperModel: models[0].path
    });
  });
});
