<script lang="ts">
  import { streamsForCaptureMode, type AudioStream, type CaptureMode } from '$lib/audioMode';
  import { languageControlLabel, type LanguageInfo } from '$lib/languages';

  export let activeTab: 'live' | 'recordings' | 'settings';
  export let selectedCaptureMode: CaptureMode;
  export let isCaptureBusy: boolean;
  export let isRecording: boolean;
  export let captureTransition: 'starting' | 'stopping' | 'switching' | null;
  export let mainLanguage: string;
  export let subLanguage: string;
  export let installedLanguages: LanguageInfo[];
  export let recordingEnabled: boolean;
  export let activeStreams: Set<AudioStream>;
  export let onSelectCaptureMode: (mode: CaptureMode) => void;
  export let onLanguageChange: (which: 'source' | 'target', value: string) => void;
  export let onRefreshLanguages: () => void;
  export let onRecordingEnabledChange: (enabled: boolean) => void;
  export let onToggleRecording: () => void;

  const captureModeOptions: Array<{ mode: CaptureMode; label: string }> = [
    { mode: 'mic', label: 'Mic' },
    { mode: 'both', label: 'Both' },
    { mode: 'speaker', label: 'Speaker' }
  ];
</script>

<header class="toolbar" data-tauri-drag-region>
  <div class="toolbar-lead">
    <div class="tab-switch" role="tablist" aria-label="View">
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'live'}
        class:active={activeTab === 'live'}
        on:click={() => (activeTab = 'live')}
      >
        Live
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'recordings'}
        class:active={activeTab === 'recordings'}
        on:click={() => (activeTab = 'recordings')}
      >
        Recordings
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'settings'}
        class:active={activeTab === 'settings'}
        on:click={() => (activeTab = 'settings')}
      >
        Settings
      </button>
    </div>

    <div class="capture-switch" data-mode={selectedCaptureMode} aria-label="Audio capture mode">
      {#each captureModeOptions as option}
        <button
          type="button"
          class:active={selectedCaptureMode === option.mode}
          aria-pressed={selectedCaptureMode === option.mode}
          disabled={isCaptureBusy}
          on:click={() => onSelectCaptureMode(option.mode)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="toolbar-actions">
    <div class="language-strip" aria-label="Main and sub languages">
      <label>
        <span>Main</span>
        <select
          value={mainLanguage}
          aria-label="Main language"
          disabled={isRecording || isCaptureBusy}
          on:change={(event) => onLanguageChange('source', event.currentTarget.value)}
        >
          {#each installedLanguages as language}
            <option value={language.id}>{languageControlLabel(language)}</option>
          {/each}
        </select>
      </label>
      <span class="arrow">􀄫</span>
      <label>
        <span>Sub</span>
        <select
          value={subLanguage}
          aria-label="Sub language"
          disabled={isRecording || isCaptureBusy}
          on:change={(event) => onLanguageChange('target', event.currentTarget.value)}
        >
          {#each installedLanguages as language}
            <option value={language.id}>{languageControlLabel(language)}</option>
          {/each}
        </select>
      </label>
      <button
        type="button"
        class="refresh-languages"
        title="Refresh installed languages"
        aria-label="Refresh installed languages"
        disabled={isRecording || isCaptureBusy}
        on:click={onRefreshLanguages}
      >
        ↻
      </button>
    </div>

    <label class="record-toggle" title="Save mic and speaker audio files while transcribing">
      <input
        type="checkbox"
        checked={recordingEnabled}
        disabled={isRecording}
        on:change={(event) => onRecordingEnabledChange(event.currentTarget.checked)}
      />
      <span>Save audio</span>
    </label>

    {#if isRecording}
      <!-- Always-visible per-stream liveness: with capture mode Both, one
           lane can die while the button still reads Stop, silently losing
           half the conversation. -->
      <div class="stream-health" aria-label="Capture status">
        {#each streamsForCaptureMode(selectedCaptureMode) as stream (stream)}
          <span
            class="stream-dot"
            class:dead={!activeStreams.has(stream)}
            title={activeStreams.has(stream)
              ? `${stream} capture is running`
              : `${stream} capture is down`}
          >
            {stream === 'mic' ? 'Mic' : 'Speaker'}
          </span>
        {/each}
      </div>
    {/if}

    <button
      class="record"
      class:recording={isRecording || captureTransition === 'stopping'}
      disabled={isCaptureBusy}
      on:click={onToggleRecording}
    >
      <span></span>{captureTransition === 'starting'
        ? 'Starting'
        : captureTransition === 'stopping'
          ? 'Stopping'
          : isRecording
            ? 'Stop'
            : 'Record'}
    </button>
  </div>
</header>

<style>
  button,
  select {
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  button:focus-visible,
  select:focus-visible {
    outline: 3px solid rgba(0, 102, 204, 0.24);
    outline-offset: 2px;
  }

  .toolbar {
    display: grid;
    align-items: center;
    grid-template-columns: auto 1fr;
    gap: 12px;
    min-height: 54px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--legacy-hairline);
    background: rgba(245, 245, 247, 0.92);
    backdrop-filter: blur(18px);
  }

  .toolbar-lead {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-self: end;
  }

  .language-strip,
  .capture-switch,
  .tab-switch,
  .record,
  .record-toggle {
    border: 1px solid var(--legacy-hairline);
    background: var(--legacy-canvas);
  }

  .capture-switch,
  .tab-switch {
    position: relative;
    display: grid;
    isolation: isolate;
    overflow: hidden;
    border-radius: 11px;
    padding: 3px;
    background: #e9e9ed;
  }

  .capture-switch {
    width: 238px;
    grid-template-columns: repeat(3, 1fr);
  }

  .tab-switch {
    width: 276px;
    grid-template-columns: repeat(3, 1fr);
  }

  .capture-switch::before {
    position: absolute;
    z-index: 0;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / 3);
    border: 1px solid rgba(0, 0, 0, 0.05);
    border-radius: 8px;
    background: var(--legacy-canvas);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
    content: '';
    transform: translateX(var(--capture-pill-x, 0%));
    transition:
      transform 260ms cubic-bezier(0.22, 1, 0.36, 1),
      background 260ms ease;
  }

  .capture-switch[data-mode='both'] {
    --capture-pill-x: 100%;
  }

  .capture-switch[data-mode='speaker'] {
    --capture-pill-x: 200%;
  }

  .capture-switch button,
  .tab-switch button {
    position: relative;
    z-index: 1;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-muted);
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0;
    transition:
      color 180ms ease,
      opacity 180ms ease;
  }

  .capture-switch button:hover:not(:disabled),
  .tab-switch button:hover {
    color: var(--legacy-ink);
  }

  .capture-switch button.active,
  .tab-switch button.active {
    color: var(--legacy-ink);
  }

  .tab-switch button.active {
    background: var(--legacy-canvas);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  }

  .capture-switch button:disabled {
    cursor: wait;
    opacity: 0.58;
  }

  .language-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 11px;
    padding: 5px 9px;
  }

  .language-strip label {
    display: grid;
    gap: 1px;
  }

  .language-strip label span {
    padding-left: 1px;
    color: var(--ink-muted);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .language-strip select {
    width: 112px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--legacy-ink);
    font-size: 12px;
    font-weight: 600;
  }

  .language-strip select:disabled {
    cursor: default;
    opacity: 0.56;
  }

  .refresh-languages {
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-muted);
    padding: 4px 6px;
    font-size: 14px;
  }

  .refresh-languages:hover:not(:disabled) {
    color: var(--apple-blue);
  }

  .refresh-languages:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .arrow {
    color: var(--ink-muted);
    font-size: 13px;
  }

  .record-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    border-radius: 999px;
    color: var(--ink-muted);
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    user-select: none;
  }

  .record-toggle input {
    accent-color: var(--apple-blue);
    margin: 0;
  }

  .record-toggle input:disabled + span {
    opacity: 0.6;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 7px;
    border-radius: 999px;
    border-color: rgba(255, 59, 48, 0.35);
    color: var(--apple-red);
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 600;
  }

  .record span {
    width: 10px;
    height: 10px;
    border: 2px solid var(--apple-red);
    border-radius: 999px;
  }

  .record.recording {
    border-color: var(--apple-red);
    background: var(--apple-red);
    color: white;
  }

  .record.recording span {
    border-color: white;
    border-radius: 3px;
    background: white;
  }

  .record:disabled {
    cursor: wait;
    opacity: 0.74;
  }

  .stream-health {
    display: flex;
    gap: 8px;
  }

  .stream-dot {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--ink-muted);
    font-size: 11px;
    font-weight: 600;
  }

  .stream-dot::before {
    content: '';
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #34c759;
  }

  .stream-dot.dead {
    color: #b3261e;
  }

  .stream-dot.dead::before {
    background: #b3261e;
    animation: stream-dead-pulse 1s ease-in-out infinite;
  }

  @keyframes stream-dead-pulse {
    50% {
      opacity: 0.3;
    }
  }

  @media (max-width: 1140px) {
    .toolbar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .toolbar-actions {
      justify-self: stretch;
      justify-content: end;
      flex-wrap: wrap;
    }
  }

  @media (max-width: 900px) {
    .toolbar-lead {
      flex-wrap: wrap;
    }

    .toolbar-actions {
      display: grid;
      grid-template-columns: 1fr auto auto;
      justify-self: stretch;
    }

    .language-strip,
    .capture-switch,
    .record {
      justify-self: stretch;
    }

    .capture-switch {
      width: auto;
      flex: 1 1 auto;
    }
  }

  @media (max-width: 640px) {
    .toolbar-actions {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .language-strip {
      min-width: 0;
      overflow: hidden;
    }

    .language-strip label {
      min-width: 0;
      flex: 1 1 0;
    }

    .language-strip select {
      width: 100%;
      min-width: 0;
    }

    .record {
      width: auto;
      justify-self: end;
      white-space: nowrap;
    }
  }
</style>
