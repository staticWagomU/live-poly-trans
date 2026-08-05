<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open as openFileDialog, save as saveFileDialog } from '@tauri-apps/plugin-dialog';
  import { onMount, untrack } from 'svelte';
  import { importGlossaryJson, serializeGlossaryRules, type GlossaryRule } from '$lib/glossary';
  import { filterLanguagePacks, partitionLanguagePacks } from '$lib/languagePacks';
  import type { LanguageInfo } from '$lib/languages';
  import {
    PERMISSION_KINDS,
    permissionActionFor,
    permissionLabel,
    permissionStateOf,
    type PermissionKind,
    type PermissionStatus
  } from '$lib/permissions';
  import {
    isUvInstallStage,
    uvInstallStageLabel,
    uvInstallStageProgress,
    type UvInstallStage
  } from '$lib/uvInstall';
  import {
    formatModelSize,
    resolveSpeechModelSelection,
    speechModelLabel,
    speechModelPreferenceValue,
    type SpeechModelSelection,
    type SpeechModelsPayload
  } from '$lib/speechModels';
  import {
    DEFAULT_FILE_NAME_TEMPLATE,
    renderFileName
  } from '$lib/saveSettings';
  import {
    getHfToken,
    getCaptionFontFamily,
    getCaptionLineHeight,
    getGlossaryRules,
    getMarkdownAutoExport,
    getOllamaEndpoint,
    getOllamaModel,
    getExportDirectory,
    getFileNameTemplate,
    getGlobalShortcutsEnabled,
    getOverlayFadeSeconds,
    getOverlayFontScale,
    getOverlayLineCount,
    getOverlayShowTranslation,
    getKeepInMenuBar,
    getOtherSpeakerName,
    getRecordingShortcut,
    getOverlayShortcut,
    getSelfSpeakerName,
    getTranscriptFontScale,
    getTranslationEngine,
    getTranslationFallbackEnabled,
    setCaptionFontFamily,
    setCaptionLineHeight,
    setHfToken,
    getThemePreference,
    setGlossaryRules,
    setMarkdownAutoExport,
    setOllamaEndpoint,
    setOllamaModel,
    setExportDirectory,
    setFileNameTemplate,
    setGlobalShortcutsEnabled,
    setOverlayFadeSeconds,
    setOverlayFontScale,
    setOverlayLineCount,
    setOverlayShowTranslation,
    setKeepInMenuBar,
    setOtherSpeakerName,
    setRecordingShortcut,
    setOverlayShortcut,
    setSelfSpeakerName,
    setThemePreference,
    setTranscriptFontScale,
    setTranslationEngine,
    setTranslationFallbackEnabled
  } from '$lib/settingsStore';
  import {
    DEFAULT_CAPTION_FONT_FAMILY,
    DEFAULT_CAPTION_LINE_HEIGHT,
    type CaptionFontFamily,
    type CaptionLineHeight
  } from '$lib/captionAppearance';
  import type { ThemePreference } from '$lib/themePreference';
  import {
    translationEngineDescription,
    translationEngineLabel,
    translationEngineStatus,
    type TranslationEngine
  } from '$lib/translationSettings';
  import {
    DEFAULT_TRANSCRIPT_FONT_SCALE,
    TRANSCRIPT_FONT_SCALE_STEPS
  } from '$lib/transcriptFontSize';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
    reserved?: LanguageInfo[];
  };

  type SettingsPane =
    | 'general'
    | 'appearance'
    | 'privacy'
    | 'model'
    | 'translation'
    | 'langs'
    | 'glossary'
    | 'save';

  let {
    isRecording = false,
    onInstalledChanged,
    speechModel = { engine: 'builtin' } as SpeechModelSelection,
    onSpeechModelChanged,
    autoStartEnabled = true,
    onAutoStartChange,
    includeAudioEnabled = true,
    onIncludeAudioChange,
    initialPane = 'general',
    onPermissionsChanged
  }: {
    isRecording?: boolean;
    onInstalledChanged?: (installed: LanguageInfo[]) => void;
    speechModel?: SpeechModelSelection;
    onSpeechModelChanged?: (selection: SpeechModelSelection) => void;
    autoStartEnabled?: boolean;
    onAutoStartChange?: (enabled: boolean) => void;
    includeAudioEnabled?: boolean;
    onIncludeAudioChange?: (enabled: boolean) => void;
    initialPane?: SettingsPane;
    onPermissionsChanged?: (status: PermissionStatus) => void;
  } = $props();

  type WhisperxRunner = { program: string; prefixArgs?: string[] };
  const translationEngines = ['apple', 'deepl', 'ollama'] as const;

  // Seed only: which pane opens is the caller's business once (the privacy
  // banner deep-links here), and the view remounts each time Settings opens.
  let pane = $state<SettingsPane>(untrack(() => initialPane));
  let whisperxRunner = $state<WhisperxRunner | null>(null);
  let whisperxChecked = $state(false);
  let uvStage = $state<UvInstallStage | null>(null);
  let uvError = $state<string | null>(null);
  let permissionStatus = $state<PermissionStatus | null>(null);
  let permissionError = $state<string | null>(null);
  let busyPermission = $state<PermissionKind | null>(null);
  let hfToken = $state('');
  let selfSpeakerName = $state('');
  let otherSpeakerName = $state('');
  let glossaryRules = $state<GlossaryRule[]>([]);
  let glossaryFrom = $state('');
  let glossaryTo = $state('');
  let glossaryNotice = $state<string | null>(null);
  let glossaryError = $state<string | null>(null);
  let glossaryFileInput = $state<HTMLInputElement | null>(null);
  let exportDirectory = $state('');
  let fileNameTemplate = $state(DEFAULT_FILE_NAME_TEMPLATE);
  let markdownAutoExport = $state(false);
  let overlayLineCount = $state(2);
  let overlayShowTranslation = $state(true);
  let overlayFadeSeconds = $state(0);
  let overlayFontScale = $state(1);
  let globalShortcutsEnabled = $state(true);
  let recordingShortcut = $state('CommandOrControl+Alt+R');
  let overlayShortcut = $state('CommandOrControl+Alt+L');
  let keepInMenuBar = $state(false);
  let themePreference = $state<ThemePreference>('auto');
  let captionFontFamily = $state<CaptionFontFamily>(DEFAULT_CAPTION_FONT_FAMILY);
  let captionLineHeight = $state<CaptionLineHeight>(DEFAULT_CAPTION_LINE_HEIGHT);
  let transcriptFontScale = $state(DEFAULT_TRANSCRIPT_FONT_SCALE);
  let translationEngine = $state<TranslationEngine>('apple');
  let translationFallbackEnabled = $state(true);
  let ollamaEndpoint = $state('');
  let ollamaModel = $state('');
  let deeplApiKeyInput = $state('');
  let deeplApiKeyConfigured = $state(false);
  let deeplKeyBusy = $state<'save' | 'test' | null>(null);
  let translationEngineMenuOpen = $state(false);
  let translationSettingsError = $state<string | null>(null);
  let translationSettingsNotice = $state<string | null>(null);
  let saveSettingsError = $state<string | null>(null);
  let payload = $state<LanguageDetectionPayload | null>(null);
  let query = $state('');
  let busyLanguage = $state<string | null>(null);
  let busyAction = $state<'install' | 'uninstall' | null>(null);
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let modelsPayload = $state<SpeechModelsPayload | null>(null);
  let modelsError = $state<string | null>(null);
  let recordingsPath = $state<string | null>(null);

  const groups = $derived(
    payload
      ? partitionLanguagePacks(payload.installed, payload.supported)
      : { installed: [], available: [] }
  );
  const visibleInstalled = $derived(filterLanguagePacks(groups.installed, query));
  const visibleAvailable = $derived(filterLanguagePacks(groups.available, query));
  const reservedIds = $derived(new Set((payload?.reserved ?? []).map((language) => language.id)));
  const translationStatus = $derived(
    translationEngineStatus(translationEngine, ollamaEndpoint, ollamaModel)
  );
  const displayedTranslationStatus = $derived(
    translationEngine === 'deepl'
      ? {
          detail: deeplApiKeyConfigured ? 'DeepL API · APIキー保存済み' : 'DeepL API · APIキー未設定',
          badge: deeplApiKeyConfigured ? '保存済み' : '未設定',
          ok: deeplApiKeyConfigured
        }
      : translationStatus
  );
  const deeplKeyActionLabel = $derived(
    deeplApiKeyInput.trim() ? '保存' : deeplApiKeyConfigured ? '削除' : '保存'
  );
  const previewFileName = $derived(
    `${renderFileName(fileNameTemplate, {
      date: new Date(2026, 7, 5, 14, 0),
      title: '定例ミーティング',
      lang: 'ja-en'
    })}.md`
  );

  onMount(() => {
    hfToken = getHfToken();
    selfSpeakerName = getSelfSpeakerName();
    otherSpeakerName = getOtherSpeakerName();
    glossaryRules = getGlossaryRules();
    exportDirectory = getExportDirectory();
    fileNameTemplate = getFileNameTemplate();
    markdownAutoExport = getMarkdownAutoExport();
    overlayLineCount = getOverlayLineCount();
    overlayShowTranslation = getOverlayShowTranslation();
    overlayFadeSeconds = getOverlayFadeSeconds();
    overlayFontScale = getOverlayFontScale();
    globalShortcutsEnabled = getGlobalShortcutsEnabled();
    recordingShortcut = getRecordingShortcut();
    overlayShortcut = getOverlayShortcut();
    keepInMenuBar = getKeepInMenuBar();
    themePreference = getThemePreference();
    captionFontFamily = getCaptionFontFamily();
    captionLineHeight = getCaptionLineHeight();
    transcriptFontScale = getTranscriptFontScale();
    translationEngine = getTranslationEngine();
    translationFallbackEnabled = getTranslationFallbackEnabled();
    ollamaEndpoint = getOllamaEndpoint();
    ollamaModel = getOllamaModel();
    void refreshDeeplApiKeyStatus();
    void refresh();
    void refreshModels();
    void refreshPermissions();
    void invoke<string>('recordings_directory')
      .then((path) => (recordingsPath = path))
      .catch(() => (recordingsPath = null));
    void refreshWhisperxStatus();

    let unlisten: UnlistenFn | null = null;
    let disposed = false;
    void listen<string>('uv-install-progress', (event) => {
      if (isUvInstallStage(event.payload)) {
        uvStage = event.payload;
      }
    }).then((stop) => {
      if (disposed) {
        void stop();
      } else {
        unlisten = stop;
      }
    });

    return () => {
      disposed = true;
      void unlisten?.();
    };
  });

  async function refreshWhisperxStatus() {
    try {
      whisperxRunner = await invoke<WhisperxRunner | null>('whisperx_status');
    } catch {
      whisperxRunner = null;
    } finally {
      whisperxChecked = true;
    }
  }

  /// Reads the current grants without prompting, so opening Settings never
  /// pops a system dialog on its own.
  async function refreshPermissions() {
    try {
      permissionStatus = await invoke<PermissionStatus>('permission_status');
      permissionError = null;
      onPermissionsChanged?.(permissionStatus);
    } catch (statusError) {
      permissionError = String(statusError);
    }
  }

  async function askForPermission(kind: PermissionKind) {
    busyPermission = kind;
    permissionError = null;
    try {
      permissionStatus = await invoke<PermissionStatus>('request_permission', { kind });
      onPermissionsChanged?.(permissionStatus);
    } catch (requestError) {
      permissionError = String(requestError);
    } finally {
      busyPermission = null;
    }
  }

  function openPrivacySettings(kind: PermissionKind) {
    void invoke('open_privacy_settings', { kind }).catch((openError) => {
      permissionError = String(openError);
    });
  }

  /// Downloads uv into the app's own data directory. The heavy part comes
  /// later — the first WhisperX run still pulls its models — so the copy sets
  /// that expectation rather than implying this is the whole wait.
  async function prepareWhisperx() {
    uvError = null;
    uvStage = 'download';
    try {
      whisperxRunner = await invoke<WhisperxRunner>('ensure_uv');
      uvStage = 'done';
    } catch (installError) {
      uvError = String(installError);
      uvStage = null;
    }
  }

  function saveHfToken(value: string) {
    hfToken = value;
    setHfToken(value);
  }

  // The setters trim and treat blank as "back to default", so the inputs
  // re-read the stored value to show what will actually be displayed.
  function saveSelfSpeakerName(value: string) {
    setSelfSpeakerName(value);
    selfSpeakerName = getSelfSpeakerName();
  }

  function saveOtherSpeakerName(value: string) {
    setOtherSpeakerName(value);
    otherSpeakerName = getOtherSpeakerName();
  }

  function persistGlossary(rules: GlossaryRule[], notice: string | null = null) {
    glossaryRules = rules;
    setGlossaryRules(rules);
    glossaryNotice = notice;
    glossaryError = null;
  }

  function addGlossaryRule() {
    const from = glossaryFrom.trim();
    const to = glossaryTo.trim();
    if (!from || !to) {
      glossaryError = '誤認識される表記と正しい表記を両方入力してください。';
      glossaryNotice = null;
      return;
    }

    persistGlossary([...glossaryRules, { from, to, matchType: 'text', enabled: true }], '追加しました。');
    glossaryFrom = '';
    glossaryTo = '';
  }

  function updateGlossaryRule(index: number, patch: Partial<Pick<GlossaryRule, 'from' | 'to'>>) {
    const current = glossaryRules[index];
    if (!current) {
      return;
    }
    const next = glossaryRules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...patch } : rule
    );
    persistGlossary(next);
  }

  function deleteGlossaryRule(index: number) {
    persistGlossary(
      glossaryRules.filter((_, ruleIndex) => ruleIndex !== index),
      '削除しました。'
    );
  }

  async function importGlossaryFromFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) {
      return;
    }

    try {
      const result = importGlossaryJson(await file.text());
      if (!result.ok) {
        glossaryError = result.error;
        glossaryNotice = null;
        return;
      }
      persistGlossary(result.rules, `${result.rules.length}件を読み込みました。`);
    } catch (readError) {
      glossaryError = String(readError);
      glossaryNotice = null;
    }
  }

  async function exportGlossaryToFile() {
    glossaryError = null;
    try {
      const path = await saveFileDialog({
        defaultPath: 'live-poly-trans-glossary.json',
        filters: [{ name: 'JSON', extensions: ['json'] }]
      });
      if (path === null) {
        return;
      }
      await invoke('save_text_file', {
        path,
        contents: `${serializeGlossaryRules(glossaryRules)}\n`
      });
      glossaryNotice = '書き出しました。';
    } catch (exportError) {
      glossaryError = String(exportError);
      glossaryNotice = null;
    }
  }

  async function chooseExportDirectory() {
    saveSettingsError = null;
    try {
      const selected = await openFileDialog({ directory: true, multiple: false });
      if (typeof selected !== 'string') {
        return;
      }
      await invoke('validate_export_directory', { path: selected });
      setExportDirectory(selected);
      exportDirectory = getExportDirectory();
    } catch (directoryError) {
      saveSettingsError = String(directoryError);
    }
  }

  function saveFileNameTemplate(value: string) {
    setFileNameTemplate(value);
    fileNameTemplate = getFileNameTemplate();
  }

  function saveMarkdownAutoExport(enabled: boolean) {
    setMarkdownAutoExport(enabled);
    markdownAutoExport = getMarkdownAutoExport();
  }

  function saveOverlayLineCount(count: number) {
    setOverlayLineCount(count);
    overlayLineCount = getOverlayLineCount();
  }

  function saveOverlayShowTranslation(enabled: boolean) {
    setOverlayShowTranslation(enabled);
    overlayShowTranslation = getOverlayShowTranslation();
  }

  function saveOverlayFadeSeconds(seconds: number) {
    setOverlayFadeSeconds(seconds);
    overlayFadeSeconds = getOverlayFadeSeconds();
  }

  function saveOverlayFontScale(scale: number) {
    setOverlayFontScale(scale);
    overlayFontScale = getOverlayFontScale();
  }

  function saveGlobalShortcutsEnabled(enabled: boolean) {
    setGlobalShortcutsEnabled(enabled);
    globalShortcutsEnabled = getGlobalShortcutsEnabled();
  }

  function saveRecordingShortcut(shortcut: string) {
    setRecordingShortcut(shortcut);
    recordingShortcut = getRecordingShortcut();
  }

  function saveOverlayShortcut(shortcut: string) {
    setOverlayShortcut(shortcut);
    overlayShortcut = getOverlayShortcut();
  }

  function saveKeepInMenuBar(enabled: boolean) {
    setKeepInMenuBar(enabled);
    keepInMenuBar = getKeepInMenuBar();
  }

  function saveThemePreference(preference: ThemePreference) {
    setThemePreference(preference);
    themePreference = getThemePreference();
  }

  function saveCaptionFontFamily(preference: CaptionFontFamily) {
    setCaptionFontFamily(preference);
    captionFontFamily = getCaptionFontFamily();
  }

  function saveCaptionLineHeight(preference: CaptionLineHeight) {
    setCaptionLineHeight(preference);
    captionLineHeight = getCaptionLineHeight();
  }

  function saveTranscriptFontScale(scale: number) {
    setTranscriptFontScale(scale);
    transcriptFontScale = getTranscriptFontScale();
  }

  function saveTranslationEngine(engine: TranslationEngine) {
    setTranslationEngine(engine);
    translationEngine = getTranslationEngine();
    translationEngineMenuOpen = false;
  }

  function saveTranslationFallbackEnabled(enabled: boolean) {
    setTranslationFallbackEnabled(enabled);
    translationFallbackEnabled = getTranslationFallbackEnabled();
  }

  function saveOllamaEndpoint(endpoint: string) {
    setOllamaEndpoint(endpoint);
    ollamaEndpoint = getOllamaEndpoint();
  }

  function saveOllamaModel(model: string) {
    setOllamaModel(model);
    ollamaModel = getOllamaModel();
  }

  async function refreshDeeplApiKeyStatus() {
    try {
      deeplApiKeyConfigured = await invoke<boolean>('deepl_api_key_configured');
    } catch (keyStatusError) {
      translationSettingsError = String(keyStatusError);
    }
  }

  async function saveDeepLApiKey() {
    deeplKeyBusy = 'save';
    translationSettingsError = null;
    translationSettingsNotice = null;
    try {
      await invoke('set_deepl_api_key', { apiKey: deeplApiKeyInput });
      deeplApiKeyInput = '';
      await refreshDeeplApiKeyStatus();
      translationSettingsNotice = deeplApiKeyConfigured ? '保存しました。' : '削除しました。';
    } catch (keySaveError) {
      translationSettingsError = String(keySaveError);
    } finally {
      deeplKeyBusy = null;
    }
  }

  async function testDeepLTranslation() {
    deeplKeyBusy = 'test';
    translationSettingsError = null;
    translationSettingsNotice = null;
    try {
      const inlineKey = deeplApiKeyInput.trim();
      const result = await invoke<string>('test_deepl_translation', {
        apiKey: inlineKey || null
      });
      translationSettingsNotice = `接続できました: ${result}`;
    } catch (keyTestError) {
      translationSettingsError = String(keyTestError);
    } finally {
      deeplKeyBusy = null;
    }
  }

  async function refreshModels() {
    modelsError = null;
    try {
      const next = await invoke<SpeechModelsPayload>('list_speech_models');
      modelsPayload = next;

      // A stored whisper selection may have gone stale since last launch
      // (model deleted, whisper-cpp uninstalled); silently return to builtin.
      const resolved = resolveSpeechModelSelection(speechModel, next.models, next.cliAvailable);
      if (speechModelPreferenceValue(resolved) !== speechModelPreferenceValue(speechModel)) {
        onSpeechModelChanged?.(resolved);
      }
    } catch (modelsRefreshError) {
      modelsError = String(modelsRefreshError);
    }
  }

  async function refresh() {
    isLoading = true;
    error = null;
    try {
      applyPayload(await invoke<LanguageDetectionPayload>('detect_languages'));
    } catch (refreshError) {
      error = String(refreshError);
    } finally {
      isLoading = false;
    }
  }

  function applyPayload(next: LanguageDetectionPayload) {
    payload = next;
    onInstalledChanged?.(next.installed);
  }

  async function installLanguage(language: LanguageInfo) {
    busyLanguage = language.id;
    busyAction = 'install';
    error = null;
    try {
      applyPayload(
        await invoke<LanguageDetectionPayload>('install_language', { language: language.id })
      );
    } catch (installError) {
      error = String(installError);
    } finally {
      busyLanguage = null;
      busyAction = null;
    }
  }

  async function uninstallLanguage(language: LanguageInfo) {
    busyLanguage = language.id;
    busyAction = 'uninstall';
    error = null;
    try {
      applyPayload(
        await invoke<LanguageDetectionPayload>('uninstall_language', { language: language.id })
      );
    } catch (uninstallError) {
      error = String(uninstallError);
    } finally {
      busyLanguage = null;
      busyAction = null;
    }
  }

  function revealRecordings() {
    void invoke('reveal_recordings_directory').catch((revealError) => {
      error = String(revealError);
    });
  }
</script>

<div class="settings" aria-label="Settings">
  <aside class="set-side">
    <button type="button" class:active={pane === 'general'} onclick={() => (pane = 'general')}>
      ⚙︎ 一般
    </button>
    <button
      type="button"
      class:active={pane === 'appearance'}
      onclick={() => (pane = 'appearance')}
    >
      ◐ 外観
    </button>
    <button type="button" class:active={pane === 'privacy'} onclick={() => (pane = 'privacy')}>
      🔐 プライバシー
    </button>
    <button type="button" class:active={pane === 'model'} onclick={() => (pane = 'model')}>
      🧠 認識モデル
    </button>
    <button
      type="button"
      class:active={pane === 'translation'}
      onclick={() => (pane = 'translation')}
    >
      🌐 翻訳
    </button>
    <button type="button" class:active={pane === 'glossary'} onclick={() => (pane = 'glossary')}>
      📖 用語集
    </button>
    <button type="button" class:active={pane === 'save'} onclick={() => (pane = 'save')}>
      💾 保存
    </button>
    <button type="button" class:active={pane === 'langs'} onclick={() => (pane = 'langs')}>
      🌐 言語
    </button>
  </aside>

  {#if pane === 'general'}
    <div class="set-pane">
      <h2>一般</h2>
      <p class="lede">録音と文字起こしの基本設定。</p>

      <div class="set-group">
        <h3>文字起こし</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              起動時に自動で開始
              <div class="d">
                アプリを開くとすぐに文字起こしを始めます。オフの場合は一時停止状態で起動します。
              </div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={autoStartEnabled}
              role="switch"
              aria-checked={autoStartEnabled}
              aria-label="起動時に自動で開始"
              onclick={() => onAutoStartChange?.(!autoStartEnabled)}
            ></button>
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>ライブの話者名</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              自分(マイク)
              <div class="d">
                ライブ字幕とテキスト書き出しに表示される名前。空欄で既定の「Speaker A」に戻ります。
              </div>
            </div>
            <input
              class="name-input"
              type="text"
              placeholder="Speaker A"
              aria-label="自分の話者名"
              value={selfSpeakerName}
              onchange={(event) => saveSelfSpeakerName(event.currentTarget.value)}
            />
          </div>
          <div class="set-row">
            <div>
              相手(システム音声)
              <div class="d">
                Zoom などの相手側の名前。空欄で既定の「Speaker B」に戻ります。
              </div>
            </div>
            <input
              class="name-input"
              type="text"
              placeholder="Speaker B"
              aria-label="相手の話者名"
              value={otherSpeakerName}
              onchange={(event) => saveOtherSpeakerName(event.currentTarget.value)}
            />
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>録音</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              録音に音声ファイルを含める
              <div class="d">
                オフにすると「録音」は文字起こしテキストのみを保存します(音声は残しません)。
              </div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={includeAudioEnabled}
              role="switch"
              aria-checked={includeAudioEnabled}
              aria-label="録音に音声ファイルを含める"
              onclick={() => onIncludeAudioChange?.(!includeAudioEnabled)}
            ></button>
          </div>
          <div class="set-row">
            <div>
              保存先
              <div class="d">{recordingsPath ?? '…'}</div>
            </div>
            <button type="button" class="link-btn" onclick={revealRecordings}>Finderで表示</button>
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>字幕オーバーレイ</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              表示行数
              <div class="d">画面上に残す最新字幕の行数。</div>
            </div>
            <select
              class="compact-select"
              aria-label="字幕オーバーレイの表示行数"
              value={overlayLineCount}
              onchange={(event) => saveOverlayLineCount(Number(event.currentTarget.value))}
            >
              <option value="1">1行</option>
              <option value="2">2行</option>
              <option value="3">3行</option>
            </select>
          </div>
          <div class="set-row">
            <div>
              訳文を表示
              <div class="d">サブ言語の翻訳を本文の下に表示します。</div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={overlayShowTranslation}
              role="switch"
              aria-checked={overlayShowTranslation}
              aria-label="字幕オーバーレイに訳文を表示"
              onclick={() => saveOverlayShowTranslation(!overlayShowTranslation)}
            ></button>
          </div>
          <div class="set-row">
            <div>
              自動フェード
              <div class="d">0秒ならフェードしません。</div>
            </div>
            <input
              class="number-input"
              type="number"
              min="0"
              max="10"
              step="1"
              value={overlayFadeSeconds}
              aria-label="字幕オーバーレイの自動フェード秒数"
              onchange={(event) => saveOverlayFadeSeconds(Number(event.currentTarget.value))}
            />
          </div>
          <div class="set-row">
            <div>
              文字サイズ
              <div class="d">{Math.round(overlayFontScale * 100)}%</div>
            </div>
            <input
              class="range-input"
              type="range"
              min="0.7"
              max="1.8"
              step="0.05"
              value={overlayFontScale}
              aria-label="字幕オーバーレイの文字サイズ"
              oninput={(event) => saveOverlayFontScale(Number(event.currentTarget.value))}
            />
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>グローバルショートカット</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              有効
              <div class="d">他のアプリを操作中でも LivePolyTrans の操作を受け付けます。</div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={globalShortcutsEnabled}
              role="switch"
              aria-checked={globalShortcutsEnabled}
              aria-label="グローバルショートカットを有効化"
              onclick={() => saveGlobalShortcutsEnabled(!globalShortcutsEnabled)}
            ></button>
          </div>
          <div class="set-row">
            <div>
              録音開始/停止
              <div class="d">既定: CommandOrControl+Alt+R</div>
            </div>
            <input
              class="shortcut-input"
              type="text"
              value={recordingShortcut}
              aria-label="録音開始停止ショートカット"
              onchange={(event) => saveRecordingShortcut(event.currentTarget.value)}
            />
          </div>
          <div class="set-row">
            <div>
              オーバーレイ切替
              <div class="d">既定: CommandOrControl+Alt+L</div>
            </div>
            <input
              class="shortcut-input"
              type="text"
              value={overlayShortcut}
              aria-label="オーバーレイ切替ショートカット"
              onchange={(event) => saveOverlayShortcut(event.currentTarget.value)}
            />
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>メニューバー常駐</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              ウィンドウを閉じても常駐
              <div class="d">閉じる操作では終了せず、メニューバーからメイン画面を戻せます。</div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={keepInMenuBar}
              role="switch"
              aria-checked={keepInMenuBar}
              aria-label="ウィンドウを閉じてもメニューバーに常駐"
              onclick={() => saveKeepInMenuBar(!keepInMenuBar)}
            ></button>
          </div>
        </div>
      </div>
    </div>
  {:else if pane === 'appearance'}
    <div class="set-pane">
      <h2>外観</h2>
      <p class="lede">画面の見た目を環境や作業場所に合わせます。</p>

      <div class="set-group">
        <h3>テーマ</h3>
        <div class="set-card">
          <label class="set-row selectable">
            <span class="name-col">
              <input
                type="radio"
                name="theme-preference"
                checked={themePreference === 'auto'}
                onchange={() => saveThemePreference('auto')}
              />
              <span>
                自動
                <span class="d">macOS の外観設定に合わせます。</span>
              </span>
            </span>
            <span class="theme-swatch split" aria-hidden="true"></span>
          </label>
          <label class="set-row selectable">
            <span class="name-col">
              <input
                type="radio"
                name="theme-preference"
                checked={themePreference === 'light'}
                onchange={() => saveThemePreference('light')}
              />
              <span>
                ライト
                <span class="d">明るい背景で固定します。</span>
              </span>
            </span>
            <span class="theme-swatch light" aria-hidden="true"></span>
          </label>
          <label class="set-row selectable">
            <span class="name-col">
              <input
                type="radio"
                name="theme-preference"
                checked={themePreference === 'dark'}
                onchange={() => saveThemePreference('dark')}
              />
              <span>
                ダーク
                <span class="d">暗い背景で固定します。</span>
              </span>
            </span>
            <span class="theme-swatch dark" aria-hidden="true"></span>
          </label>
        </div>
      </div>

      <div class="set-group">
        <h3>字幕</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              フォント
              <div class="d">Live・オーバーレイ・対面モードに適用します。</div>
            </div>
            <select
              class="compact-select"
              aria-label="字幕フォント"
              value={captionFontFamily}
              onchange={(event) =>
                saveCaptionFontFamily(event.currentTarget.value as CaptionFontFamily)}
            >
              <option value="system">システム</option>
              <option value="rounded">丸ゴシック</option>
              <option value="serif">明朝</option>
            </select>
          </div>
          <div class="set-row">
            <div>
              行間
              <div class="d">長い字幕の読みやすさを調整します。</div>
            </div>
            <select
              class="compact-select"
              aria-label="字幕の行間"
              value={captionLineHeight}
              onchange={(event) =>
                saveCaptionLineHeight(event.currentTarget.value as CaptionLineHeight)}
            >
              <option value="compact">狭め</option>
              <option value="normal">標準</option>
              <option value="relaxed">広め</option>
            </select>
          </div>
          <div class="set-row">
            <div>
              Live 文字サイズ
              <div class="d">ショートカット ⌘+ / ⌘- と同じ設定です。</div>
            </div>
            <select
              class="compact-select"
              aria-label="Live 字幕の文字サイズ"
              value={transcriptFontScale}
              onchange={(event) => saveTranscriptFontScale(Number(event.currentTarget.value))}
            >
              {#each TRANSCRIPT_FONT_SCALE_STEPS as scale}
                <option value={scale}>{Math.round(scale * 100)}%</option>
              {/each}
            </select>
          </div>
        </div>
      </div>
    </div>
  {:else if pane === 'privacy'}
    <div class="set-pane">
      <h2>プライバシー</h2>
      <p class="lede">
        文字起こしに必要な許可を1つずつ確認できます。ここで「許可する」を押したものだけ、
        macOS の確認ダイアログが表示されます。
        <button type="button" class="link-btn" onclick={refreshPermissions}>↻ 再確認</button>
      </p>

      {#if permissionError}
        <p class="settings-error" role="alert">{permissionError}</p>
      {/if}

      <div class="set-card">
        {#each PERMISSION_KINDS as kind (kind)}
          {@const state = permissionStatus ? permissionStateOf(permissionStatus, kind) : null}
          <div class="set-row">
            <div>
              {permissionLabel(kind)}
              <div class="d">
                {#if kind === 'microphone'}
                  自分の声を文字起こしするために使います。
                {:else}
                  相手の声(Zoom などのシステム音声)を取り込むために使います。
                {/if}
                {#if state && state !== 'granted'}
                  <br />一度許可を求めたあとは、システム設定の一覧に LivePolyTrans が
                  並びます。ダイアログが出ない場合はそちらのチェックを入れてください。
                {/if}
              </div>
            </div>
            <div class="perm-action">
              <span class="tag" class:ok={state === 'granted'}>
                {#if state === null}
                  確認中…
                {:else if state === 'granted'}
                  許可済み
                {:else if state === 'denied'}
                  拒否
                {:else}
                  未確認
                {/if}
              </span>
              {#if busyPermission === kind}
                <span class="pack-busy"><span class="spinner"></span>確認中…</span>
              {:else if state && state !== 'granted'}
                {#if permissionActionFor(state) === 'request'}
                  <button
                    type="button"
                    class="link-btn"
                    disabled={busyPermission !== null}
                    onclick={() => askForPermission(kind)}
                  >
                    許可する
                  </button>
                {/if}
                <!-- Screen recording cannot report a refusal (the preflight
                     API only answers yes/no), so the way out of a denial is
                     offered on every ungranted row rather than on `denied`. -->
                <button type="button" class="link-btn" onclick={() => openPrivacySettings(kind)}>
                  システム設定
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>

      <p class="pack-empty perm-note">
        許可を取り消したいときは、システム設定 &gt; プライバシーとセキュリティ
        から同じ項目のチェックを外してください。
      </p>
    </div>
  {:else if pane === 'model'}
    <div class="set-pane">
      <h2>認識モデル</h2>
      <p class="lede">
        ライブ文字起こしと録音に使うエンジンを選びます。Whisper モデルは superwhisper
        のフォルダから検出されます。
      </p>

      {#if modelsError}
        <p class="settings-error" role="alert">{modelsError}</p>
      {/if}
      {#if isRecording}
        <p class="settings-warning">エンジンの切り替えは文字起こしを再起動します。</p>
      {/if}

      <div class="set-card">
        <label class="set-row selectable">
          <span class="name-col">
            <input
              type="radio"
              name="speech-model"
              checked={speechModel.engine === 'builtin'}
              onchange={() => onSpeechModelChanged?.({ engine: 'builtin' })}
            />
            <span>
              Apple 内蔵
              <span class="d">話しながらリアルタイムに文字が出ます。「言語」の言語パックを使用。</span>
            </span>
          </span>
          <span class="tag ok">推奨</span>
        </label>
        {#each modelsPayload?.models ?? [] as model (model.path)}
          <label class="set-row selectable">
            <span class="name-col">
              <input
                type="radio"
                name="speech-model"
                checked={speechModel.engine === 'whisper' && speechModel.modelPath === model.path}
                disabled={!modelsPayload?.cliAvailable}
                onchange={() => onSpeechModelChanged?.({ engine: 'whisper', modelPath: model.path })}
              />
              <span>
                Whisper — {speechModelLabel(model.fileName)}
                <span class="d">
                  {formatModelSize(model.sizeBytes)} · 発話の区切りごとに変換(数秒の遅れ)。精度重視。
                </span>
              </span>
            </span>
            <span class="tag">superwhisper</span>
          </label>
        {/each}
      </div>

      {#if modelsPayload && modelsPayload.models.length === 0}
        <p class="pack-empty">superwhisper のフォルダに Whisper モデルが見つかりませんでした。</p>
      {/if}
      {#if modelsPayload && !modelsPayload.cliAvailable}
        <p class="pack-empty">
          whisper-cli が未インストールのため Whisper モデルは選べません。
          <code>brew install whisper-cpp</code> でインストールできます。
        </p>
      {/if}

      <div class="set-group whisperx-group">
        <h3>WhisperX(録音後の再処理)</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              実行環境
              <div class="d">
                {#if !whisperxChecked}
                  確認中…
                {:else if whisperxRunner}
                  {whisperxRunner.program} 経由で実行します。Recordings の「再処理」から使えます。
                {:else if uvStage}
                  {uvInstallStageLabel(uvStage)}
                {:else}
                  未準備です。「準備する」を押すと実行環境(uv)を約 17MB
                  ダウンロードします。最初の再処理では WhisperX 本体とモデル(数 GB)の
                  取得も走るため、初回だけ時間がかかります。
                {/if}
              </div>
              {#if uvStage && !whisperxRunner}
                <div
                  class="uv-bar"
                  role="progressbar"
                  aria-label="WhisperX 実行環境の準備"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(uvInstallStageProgress(uvStage) * 100)}
                >
                  <span style:width={`${uvInstallStageProgress(uvStage) * 100}%`}></span>
                </div>
              {/if}
            </div>
            {#if whisperxRunner}
              <span class="tag ok">Ready</span>
            {:else if uvStage}
              <span class="pack-busy"><span class="spinner"></span>準備中…</span>
            {:else}
              <button
                type="button"
                class="link-btn"
                disabled={!whisperxChecked}
                onclick={prepareWhisperx}
              >
                準備する
              </button>
            {/if}
          </div>
          {#if uvError}
            <div class="set-row">
              <div>
                <p class="settings-error" role="alert">{uvError}</p>
                <div class="d">
                  自動取得に失敗した場合は <code>brew install uv</code> でも同じことができます。
                </div>
              </div>
              <button type="button" class="link-btn" onclick={prepareWhisperx}>再試行</button>
            </div>
          {/if}
          <div class="set-row">
            <div>
              Hugging Face トークン(話者分離用)
              <div class="d">
                話者分離(pyannote)にはトークンが必要です。未設定でも再処理は動きますが、
                話者ラベルなしになります。
              </div>
            </div>
            <input
              class="token-input"
              type="password"
              placeholder="hf_..."
              aria-label="Hugging Face トークン"
              value={hfToken}
              onchange={(event) => saveHfToken(event.currentTarget.value)}
            />
          </div>
        </div>
      </div>
    </div>
  {:else if pane === 'translation'}
    <div class="set-pane">
      <h2>翻訳</h2>
      <p class="lede">字幕の翻訳に使うエンジンを選びます。</p>

      {#if translationSettingsError}
        <p class="settings-error" role="alert">{translationSettingsError}</p>
      {/if}
      {#if translationSettingsNotice}
        <p class="settings-warning">{translationSettingsNotice}</p>
      {/if}

      <div class="set-group">
        <h3>エンジン</h3>
        <div class="set-card popover-card">
          <div class="set-row">
            <div>
              翻訳エンジン
              <div class="d">
                {translationEngineDescription(translationEngine)}
              </div>
            </div>
            <div class="translation-engine-picker">
              <button
                type="button"
                class="popup-button"
                aria-haspopup="menu"
                aria-expanded={translationEngineMenuOpen}
                onclick={() => (translationEngineMenuOpen = !translationEngineMenuOpen)}
              >
                <span>{translationEngineLabel(translationEngine)}</span>
                <span aria-hidden="true">⌄</span>
              </button>
              {#if translationEngineMenuOpen}
                <div class="engine-menu" role="menu" aria-label="翻訳エンジン">
                  {#each translationEngines as typedEngine (typedEngine)}
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={translationEngine === typedEngine}
                      class:active={translationEngine === typedEngine}
                      onclick={() => saveTranslationEngine(typedEngine)}
                    >
                      <span class="engine-check" aria-hidden="true">
                        {translationEngine === typedEngine ? '✓' : ''}
                      </span>
                      <span>
                        <span class="engine-name">{translationEngineLabel(typedEngine)}</span>
                        <span class="engine-detail">
                          {translationEngineDescription(typedEngine)}
                        </span>
                      </span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="set-row">
            <div>
              ステータス
              <div class="d">{displayedTranslationStatus.detail}</div>
            </div>
            <span class="tag" class:ok={displayedTranslationStatus.ok}>
              {displayedTranslationStatus.badge}
            </span>
          </div>
          <div class="set-row">
            <div>
              フォールバック
              <div class="d">選択中のエンジンが失敗したとき Apple 翻訳に切り替える</div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={translationFallbackEnabled}
              role="switch"
              aria-checked={translationFallbackEnabled}
              aria-label="翻訳失敗時に Apple 翻訳へフォールバック"
              onclick={() => saveTranslationFallbackEnabled(!translationFallbackEnabled)}
            ></button>
          </div>
          {#if translationEngine === 'deepl'}
            <div class="set-row">
              <div>
                DeepL API キー
                <div class="d">キーは Keychain に保存され、平文では書き込まれません。</div>
              </div>
              <div class="translation-secret-action">
                <input
                  class="token-input"
                  type="password"
                  placeholder={deeplApiKeyConfigured ? '保存済み' : '未設定'}
                  aria-label="DeepL API キー"
                  bind:value={deeplApiKeyInput}
                />
                <button
                  type="button"
                  class="link-btn"
                  disabled={deeplKeyBusy !== null || (!deeplApiKeyInput.trim() && !deeplApiKeyConfigured)}
                  onclick={testDeepLTranslation}
                >
                  {deeplKeyBusy === 'test' ? '確認中…' : '接続テスト'}
                </button>
                <button
                  type="button"
                  class="link-btn"
                  disabled={deeplKeyBusy !== null || (!deeplApiKeyInput.trim() && !deeplApiKeyConfigured)}
                  onclick={saveDeepLApiKey}
                >
                  {deeplKeyBusy === 'save' ? '保存中…' : deeplKeyActionLabel}
                </button>
              </div>
            </div>
          {/if}
          {#if translationEngine === 'ollama'}
            <div class="set-row">
              <div>
                エンドポイント
                <div class="d">Ollama のローカル HTTP API。</div>
              </div>
              <input
                class="endpoint-input"
                type="url"
                aria-label="Ollama エンドポイント"
                value={ollamaEndpoint}
                onchange={(event) => saveOllamaEndpoint(event.currentTarget.value)}
              />
            </div>
            <div class="set-row">
              <div>
                モデル
                <div class="d">翻訳に使うローカルモデル名。</div>
              </div>
              <input
                class="model-input"
                type="text"
                aria-label="Ollama モデル"
                value={ollamaModel}
                onchange={(event) => saveOllamaModel(event.currentTarget.value)}
              />
            </div>
          {/if}
        </div>
      </div>
    </div>
  {:else if pane === 'glossary'}
    <div class="set-pane">
      <h2>用語集</h2>
      <p class="lede">
        よく誤認識される固有名詞を正しい表記に置き換えます。表示と書き出しに適用され、
        元の認識結果は変更されません。
      </p>

      {#if glossaryError}
        <p class="settings-error" role="alert">{glossaryError}</p>
      {/if}
      {#if glossaryNotice}
        <p class="settings-warning">{glossaryNotice}</p>
      {/if}

      <div class="set-group">
        <h3>対応表</h3>
        <div class="set-card glossary-card">
          <div class="glossary-head" aria-hidden="true">
            <span>誤認識される表記</span>
            <span></span>
            <span>正しい表記</span>
            <span></span>
          </div>
          {#if glossaryRules.length === 0}
            <p class="glossary-empty">まだ登録されていません。</p>
          {:else}
            {#each glossaryRules as rule, index (`${index}-${rule.from}-${rule.to}`)}
              <div class="glossary-row">
                <input
                  class="glossary-input"
                  type="text"
                  aria-label={`誤認識される表記 ${index + 1}`}
                  value={rule.from}
                  onchange={(event) =>
                    updateGlossaryRule(index, { from: event.currentTarget.value.trim() })}
                />
                <span class="glossary-arrow">→</span>
                <input
                  class="glossary-input"
                  type="text"
                  aria-label={`正しい表記 ${index + 1}`}
                  value={rule.to}
                  onchange={(event) =>
                    updateGlossaryRule(index, { to: event.currentTarget.value.trim() })}
                />
                <button
                  type="button"
                  class="glossary-delete"
                  aria-label={`${rule.from} を削除`}
                  onclick={() => deleteGlossaryRule(index)}
                >
                  ×
                </button>
              </div>
            {/each}
          {/if}
          <div class="glossary-row glossary-add">
            <input
              class="glossary-input"
              type="text"
              placeholder="例: すべると"
              aria-label="追加する誤認識表記"
              bind:value={glossaryFrom}
            />
            <span class="glossary-arrow">→</span>
            <input
              class="glossary-input"
              type="text"
              placeholder="例: Svelte"
              aria-label="追加する正しい表記"
              bind:value={glossaryTo}
              onkeydown={(event) => {
                if (event.key === 'Enter') {
                  addGlossaryRule();
                }
              }}
            />
            <button type="button" class="link-btn add-rule" onclick={addGlossaryRule}>追加</button>
          </div>
        </div>
      </div>

      <div class="glossary-actions">
        <input
          bind:this={glossaryFileInput}
          class="visually-hidden"
          type="file"
          accept="application/json,.json"
          onchange={importGlossaryFromFile}
        />
        <button type="button" class="link-btn" onclick={() => glossaryFileInput?.click()}>
          読み込む…
        </button>
        <button type="button" class="link-btn" onclick={exportGlossaryToFile}>書き出す…</button>
        <span class="glossary-count">{glossaryRules.length}件 · ライブ字幕と書き出しに適用中</span>
      </div>
    </div>
  {:else if pane === 'save'}
    <div class="set-pane">
      <h2>保存</h2>
      <p class="lede">
        書き出し先とファイル名の形式を設定します。録音データ本体の保存場所は変わりません。
      </p>

      {#if saveSettingsError}
        <p class="settings-error" role="alert">{saveSettingsError}</p>
      {/if}

      <div class="set-group">
        <h3>書き出し</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              書き出し先フォルダ
              <div class="d">{exportDirectory || '未設定'}</div>
            </div>
            <button type="button" class="link-btn" onclick={chooseExportDirectory}>変更…</button>
          </div>
          <div class="set-row">
            <div>
              ファイル名テンプレート
              <div class="d">結果: <code>{previewFileName}</code></div>
            </div>
            <input
              class="template-input"
              type="text"
              aria-label="ファイル名テンプレート"
              value={fileNameTemplate}
              onchange={(event) => saveFileNameTemplate(event.currentTarget.value)}
            />
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>自動化</h3>
        <div class="set-card">
          <div class="set-row">
            <div>
              録音停止時に Markdown 議事録を自動書き出し
              <div class="d">書き出し先フォルダが設定されている場合だけ実行します。</div>
            </div>
            <button
              type="button"
              class="switch"
              class:on={markdownAutoExport}
              role="switch"
              aria-checked={markdownAutoExport}
              aria-label="録音停止時に Markdown 議事録を自動書き出し"
              onclick={() => saveMarkdownAutoExport(!markdownAutoExport)}
            ></button>
          </div>
        </div>
      </div>

      <div class="set-group">
        <h3>プレースホルダ</h3>
        <div class="set-card">
          <div class="set-row placeholders-row">
            <div>
              <code>{'{date}'}</code> 2026-08-05 · <code>{'{time}'}</code> 1400 ·
              <code>{'{title}'}</code> 録音名 · <code>{'{lang}'}</code> ja-en
            </div>
          </div>
        </div>
      </div>
    </div>
  {:else}
    <div class="set-pane">
      <h2>言語</h2>
      <p class="lede">
        オンデバイスの言語パックを管理します(Apple 内蔵エンジンで使用)。
        <button type="button" class="link-btn" disabled={isLoading} onclick={refresh}>
          {isLoading ? '再検出中…' : '↻ 再検出'}
        </button>
      </p>

      {#if error}
        <p class="settings-error" role="alert">{error}</p>
      {/if}
      {#if isRecording}
        <p class="settings-warning">言語パックの変更は文字起こしの一時停止中に行ってください。</p>
      {/if}

      <input
        class="filter"
        type="search"
        placeholder="言語を絞り込む"
        aria-label="言語を絞り込む"
        bind:value={query}
      />

      <div class="set-group">
        <h3>インストール済み</h3>
        {#if visibleInstalled.length === 0}
          <p class="pack-empty">
            {payload ? '絞り込みに一致する言語がありません。' : '言語を読み込み中…'}
          </p>
        {:else}
          <div class="set-card">
            {#each visibleInstalled as language (language.id)}
              <div class="set-row">
                <div>
                  {language.label}
                  <div class="d">{language.id}</div>
                </div>
                {#if busyLanguage === language.id}
                  <span class="pack-busy"><span class="spinner"></span>削除中…</span>
                {:else if reservedIds.has(language.id)}
                  <button
                    type="button"
                    class="link-btn danger"
                    disabled={busyLanguage !== null || isRecording}
                    onclick={() => uninstallLanguage(language)}
                  >
                    削除
                  </button>
                {:else}
                  <span class="tag ok">Ready</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="set-group">
        <h3>追加できる言語</h3>
        {#if visibleAvailable.length === 0}
          <p class="pack-empty">
            {payload ? '絞り込みに一致する言語がありません。' : '言語を読み込み中…'}
          </p>
        {:else}
          <div class="set-card">
            {#each visibleAvailable as language (language.id)}
              <div class="set-row">
                <div>
                  {language.label}
                  <div class="d">{language.id}</div>
                </div>
                {#if busyLanguage === language.id && busyAction === 'install'}
                  <span class="pack-busy"><span class="spinner"></span>ダウンロード中…</span>
                {:else}
                  <button
                    type="button"
                    class="link-btn"
                    disabled={busyLanguage !== null}
                    onclick={() => installLanguage(language)}
                  >
                    追加
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  button {
    font: inherit;
    cursor: pointer;
    color: inherit;
    background: none;
    border: 0;
  }

  button:focus-visible,
  input:focus-visible,
  select:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: 2px;
    border-radius: 8px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  .settings {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    min-height: 0;
    overflow: hidden;
    background: var(--canvas);
  }

  .set-side {
    border-right: 1px solid var(--divider);
    background: var(--parchment);
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .set-side button {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 11px;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    color: var(--ink);
  }

  .set-side button:hover {
    background: var(--hover-wash);
  }

  .set-side button.active {
    background: var(--blue);
    color: #fff;
    font-weight: 600;
  }

  .set-pane {
    overflow-y: auto;
    padding: 26px 32px;
  }

  .set-pane h2 {
    margin: 0 0 4px;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.015em;
    color: var(--ink);
  }

  .set-pane .lede {
    font-size: 13px;
    color: var(--muted);
    margin: 0 0 22px;
    line-height: 1.5;
  }

  .set-group {
    margin-bottom: 26px;
  }

  .set-group > h3 {
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    margin: 0 0 8px 4px;
  }

  .set-card {
    background: var(--parchment);
    border-radius: 12px;
    overflow: hidden;
    max-width: 640px;
  }

  .set-card.popover-card {
    position: relative;
    z-index: 1;
    overflow: visible;
  }

  .set-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 11px 16px;
    font-size: 13.5px;
    color: var(--ink);
  }

  .set-row + .set-row {
    border-top: 1px solid var(--divider);
  }

  .set-row .d {
    font-size: 12px;
    color: var(--muted);
    margin-top: 2px;
    line-height: 1.45;
    word-break: break-all;
  }

  .set-row.selectable {
    cursor: pointer;
  }

  .name-col {
    display: flex;
    align-items: center;
    gap: 11px;
  }

  .name-col input {
    accent-color: var(--blue);
    margin: 0;
    flex: 0 0 auto;
  }

  .name-col .d {
    display: block;
  }

  .theme-swatch {
    width: 34px;
    height: 22px;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    flex: 0 0 auto;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
  }

  .theme-swatch.light {
    background: #f5f5f7;
  }

  .theme-swatch.dark {
    background: #1c1c1e;
  }

  .theme-swatch.split {
    background: linear-gradient(90deg, #f5f5f7 0 50%, #1c1c1e 50% 100%);
  }

  .switch {
    width: 42px;
    height: 26px;
    border-radius: 999px;
    background: rgba(120, 120, 128, 0.28);
    position: relative;
    transition: background 0.22s ease;
    flex: 0 0 auto;
  }

  .switch::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2);
    transition: transform 0.22s cubic-bezier(0.25, 0.1, 0.25, 1);
  }

  .switch.on {
    background: var(--green);
  }

  .switch.on::after {
    transform: translateX(16px);
  }

  @media (prefers-reduced-motion: reduce) {
    .switch,
    .switch::after {
      transition: none;
    }
  }

  .tag {
    font-size: 11.5px;
    color: var(--muted);
    border: 1px solid var(--hairline);
    border-radius: 6px;
    padding: 2px 7px;
    white-space: nowrap;
  }

  .tag.ok {
    color: var(--green);
    border-color: rgba(52, 199, 89, 0.4);
  }

  .link-btn {
    color: var(--blue);
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }

  .link-btn.danger {
    color: var(--red);
  }

  .link-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .whisperx-group {
    margin-top: 26px;
  }

  .perm-action {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 0 0 auto;
  }

  .perm-note {
    margin-top: 14px;
    line-height: 1.5;
  }

  .uv-bar {
    margin-top: 8px;
    width: min(280px, 100%);
    height: 6px;
    border-radius: 999px;
    background: var(--divider);
    overflow: hidden;
  }

  .uv-bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--blue);
    transition: width 0.3s ease;
  }

  @media (prefers-reduced-motion: reduce) {
    .uv-bar span {
      transition: none;
    }
  }

  .token-input,
  .name-input,
  .endpoint-input,
  .model-input,
  .template-input,
  .shortcut-input,
  .compact-select,
  .number-input {
    width: min(220px, 40%);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink);
    padding: 6px 9px;
    font: inherit;
    font-size: 12.5px;
  }

  .number-input {
    width: 86px;
  }

  .range-input {
    width: min(220px, 42%);
    accent-color: var(--blue);
  }

  .template-input {
    width: min(240px, 42%);
    font-family: ui-monospace, 'SF Mono', 'Menlo', monospace;
  }

  .shortcut-input {
    width: min(260px, 46%);
    font-family: ui-monospace, 'SF Mono', 'Menlo', monospace;
  }

  .endpoint-input {
    width: min(260px, 46%);
    font-family: ui-monospace, 'SF Mono', 'Menlo', monospace;
  }

  .model-input {
    width: min(180px, 36%);
    font-family: ui-monospace, 'SF Mono', 'Menlo', monospace;
  }

  .translation-engine-picker {
    position: relative;
    flex: 0 0 auto;
  }

  .popup-button {
    min-width: 156px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink);
    padding: 6px 9px;
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    font-size: 12.5px;
  }

  .engine-menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    right: 0;
    width: 260px;
    max-height: calc(100vh - 180px);
    overflow-y: auto;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--canvas);
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.18);
    padding: 5px;
  }

  .engine-menu button {
    width: 100%;
    min-height: 48px;
    border-radius: 7px;
    padding: 7px 8px;
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    gap: 8px;
    text-align: left;
  }

  .engine-menu button:hover,
  .engine-menu button.active {
    background: var(--hover-wash);
  }

  .engine-check {
    color: var(--blue);
    font-size: 13px;
    line-height: 1.2;
  }

  .engine-name,
  .engine-detail {
    display: block;
    min-width: 0;
  }

  .engine-name {
    color: var(--ink);
    font-size: 13px;
    font-weight: 600;
  }

  .engine-detail {
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.35;
    white-space: normal;
  }

  .token-input:disabled {
    opacity: 0.58;
    cursor: not-allowed;
  }

  .translation-secret-action {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    min-width: min(390px, 58%);
  }

  .translation-secret-action .token-input {
    width: min(170px, 100%);
  }

  .filter {
    width: min(280px, 100%);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink);
    padding: 7px 10px;
    font: inherit;
    font-size: 12.5px;
    margin-bottom: 18px;
  }

  .glossary-card {
    max-width: 720px;
  }

  .glossary-head,
  .glossary-row {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) 24px minmax(120px, 1fr) 54px;
    gap: 10px;
    align-items: center;
  }

  .glossary-head {
    padding: 10px 14px 7px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
  }

  .glossary-row {
    padding: 8px 14px;
    border-top: 1px solid var(--divider);
  }

  .glossary-empty {
    margin: 0;
    padding: 18px 14px;
    border-top: 1px solid var(--divider);
    color: var(--muted);
    font-size: 12.5px;
  }

  .glossary-input {
    min-width: 0;
    width: 100%;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--ink);
    padding: 6px 8px;
    font: inherit;
    font-size: 13px;
  }

  .glossary-input:hover,
  .glossary-input:focus {
    border-color: var(--hairline);
    background: var(--canvas);
  }

  .glossary-arrow {
    color: var(--muted);
    text-align: center;
    font-size: 13px;
  }

  .glossary-delete {
    justify-self: end;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--red);
    opacity: 0;
  }

  .glossary-row:hover .glossary-delete,
  .glossary-delete:focus-visible {
    opacity: 1;
  }

  .glossary-add {
    background: color-mix(in srgb, var(--blue-soft) 55%, transparent);
  }

  .add-rule {
    justify-self: end;
  }

  .glossary-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 720px;
  }

  .glossary-count {
    margin-left: auto;
    color: var(--muted);
    font-size: 12px;
  }

  .placeholders-row {
    justify-content: flex-start;
    color: var(--muted);
  }

  .settings-error {
    margin: 0 0 12px;
    color: var(--red);
    font-size: 12.5px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .settings-warning {
    width: fit-content;
    margin: 0 0 12px;
    border: 1px solid rgba(0, 102, 204, 0.24);
    border-radius: 999px;
    background: var(--blue-soft);
    color: var(--blue);
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
  }

  .pack-empty {
    margin: 0;
    color: var(--muted);
    font-size: 12.5px;
  }

  .pack-busy {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--divider);
    border-top-color: var(--blue);
    animation: spin 0.8s linear infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 1.6s;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  code {
    font-size: 12px;
  }
</style>
