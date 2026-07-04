<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { filterLanguagePacks, partitionLanguagePacks } from '$lib/languagePacks';
  import type { LanguageInfo } from '$lib/languages';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
    reserved?: LanguageInfo[];
  };

  let {
    isRecording = false,
    onInstalledChanged
  }: {
    isRecording?: boolean;
    onInstalledChanged?: (installed: LanguageInfo[]) => void;
  } = $props();

  let payload = $state<LanguageDetectionPayload | null>(null);
  let query = $state('');
  let busyLanguage = $state<string | null>(null);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

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
  });

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
    error = null;
    try {
      applyPayload(
        await invoke<LanguageDetectionPayload>('install_language', { language: language.id })
      );
    } catch (installError) {
      error = String(installError);
    } finally {
      busyLanguage = null;
    }
  }

  async function uninstallLanguage(language: LanguageInfo) {
    busyLanguage = language.id;
    error = null;
    try {
      applyPayload(
        await invoke<LanguageDetectionPayload>('uninstall_language', { language: language.id })
      );
    } catch (uninstallError) {
      error = String(uninstallError);
    } finally {
      busyLanguage = null;
    }
  }
</script>

<div class="settings" aria-label="Settings">
  <header class="settings-head">
    <div>
      <span class="section-pill">Settings</span>
      <h1>Language packs</h1>
      <p class="settings-note">
        Speech models are downloaded to this Mac and used for on-device transcription. Removing a
        pack releases this app's copy; macOS frees the storage in the background.
      </p>
    </div>
    <div class="settings-tools">
      <input
        type="search"
        placeholder="Filter languages"
        aria-label="Filter languages"
        bind:value={query}
      />
      <button type="button" disabled={isLoading} onclick={refresh}>
        {isLoading ? 'Refreshing…' : 'Refresh'}
      </button>
    </div>
  </header>

  {#if error}
    <p class="settings-error" role="alert">{error}</p>
  {/if}

  {#if isRecording}
    <p class="settings-warning">Stop the current recording before changing language packs.</p>
  {/if}

  <section class="pack-section" aria-label="Installed language packs">
    <h2>Installed</h2>
    {#if visibleInstalled.length === 0}
      <p class="pack-empty">
        {payload ? 'No installed languages match the filter.' : 'Loading languages…'}
      </p>
    {/if}
    <ul class="pack-list">
      {#each visibleInstalled as language (language.id)}
        <li class="pack-row">
          <div class="pack-name">
            <span class="pack-label">{language.label}</span>
            <span class="pack-id">{language.id}</span>
          </div>
          {#if busyLanguage === language.id}
            <span class="pack-busy">Removing…</span>
          {:else if reservedIds.has(language.id)}
            <button
              type="button"
              class="pack-remove"
              disabled={busyLanguage !== null || isRecording}
              onclick={() => uninstallLanguage(language)}
            >
              Remove
            </button>
          {:else}
            <span
              class="pack-system"
              title="This language was installed by macOS or another app, so it cannot be removed from here."
            >
              System
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section class="pack-section" aria-label="Available language packs">
    <h2>Available to download</h2>
    {#if visibleAvailable.length === 0}
      <p class="pack-empty">
        {payload ? 'No downloadable languages match the filter.' : 'Loading languages…'}
      </p>
    {/if}
    <ul class="pack-list">
      {#each visibleAvailable as language (language.id)}
        <li class="pack-row">
          <div class="pack-name">
            <span class="pack-label">{language.label}</span>
            <span class="pack-id">{language.id}</span>
          </div>
          {#if busyLanguage === language.id}
            <span class="pack-busy">Downloading…</span>
          {:else}
            <button
              type="button"
              class="pack-add"
              disabled={busyLanguage !== null || isRecording}
              onclick={() => installLanguage(language)}
            >
              Add
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
</div>

<style>
  .settings {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 18px;
    overflow: auto;
    background: var(--canvas);
    padding: 20px 24px 40px;
  }

  .settings-head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--divider-soft);
    padding-bottom: 14px;
  }

  .settings-head h1 {
    margin: 8px 0 0;
    color: var(--ink);
    font-size: 21px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .section-pill {
    display: inline-block;
    width: fit-content;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--surface-pearl);
    color: var(--ink-muted);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
  }

  .settings-note {
    max-width: 520px;
    margin: 6px 0 0;
    color: var(--ink-muted);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .settings-tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .settings-tools input {
    width: 200px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink);
    padding: 7px 10px;
    font: inherit;
    font-size: 12.5px;
  }

  .settings-tools button {
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink-muted);
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
  }

  .settings-tools button:hover:not(:disabled) {
    border-color: rgba(0, 102, 204, 0.3);
    color: var(--apple-blue);
  }

  .settings-tools button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .settings-error {
    margin: 0;
    color: #b3261e;
    font-size: 12.5px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .settings-warning {
    width: fit-content;
    margin: 0;
    border: 1px solid rgba(0, 102, 204, 0.24);
    border-radius: 999px;
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
  }

  .pack-section h2 {
    margin: 0 0 8px;
    color: var(--ink-secondary);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .pack-empty {
    margin: 0;
    color: var(--ink-muted);
    font-size: 12.5px;
  }

  .pack-list {
    display: grid;
    max-width: 620px;
    margin: 0;
    padding: 0;
    border: 1px solid var(--hairline);
    border-radius: 11px;
    background: var(--canvas);
    list-style: none;
    overflow: hidden;
  }

  .pack-list:empty {
    display: none;
  }

  .pack-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
  }

  .pack-row + .pack-row {
    border-top: 1px solid var(--divider-soft);
  }

  .pack-name {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .pack-label {
    color: var(--ink);
    font-size: 13.5px;
    font-weight: 500;
  }

  .pack-id {
    color: var(--ink-muted);
    font-size: 11px;
  }

  .pack-add,
  .pack-remove {
    border-radius: 999px;
    padding: 5px 14px;
    font-size: 12px;
    font-weight: 600;
  }

  .pack-add {
    border: 1px solid rgba(0, 102, 204, 0.16);
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
  }

  .pack-add:hover:not(:disabled) {
    background: rgba(0, 102, 204, 0.15);
    color: var(--apple-blue-focus);
  }

  .pack-remove {
    border: 1px solid rgba(255, 59, 48, 0.3);
    background: var(--canvas);
    color: var(--apple-red);
  }

  .pack-remove:hover:not(:disabled) {
    background: rgba(255, 59, 48, 0.08);
  }

  .pack-add:disabled,
  .pack-remove:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .pack-busy {
    color: var(--ink-muted);
    font-size: 12px;
    font-weight: 600;
  }

  .pack-system {
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--surface-pearl);
    color: var(--ink-muted);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: help;
  }
</style>
