/// Single home for every persisted user preference: key constants, typed
/// get/set pairs, and in-process change subscription. Storage access is lazy
/// and guarded so the module also loads where localStorage does not exist
/// (vitest's node environment, svelte-check prerendering).
import {
  parseGlossaryRules,
  serializeGlossaryRules,
  type GlossaryRule
} from './glossary';
import { parseMimiScale } from './mimiDisplay';
import {
  parseOverlayFadeSeconds,
  parseOverlayFontScale,
  parseOverlayLineCount
} from './overlaySettings';
import { DEFAULT_FILE_NAME_TEMPLATE } from './saveSettings';
import {
  parseSpeechModelPreference,
  speechModelPreferenceValue,
  type SpeechModelSelection
} from './speechModels';
import { parseTranscriptFontScale } from './transcriptFontSize';

export const SETTINGS_KEYS = {
  autoStart: 'lpt-auto-start',
  includeAudio: 'lpt-include-audio',
  transcriptFontScale: 'lpt-transcript-font-scale',
  speechModel: 'lpt-speech-model',
  mimiScale: 'lpt-mimi-scale',
  mimiInvert: 'lpt-mimi-invert',
  hfToken: 'lpt-hf-token',
  selfSpeakerName: 'lpt-self-speaker-name',
  otherSpeakerName: 'lpt-other-speaker-name',
  glossary: 'lpt-glossary',
  exportDirectory: 'lpt-export-directory',
  fileNameTemplate: 'lpt-file-name-template',
  markdownAutoExport: 'lpt-markdown-auto-export',
  overlayLineCount: 'lpt-overlay-line-count',
  overlayShowTranslation: 'lpt-overlay-show-translation',
  overlayFadeSeconds: 'lpt-overlay-fade-seconds',
  overlayFontScale: 'lpt-overlay-font-scale',
  globalShortcutsEnabled: 'lpt-global-shortcuts-enabled',
  recordingShortcut: 'lpt-recording-shortcut',
  overlayShortcut: 'lpt-overlay-shortcut'
} as const;

export type SettingsKey = (typeof SETTINGS_KEYS)[keyof typeof SETTINGS_KEYS];

export type SettingsListener = () => void;

const listeners = new Map<SettingsKey, Set<SettingsListener>>();

/// Fires the listener after every write to `key` made through this module.
/// In-process only; changes from other windows or tabs are not observed.
export function subscribeSettings(key: SettingsKey, listener: SettingsListener): () => void {
  let keyListeners = listeners.get(key);
  if (!keyListeners) {
    keyListeners = new Set();
    listeners.set(key, keyListeners);
  }
  keyListeners.add(listener);

  return () => {
    keyListeners.delete(listener);
  };
}

function notify(key: SettingsKey) {
  listeners.get(key)?.forEach((listener) => listener());
}

function read(key: SettingsKey): string | null {
  if (typeof localStorage === 'undefined') {
    return null;
  }

  return localStorage.getItem(key);
}

function write(key: SettingsKey, value: string) {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(key, value);
  }
  notify(key);
}

function remove(key: SettingsKey) {
  if (typeof localStorage !== 'undefined') {
    localStorage.removeItem(key);
  }
  notify(key);
}

/// Write for settings whose empty state is "not set": the value is trimmed
/// and a blank result removes the key instead of storing ''.
function writeTrimmedOrRemove(key: SettingsKey, value: string) {
  const trimmed = value.trim();
  if (trimmed) {
    write(key, trimmed);
  } else {
    remove(key);
  }
}

export function getAutoStart(): boolean {
  return read(SETTINGS_KEYS.autoStart) !== '0';
}

export function setAutoStart(enabled: boolean) {
  write(SETTINGS_KEYS.autoStart, enabled ? '1' : '0');
}

export function getIncludeAudio(): boolean {
  return read(SETTINGS_KEYS.includeAudio) !== '0';
}

export function setIncludeAudio(enabled: boolean) {
  write(SETTINGS_KEYS.includeAudio, enabled ? '1' : '0');
}

export function getTranscriptFontScale(): number {
  return parseTranscriptFontScale(read(SETTINGS_KEYS.transcriptFontScale));
}

export function setTranscriptFontScale(scale: number) {
  write(SETTINGS_KEYS.transcriptFontScale, String(scale));
}

export function getSpeechModel(): SpeechModelSelection {
  return parseSpeechModelPreference(read(SETTINGS_KEYS.speechModel));
}

export function setSpeechModel(selection: SpeechModelSelection) {
  write(SETTINGS_KEYS.speechModel, speechModelPreferenceValue(selection));
}

export function getMimiScale(): number {
  return parseMimiScale(read(SETTINGS_KEYS.mimiScale));
}

export function setMimiScale(scale: number) {
  write(SETTINGS_KEYS.mimiScale, String(scale));
}

export function getMimiInvert(): boolean {
  return read(SETTINGS_KEYS.mimiInvert) === '1';
}

export function setMimiInvert(inverted: boolean) {
  write(SETTINGS_KEYS.mimiInvert, inverted ? '1' : '0');
}

export function getOverlayLineCount(): number {
  return parseOverlayLineCount(read(SETTINGS_KEYS.overlayLineCount));
}

export function setOverlayLineCount(count: number) {
  write(SETTINGS_KEYS.overlayLineCount, String(count));
}

export function getOverlayShowTranslation(): boolean {
  return read(SETTINGS_KEYS.overlayShowTranslation) !== '0';
}

export function setOverlayShowTranslation(enabled: boolean) {
  write(SETTINGS_KEYS.overlayShowTranslation, enabled ? '1' : '0');
}

export function getOverlayFadeSeconds(): number {
  return parseOverlayFadeSeconds(read(SETTINGS_KEYS.overlayFadeSeconds));
}

export function setOverlayFadeSeconds(seconds: number) {
  write(SETTINGS_KEYS.overlayFadeSeconds, String(seconds));
}

export function getOverlayFontScale(): number {
  return parseOverlayFontScale(read(SETTINGS_KEYS.overlayFontScale));
}

export function setOverlayFontScale(scale: number) {
  write(SETTINGS_KEYS.overlayFontScale, String(scale));
}

export const DEFAULT_RECORDING_SHORTCUT = 'CommandOrControl+Alt+R';
export const DEFAULT_OVERLAY_SHORTCUT = 'CommandOrControl+Alt+L';

export function getGlobalShortcutsEnabled(): boolean {
  return read(SETTINGS_KEYS.globalShortcutsEnabled) !== '0';
}

export function setGlobalShortcutsEnabled(enabled: boolean) {
  write(SETTINGS_KEYS.globalShortcutsEnabled, enabled ? '1' : '0');
}

export function getRecordingShortcut(): string {
  return read(SETTINGS_KEYS.recordingShortcut) ?? DEFAULT_RECORDING_SHORTCUT;
}

export function setRecordingShortcut(shortcut: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.recordingShortcut, shortcut);
}

export function getOverlayShortcut(): string {
  return read(SETTINGS_KEYS.overlayShortcut) ?? DEFAULT_OVERLAY_SHORTCUT;
}

export function setOverlayShortcut(shortcut: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.overlayShortcut, shortcut);
}

export function getHfToken(): string {
  return read(SETTINGS_KEYS.hfToken) ?? '';
}

/// For callers that must pass "no token" as null rather than '' (the Rust
/// side of reprocess_recording takes Option<String>).
export function getHfTokenOrNull(): string | null {
  return read(SETTINGS_KEYS.hfToken);
}

export function setHfToken(value: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.hfToken, value);
}

/// Custom display names for the two live speakers. Empty string means "not
/// customized" — the transcript keeps its original Speaker A/B labels.
export function getSelfSpeakerName(): string {
  return read(SETTINGS_KEYS.selfSpeakerName) ?? '';
}

export function setSelfSpeakerName(value: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.selfSpeakerName, value);
}

export function getOtherSpeakerName(): string {
  return read(SETTINGS_KEYS.otherSpeakerName) ?? '';
}

export function setOtherSpeakerName(value: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.otherSpeakerName, value);
}

/// Glossary rules stored as one JSON string. Corrupt storage reads as []
/// (parseGlossaryRules is tolerant) and an empty list removes the key so the
/// no-glossary state stays "not set".
export function getGlossaryRules(): GlossaryRule[] {
  return parseGlossaryRules(read(SETTINGS_KEYS.glossary));
}

export function setGlossaryRules(rules: GlossaryRule[]) {
  if (rules.length > 0) {
    write(SETTINGS_KEYS.glossary, serializeGlossaryRules(rules));
  } else {
    remove(SETTINGS_KEYS.glossary);
  }
}

export function getExportDirectory(): string {
  return read(SETTINGS_KEYS.exportDirectory) ?? '';
}

export function setExportDirectory(path: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.exportDirectory, path);
}

export function getFileNameTemplate(): string {
  return read(SETTINGS_KEYS.fileNameTemplate) ?? DEFAULT_FILE_NAME_TEMPLATE;
}

export function setFileNameTemplate(template: string) {
  writeTrimmedOrRemove(SETTINGS_KEYS.fileNameTemplate, template);
}

export function getMarkdownAutoExport(): boolean {
  return read(SETTINGS_KEYS.markdownAutoExport) === '1';
}

export function setMarkdownAutoExport(enabled: boolean) {
  write(SETTINGS_KEYS.markdownAutoExport, enabled ? '1' : '0');
}

/// Custom names for the two live speakers, shaped like a recording's
/// `speakers` map so resolveSpeakerName/resolveSpeakerLabels apply verbatim.
export type LiveSpeakerOverrides = Record<string, { name: string }>;

/// Speakers-map view of the live name settings, keyed by the helper's fixed
/// speakerIds ('self' for the mic, 'system-audio' for the other side) so
/// resolveSpeakerName can be reused verbatim. Blank names are omitted; null
/// when nothing is customized so callers can skip resolution entirely.
export function getLiveSpeakerOverrides(): LiveSpeakerOverrides | null {
  const overrides: LiveSpeakerOverrides = {};
  const selfName = getSelfSpeakerName();
  if (selfName !== '') {
    overrides.self = { name: selfName };
  }
  const otherName = getOtherSpeakerName();
  if (otherName !== '') {
    overrides['system-audio'] = { name: otherName };
  }
  return Object.keys(overrides).length > 0 ? overrides : null;
}
