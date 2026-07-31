<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { filterLanguagePacks, partitionLanguagePacks } from '$lib/languagePacks';
  import type { LanguageInfo } from '$lib/languages';
  import {
    formatModelSize,
    resolveSpeechModelSelection,
    speechModelLabel,
    speechModelPreferenceValue,
    type SpeechModelSelection,
    type SpeechModelsPayload
  } from '$lib/speechModels';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
    reserved?: LanguageInfo[];
  };

  let {
    isRecording = false,
    onInstalledChanged,
    speechModel = { engine: 'builtin' } as SpeechModelSelection,
    onSpeechModelChanged,
    autoStartEnabled = true,
    onAutoStartChange,
    includeAudioEnabled = true,
    onIncludeAudioChange
  }: {
    isRecording?: boolean;
    onInstalledChanged?: (installed: LanguageInfo[]) => void;
    speechModel?: SpeechModelSelection;
    onSpeechModelChanged?: (selection: SpeechModelSelection) => void;
    autoStartEnabled?: boolean;
    onAutoStartChange?: (enabled: boolean) => void;
    includeAudioEnabled?: boolean;
    onIncludeAudioChange?: (enabled: boolean) => void;
  } = $props();

  let pane = $state<'general' | 'model' | 'langs'>('general');
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

  onMount(() => {
    void refresh();
    void refreshModels();
    void invoke<string>('recordings_directory')
      .then((path) => (recordingsPath = path))
      .catch(() => (recordingsPath = null));
  });

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
    <button type="button" class:active={pane === 'model'} onclick={() => (pane = 'model')}>
      🧠 認識モデル
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
  input:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: 2px;
    border-radius: 8px;
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
