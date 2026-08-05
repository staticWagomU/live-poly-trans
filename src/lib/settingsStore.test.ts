import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getAutoStart,
  getCaptionFontFamily,
  getCaptionLineHeight,
  getGlossaryRules,
  getMarkdownAutoExport,
  getExportDirectory,
  getFileNameTemplate,
  getGlobalShortcutsEnabled,
  getHfToken,
  getHfTokenOrNull,
  getIncludeAudio,
  getKeepInMenuBar,
  getLiveSpeakerOverrides,
  getMimiInvert,
  getMimiScale,
  getOverlayFadeSeconds,
  getOverlayFontScale,
  getOverlayLineCount,
  getOverlayShowTranslation,
  getOverlayShortcut,
  getOtherSpeakerName,
  getRecordingShortcut,
  getSelfSpeakerName,
  getSpeechModel,
  getThemePreference,
  getTranscriptFontScale,
  SETTINGS_KEYS,
  setAutoStart,
  setCaptionFontFamily,
  setCaptionLineHeight,
  setGlossaryRules,
  setMarkdownAutoExport,
  setExportDirectory,
  setFileNameTemplate,
  setGlobalShortcutsEnabled,
  setHfToken,
  setIncludeAudio,
  setKeepInMenuBar,
  setMimiInvert,
  setMimiScale,
  setOverlayFadeSeconds,
  setOverlayFontScale,
  setOverlayLineCount,
  setOverlayShowTranslation,
  setOverlayShortcut,
  setOtherSpeakerName,
  setRecordingShortcut,
  setSelfSpeakerName,
  setSpeechModel,
  setThemePreference,
  setTranscriptFontScale,
  subscribeSettings
} from './settingsStore';
import { DEFAULT_MIMI_SCALE } from './mimiDisplay';
import { DEFAULT_OVERLAY_SETTINGS } from './overlaySettings';
import { DEFAULT_FILE_NAME_TEMPLATE } from './saveSettings';
import { DEFAULT_THEME_PREFERENCE } from './themePreference';
import { DEFAULT_TRANSCRIPT_FONT_SCALE } from './transcriptFontSize';
import {
  DEFAULT_CAPTION_FONT_FAMILY,
  DEFAULT_CAPTION_LINE_HEIGHT
} from './captionAppearance';

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

  it('round-trips the theme preference through its parser', () => {
    expect(getThemePreference()).toBe(DEFAULT_THEME_PREFERENCE);
    setThemePreference('dark');
    expect(localStorage.getItem(SETTINGS_KEYS.themePreference)).toBe('dark');
    expect(getThemePreference()).toBe('dark');

    localStorage.setItem(SETTINGS_KEYS.themePreference, 'solarized');
    expect(getThemePreference()).toBe(DEFAULT_THEME_PREFERENCE);
  });

  it('round-trips caption appearance preferences through their parsers', () => {
    expect(getCaptionFontFamily()).toBe(DEFAULT_CAPTION_FONT_FAMILY);
    expect(getCaptionLineHeight()).toBe(DEFAULT_CAPTION_LINE_HEIGHT);

    setCaptionFontFamily('rounded');
    setCaptionLineHeight('relaxed');
    expect(localStorage.getItem(SETTINGS_KEYS.captionFontFamily)).toBe('rounded');
    expect(localStorage.getItem(SETTINGS_KEYS.captionLineHeight)).toBe('relaxed');
    expect(getCaptionFontFamily()).toBe('rounded');
    expect(getCaptionLineHeight()).toBe('relaxed');

    localStorage.setItem(SETTINGS_KEYS.captionFontFamily, 'mono');
    localStorage.setItem(SETTINGS_KEYS.captionLineHeight, 'wide');
    expect(getCaptionFontFamily()).toBe(DEFAULT_CAPTION_FONT_FAMILY);
    expect(getCaptionLineHeight()).toBe(DEFAULT_CAPTION_LINE_HEIGHT);
  });

  it('round-trips the mimi scale through its parser', () => {
    expect(getMimiScale()).toBe(DEFAULT_MIMI_SCALE);
    setMimiScale(1.45);
    expect(getMimiScale()).toBeCloseTo(1.45);

    localStorage.setItem(SETTINGS_KEYS.mimiScale, 'junk');
    expect(getMimiScale()).toBe(DEFAULT_MIMI_SCALE);
  });

  it('round-trips overlay display settings through their parsers', () => {
    expect(getOverlayLineCount()).toBe(DEFAULT_OVERLAY_SETTINGS.lineCount);
    expect(getOverlayShowTranslation()).toBe(DEFAULT_OVERLAY_SETTINGS.showTranslation);
    expect(getOverlayFadeSeconds()).toBe(DEFAULT_OVERLAY_SETTINGS.fadeSeconds);
    expect(getOverlayFontScale()).toBe(DEFAULT_OVERLAY_SETTINGS.fontScale);

    setOverlayLineCount(3);
    setOverlayShowTranslation(false);
    setOverlayFadeSeconds(8);
    setOverlayFontScale(1.25);

    expect(getOverlayLineCount()).toBe(3);
    expect(getOverlayShowTranslation()).toBe(false);
    expect(getOverlayFadeSeconds()).toBe(8);
    expect(getOverlayFontScale()).toBe(1.25);

    localStorage.setItem(SETTINGS_KEYS.overlayLineCount, '99');
    localStorage.setItem(SETTINGS_KEYS.overlayFadeSeconds, 'junk');
    expect(getOverlayLineCount()).toBe(3);
    expect(getOverlayFadeSeconds()).toBe(DEFAULT_OVERLAY_SETTINGS.fadeSeconds);
  });

  it('round-trips global shortcut preferences', () => {
    expect(getGlobalShortcutsEnabled()).toBe(true);
    expect(getRecordingShortcut()).toBe('CommandOrControl+Alt+R');
    expect(getOverlayShortcut()).toBe('CommandOrControl+Alt+L');

    setGlobalShortcutsEnabled(false);
    setRecordingShortcut('  Alt+Shift+R  ');
    setOverlayShortcut('Alt+Shift+L');

    expect(getGlobalShortcutsEnabled()).toBe(false);
    expect(getRecordingShortcut()).toBe('Alt+Shift+R');
    expect(getOverlayShortcut()).toBe('Alt+Shift+L');

    setRecordingShortcut(' ');
    setOverlayShortcut('');
    expect(getRecordingShortcut()).toBe('CommandOrControl+Alt+R');
    expect(getOverlayShortcut()).toBe('CommandOrControl+Alt+L');
  });

  it('defaults keep-in-menu-bar to off and round-trips it', () => {
    expect(getKeepInMenuBar()).toBe(false);
    setKeepInMenuBar(true);
    expect(getKeepInMenuBar()).toBe(true);
    expect(localStorage.getItem(SETTINGS_KEYS.keepInMenuBar)).toBe('1');
    setKeepInMenuBar(false);
    expect(getKeepInMenuBar()).toBe(false);
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

  it('defaults live speaker names to empty strings when unset', () => {
    expect(getSelfSpeakerName()).toBe('');
    expect(getOtherSpeakerName()).toBe('');
  });

  it('trims live speaker names on save and removes them when blank', () => {
    setSelfSpeakerName('  田中  ');
    setOtherSpeakerName('  山田  ');
    expect(localStorage.getItem(SETTINGS_KEYS.selfSpeakerName)).toBe('田中');
    expect(localStorage.getItem(SETTINGS_KEYS.otherSpeakerName)).toBe('山田');
    expect(getSelfSpeakerName()).toBe('田中');
    expect(getOtherSpeakerName()).toBe('山田');

    setSelfSpeakerName('   ');
    setOtherSpeakerName('');
    expect(localStorage.getItem(SETTINGS_KEYS.selfSpeakerName)).toBeNull();
    expect(localStorage.getItem(SETTINGS_KEYS.otherSpeakerName)).toBeNull();
  });

  it('returns null live speaker overrides when both names are blank', () => {
    expect(getLiveSpeakerOverrides()).toBeNull();
  });

  it('builds live speaker overrides keyed by the fixed live speaker ids', () => {
    setSelfSpeakerName('田中');
    setOtherSpeakerName('山田');
    expect(getLiveSpeakerOverrides()).toEqual({
      self: { name: '田中' },
      'system-audio': { name: '山田' }
    });
  });

  it('omits blank names from the live speaker overrides', () => {
    setOtherSpeakerName('山田');
    expect(getLiveSpeakerOverrides()).toEqual({ 'system-audio': { name: '山田' } });
  });

  it('notifies subscribers when a live speaker name changes', () => {
    let notified = 0;
    subscribeSettings(SETTINGS_KEYS.selfSpeakerName, () => {
      notified += 1;
    });

    setSelfSpeakerName('田中');
    setSelfSpeakerName('');
    expect(notified).toBe(2);
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

  it('defaults glossary rules to an empty list when unset', () => {
    expect(getGlossaryRules()).toEqual([]);
  });

  it('round-trips glossary rules as JSON', () => {
    const rules = [
      { from: 'Cloud', to: 'Claude', matchType: 'text' as const, enabled: true },
      { from: '(\\d+)円', to: '¥$1', matchType: 'regex' as const, enabled: false }
    ];
    setGlossaryRules(rules);
    expect(getGlossaryRules()).toEqual(rules);
  });

  it('removes the glossary entry when set to an empty list', () => {
    setGlossaryRules([{ from: 'a', to: 'b', matchType: 'text', enabled: true }]);
    setGlossaryRules([]);
    expect(localStorage.getItem(SETTINGS_KEYS.glossary)).toBeNull();
  });

  it('returns an empty list for corrupt glossary storage', () => {
    localStorage.setItem(SETTINGS_KEYS.glossary, '{broken');
    expect(getGlossaryRules()).toEqual([]);
  });

  it('notifies subscribers when the glossary changes', () => {
    let notified = 0;
    subscribeSettings(SETTINGS_KEYS.glossary, () => {
      notified += 1;
    });

    setGlossaryRules([{ from: 'a', to: 'b', matchType: 'text', enabled: true }]);
    setGlossaryRules([]);
    expect(notified).toBe(2);
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

  it('round-trips export directory and removes it when blank', () => {
    expect(getExportDirectory()).toBe('');
    setExportDirectory('  /Users/me/Documents/Meetings  ');
    expect(getExportDirectory()).toBe('/Users/me/Documents/Meetings');

    setExportDirectory(' ');
    expect(getExportDirectory()).toBe('');
  });

  it('round-trips the file name template and falls back to the default when blank', () => {
    expect(getFileNameTemplate()).toBe(DEFAULT_FILE_NAME_TEMPLATE);
    setFileNameTemplate('{date} {title}');
    expect(getFileNameTemplate()).toBe('{date} {title}');

    setFileNameTemplate(' ');
    expect(getFileNameTemplate()).toBe(DEFAULT_FILE_NAME_TEMPLATE);
  });

  it('round-trips markdown auto export as a 1/0 flag', () => {
    expect(getMarkdownAutoExport()).toBe(false);
    setMarkdownAutoExport(true);
    expect(getMarkdownAutoExport()).toBe(true);
    setMarkdownAutoExport(false);
    expect(getMarkdownAutoExport()).toBe(false);
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
    expect(getSelfSpeakerName()).toBe('');
    expect(getOtherSpeakerName()).toBe('');
    expect(getLiveSpeakerOverrides()).toBeNull();
    expect(getGlossaryRules()).toEqual([]);
    expect(getExportDirectory()).toBe('');
    expect(getFileNameTemplate()).toBe(DEFAULT_FILE_NAME_TEMPLATE);
    expect(getMarkdownAutoExport()).toBe(false);
    expect(() => setAutoStart(false)).not.toThrow();
    expect(() =>
      setGlossaryRules([{ from: 'a', to: 'b', matchType: 'text', enabled: true }])
    ).not.toThrow();
    expect(() => setHfToken('token')).not.toThrow();
    expect(() => setSelfSpeakerName('田中')).not.toThrow();
  });
});
