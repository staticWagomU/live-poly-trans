import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getAutoStart,
  getHfToken,
  getHfTokenOrNull,
  getIncludeAudio,
  getMimiInvert,
  getMimiScale,
  getSpeechModel,
  getTranscriptFontScale,
  SETTINGS_KEYS,
  setAutoStart,
  setHfToken,
  setIncludeAudio,
  setMimiInvert,
  setMimiScale,
  setSpeechModel,
  setTranscriptFontScale,
  subscribeSettings
} from './settingsStore';
import { DEFAULT_MIMI_SCALE } from './mimiDisplay';
import { DEFAULT_TRANSCRIPT_FONT_SCALE } from './transcriptFontSize';

function createMemoryStorage(): Storage {
  const data = new Map<string, string>();
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (key: string) => data.get(key) ?? null,
    key: (index: number) => [...data.keys()][index] ?? null,
    removeItem: (key: string) => {
      data.delete(key);
    },
    setItem: (key: string, value: string) => {
      data.set(key, value);
    }
  };
}

describe('settings store', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', createMemoryStorage());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('defaults auto-start and include-audio to true when unset', () => {
    expect(getAutoStart()).toBe(true);
    expect(getIncludeAudio()).toBe(true);
  });

  it('round-trips auto-start and include-audio as 1/0 flags', () => {
    setAutoStart(false);
    setIncludeAudio(false);
    expect(localStorage.getItem(SETTINGS_KEYS.autoStart)).toBe('0');
    expect(localStorage.getItem(SETTINGS_KEYS.includeAudio)).toBe('0');
    expect(getAutoStart()).toBe(false);
    expect(getIncludeAudio()).toBe(false);

    setAutoStart(true);
    setIncludeAudio(true);
    expect(localStorage.getItem(SETTINGS_KEYS.autoStart)).toBe('1');
    expect(getAutoStart()).toBe(true);
    expect(getIncludeAudio()).toBe(true);
  });

  it('defaults mimi invert to false when unset', () => {
    expect(getMimiInvert()).toBe(false);
    setMimiInvert(true);
    expect(localStorage.getItem(SETTINGS_KEYS.mimiInvert)).toBe('1');
    expect(getMimiInvert()).toBe(true);
    setMimiInvert(false);
    expect(getMimiInvert()).toBe(false);
  });

  it('round-trips the transcript font scale through its parser', () => {
    expect(getTranscriptFontScale()).toBe(DEFAULT_TRANSCRIPT_FONT_SCALE);
    setTranscriptFontScale(1.3);
    expect(getTranscriptFontScale()).toBe(1.3);

    localStorage.setItem(SETTINGS_KEYS.transcriptFontScale, 'junk');
    expect(getTranscriptFontScale()).toBe(DEFAULT_TRANSCRIPT_FONT_SCALE);
  });

  it('round-trips the mimi scale through its parser', () => {
    expect(getMimiScale()).toBe(DEFAULT_MIMI_SCALE);
    setMimiScale(1.45);
    expect(getMimiScale()).toBeCloseTo(1.45);

    localStorage.setItem(SETTINGS_KEYS.mimiScale, 'junk');
    expect(getMimiScale()).toBe(DEFAULT_MIMI_SCALE);
  });

  it('round-trips the speech model selection', () => {
    expect(getSpeechModel()).toEqual({ engine: 'builtin' });

    setSpeechModel({ engine: 'whisper', modelPath: '/models/ggml-base.bin' });
    expect(getSpeechModel()).toEqual({ engine: 'whisper', modelPath: '/models/ggml-base.bin' });

    setSpeechModel({ engine: 'builtin' });
    expect(getSpeechModel()).toEqual({ engine: 'builtin' });
  });

  it('trims the hf token on save and removes it when blank', () => {
    setHfToken('  hf_secret  ');
    expect(localStorage.getItem(SETTINGS_KEYS.hfToken)).toBe('hf_secret');
    expect(getHfToken()).toBe('hf_secret');
    expect(getHfTokenOrNull()).toBe('hf_secret');

    setHfToken('   ');
    expect(localStorage.getItem(SETTINGS_KEYS.hfToken)).toBeNull();
  });

  it('distinguishes empty string and null for a missing hf token', () => {
    expect(getHfToken()).toBe('');
    expect(getHfTokenOrNull()).toBeNull();
  });

  it('notifies subscribers of the written key only', () => {
    const autoStartEvents: boolean[] = [];
    const hfTokenEvents: number[] = [];
    const unsubscribe = subscribeSettings(SETTINGS_KEYS.autoStart, () => {
      autoStartEvents.push(getAutoStart());
    });
    subscribeSettings(SETTINGS_KEYS.hfToken, () => {
      hfTokenEvents.push(1);
    });

    setAutoStart(false);
    expect(autoStartEvents).toEqual([false]);
    expect(hfTokenEvents).toEqual([]);

    unsubscribe();
    setAutoStart(true);
    expect(autoStartEvents).toEqual([false]);
  });

  it('notifies subscribers when a blank hf token removes the entry', () => {
    let notified = 0;
    subscribeSettings(SETTINGS_KEYS.hfToken, () => {
      notified += 1;
    });

    setHfToken('hf_secret');
    setHfToken('');
    expect(notified).toBe(2);
  });
});

describe('settings store without localStorage', () => {
  it('falls back to defaults and ignores writes', () => {
    expect(getAutoStart()).toBe(true);
    expect(getIncludeAudio()).toBe(true);
    expect(getMimiInvert()).toBe(false);
    expect(getTranscriptFontScale()).toBe(DEFAULT_TRANSCRIPT_FONT_SCALE);
    expect(getMimiScale()).toBe(DEFAULT_MIMI_SCALE);
    expect(getSpeechModel()).toEqual({ engine: 'builtin' });
    expect(getHfToken()).toBe('');
    expect(getHfTokenOrNull()).toBeNull();
    expect(() => setAutoStart(false)).not.toThrow();
    expect(() => setHfToken('token')).not.toThrow();
  });
});
