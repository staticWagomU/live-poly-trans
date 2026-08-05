<script lang="ts">
  import { onMount } from 'svelte';
  import { streamsForCaptureMode, type AudioStream, type CaptureMode } from '$lib/audioMode';
  import {
    audioLevelMeter,
    AUDIO_SILENCE_THRESHOLD_DB,
    AUDIO_SILENCE_WINDOW_MS,
    latestAudioLevel,
    silenceFor,
    streamSilenceState,
    type AudioLevelHistory,
    type SilenceState
  } from '$lib/audioLevels';
  import { TEXT_EXPORT_FORMATS, type TextExportFormat } from '$lib/export/saveTextExport';
  import { formatRecordingTimer } from '$lib/captureState';
  import { languageControlLabel, type LanguageInfo } from '$lib/languages';
  import {
    canDecreaseTranscriptFontScale,
    canIncreaseTranscriptFontScale,
    decreaseTranscriptFontScale,
    DEFAULT_TRANSCRIPT_FONT_SCALE,
    increaseTranscriptFontScale
  } from '$lib/transcriptFontSize';

  export let activeTab: 'live' | 'recordings' | 'settings';
  export let selectedCaptureMode: CaptureMode;
  export let isCaptureBusy: boolean;
  export let isTranscribing: boolean;
  export let mainLanguage: string;
  export let subLanguage: string;
  export let installedLanguages: LanguageInfo[];
  export let activeStreams: Set<AudioStream>;
  export let audioLevelHistory: AudioLevelHistory;
  export let isRecordingSession: boolean;
  export let recordingElapsed: number;
  export let isRecordingBusy: boolean;
  export let aiOpen: boolean;
  export let transcriptFontScale: number;
  export let onSelectCaptureMode: (mode: CaptureMode) => void;
  export let onLanguageChange: (which: 'source' | 'target', value: string) => void;
  export let onRefreshLanguages: () => void;
  export let onToggleAiPanel: () => void;
  export let onToggleRecordingSession: () => void;
  export let onCopy: () => void;
  export let onSave: () => void;
  export let onSaveAs: (format: TextExportFormat) => void;
  export let onToggleOverlay: () => void;
  export let onAdjustOverlay: () => void;

  // txt is omitted here because ファイルへ保存… (⌘S) already writes the
  // legacy plain-text format.
  const saveAsFormats: TextExportFormat[] = ['markdown', 'srt', 'vtt'];
  export let onFontScaleChange: (scale: number) => void;
  export let onEnterMimi: () => void;

  const captureModeOptions: Array<{ mode: CaptureMode; label: string }> = [
    { mode: 'speaker', label: 'Speaker' },
    { mode: 'both', label: 'Both' },
    { mode: 'mic', label: 'Mic' }
  ];

  let openMenu: 'lang' | 'more' | null = null;
  let meterNow = Date.now();

  function toggleMenu(menu: 'lang' | 'more', event: MouseEvent) {
    event.stopPropagation();
    openMenu = openMenu === menu ? null : menu;
  }

  function closeMenus() {
    openMenu = null;
  }

  function languageLabel(id: string): string {
    const language = installedLanguages.find((candidate) => candidate.id === id);
    return language ? languageControlLabel(language) : id;
  }

  function streamLabel(stream: AudioStream): string {
    return stream === 'mic' ? 'Mic' : 'System';
  }

  function meterValue(stream: AudioStream): number {
    const latest = latestAudioLevel(audioLevelHistory, stream);
    return latest ? audioLevelMeter(latest).value : 0;
  }

  function silenceState(stream: AudioStream): SilenceState {
    if (!activeStreams.has(stream)) {
      return 'unknown';
    }

    return streamSilenceState(audioLevelHistory, stream, { nowMs: meterNow });
  }

  function silenceSeconds(stream: AudioStream): number {
    return Math.floor(
      silenceFor(
        audioLevelHistory[stream],
        AUDIO_SILENCE_THRESHOLD_DB,
        AUDIO_SILENCE_WINDOW_MS,
        meterNow
      ) / 1000
    );
  }

  onMount(() => {
    const timer = setInterval(() => {
      meterNow = Date.now();
    }, 500);

    return () => clearInterval(timer);
  });

  $: languagePillLabel =
    subLanguage === ''
      ? `${languageLabel(mainLanguage)}(翻訳しない)`
      : `${languageLabel(mainLanguage)} → ${languageLabel(subLanguage)}`;
  $: languageControlsLocked = isCaptureBusy || isRecordingSession;
</script>

<svelte:window on:click={closeMenus} />

<header class="toolbar" data-tauri-drag-region>
  <div class="seg" role="tablist" aria-label="View">
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

  <div class="spacer"></div>

  {#if activeTab === 'live'}
    <div class="live-tools">
      <div class="seg" aria-label="文字起こしする音源" title="文字起こしする音源">
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

      {#if isTranscribing && selectedCaptureMode === 'both'}
        <!-- With Both, one lane can die while the status still says
             transcribing; these dots keep per-stream liveness visible. -->
        <div class="stream-health" aria-label="Capture status">
          {#each streamsForCaptureMode(selectedCaptureMode) as stream (stream)}
            <span
              class="stream-dot"
              class:dead={!activeStreams.has(stream)}
              title={activeStreams.has(stream)
                ? `${stream} capture is running`
                : `${stream} capture is down`}
            ></span>
          {/each}
        </div>
      {/if}

      {#if isTranscribing}
        <div class="level-lanes" aria-label="入力レベル">
          {#each streamsForCaptureMode(selectedCaptureMode) as stream (stream)}
            {@const state = silenceState(stream)}
            <div
              class="level-lane"
              class:inactive={!activeStreams.has(stream)}
              class:silent={state === 'silent'}
              title={state === 'silent'
                ? `${streamLabel(stream)} input is silent`
                : `${streamLabel(stream)} input level`}
            >
              <span class="level-label">{streamLabel(stream)}</span>
              <span class="level-track" aria-hidden="true">
                <span style={`transform: scaleX(${meterValue(stream)})`}></span>
              </span>
              {#if state === 'silent'}
                <span class="level-warning">無音 {silenceSeconds(stream)}s</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      <div class="menu-anchor">
        <button
          type="button"
          class="pill-btn"
          aria-haspopup="menu"
          aria-expanded={openMenu === 'lang'}
          disabled={languageControlsLocked}
          on:click={(event) => toggleMenu('lang', event)}
        >
          {languagePillLabel} <span class="chev">▾</span>
        </button>
        {#if openMenu === 'lang'}
          <nav class="menu">
            <div class="mlabel">メイン(認識する言語)</div>
            {#each installedLanguages as language (language.id)}
              <button
                type="button"
                class="mi"
                class:checked={language.id === mainLanguage}
                on:click={() => {
                  onLanguageChange('source', language.id);
                  closeMenus();
                }}
              >
                {languageControlLabel(language)}
              </button>
            {/each}
            <div class="sep"></div>
            <div class="mlabel">サブ(翻訳先)</div>
            <button
              type="button"
              class="mi"
              class:checked={subLanguage === ''}
              on:click={() => {
                onLanguageChange('target', '');
                closeMenus();
              }}
            >
              翻訳しない
            </button>
            {#each installedLanguages as language (language.id)}
              <button
                type="button"
                class="mi"
                class:checked={language.id === subLanguage}
                on:click={() => {
                  onLanguageChange('target', language.id);
                  closeMenus();
                }}
              >
                {languageControlLabel(language)}
              </button>
            {/each}
            <div class="sep"></div>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onRefreshLanguages();
                closeMenus();
              }}
            >
              ↻ 言語を再検出
            </button>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                activeTab = 'settings';
                closeMenus();
              }}
            >
              ＋ 言語を追加…
            </button>
          </nav>
        {/if}
      </div>

      <button
        type="button"
        class="pill-btn icon-only"
        class:on={aiOpen}
        title="Meeting AI パネル"
        aria-pressed={aiOpen}
        on:click={onToggleAiPanel}
      >
        ✦
      </button>

      <div class="menu-anchor">
        <button
          type="button"
          class="pill-btn icon-only"
          title="その他"
          aria-haspopup="menu"
          aria-expanded={openMenu === 'more'}
          on:click={(event) => toggleMenu('more', event)}
        >
          …
        </button>
        {#if openMenu === 'more'}
          <nav class="menu more-menu">
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onEnterMimi();
                closeMenus();
              }}
            >
              👂 対面モードへ切り替え
            </button>
            <div class="sep"></div>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onCopy();
                closeMenus();
              }}
            >
              文字起こしをコピー <span class="kbd">⇧⌘C</span>
            </button>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onSave();
                closeMenus();
              }}
            >
              ファイルへ保存… <span class="kbd">⌘S</span>
            </button>
            <div class="mlabel">形式を選んで保存</div>
            {#each saveAsFormats as format (format)}
              <button
                type="button"
                class="mi no-check"
                on:click={() => {
                  onSaveAs(format);
                  closeMenus();
                }}
              >
                {TEXT_EXPORT_FORMATS[format].menuLabel}
                <span class="kbd">.{TEXT_EXPORT_FORMATS[format].extension}</span>
              </button>
            {/each}
            <div class="sep"></div>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onToggleOverlay();
                closeMenus();
              }}
            >
              字幕オーバーレイを切り替え
            </button>
            <button
              type="button"
              class="mi no-check"
              on:click={() => {
                onAdjustOverlay();
                closeMenus();
              }}
            >
              オーバーレイの位置調整
            </button>
            <div class="sep"></div>
            <div class="mlabel">文字サイズ: {Math.round(transcriptFontScale * 100)}%</div>
            <!-- stopPropagation keeps the menu open for repeated size taps;
                 the window click handler would close it otherwise. -->
            <button
              type="button"
              class="mi no-check"
              disabled={!canIncreaseTranscriptFontScale(transcriptFontScale)}
              on:click|stopPropagation={() =>
                onFontScaleChange(increaseTranscriptFontScale(transcriptFontScale))}
            >
              大きく <span class="kbd">⌘+</span>
            </button>
            <button
              type="button"
              class="mi no-check"
              disabled={!canDecreaseTranscriptFontScale(transcriptFontScale)}
              on:click|stopPropagation={() =>
                onFontScaleChange(decreaseTranscriptFontScale(transcriptFontScale))}
            >
              小さく <span class="kbd">⌘−</span>
            </button>
            <button
              type="button"
              class="mi no-check"
              on:click|stopPropagation={() => onFontScaleChange(DEFAULT_TRANSCRIPT_FONT_SCALE)}
            >
              標準サイズ <span class="kbd">⌘0</span>
            </button>
          </nav>
        {/if}
      </div>
    </div>
  {/if}

  <button
    type="button"
    class="record-btn"
    class:rec={isRecordingSession}
    disabled={isRecordingBusy || (!isTranscribing && !isRecordingSession)}
    title="この会話を録音として Recordings に保存"
    on:click={onToggleRecordingSession}
  >
    <span class="dot"></span><span class="rec-label"
      >{isRecordingSession ? formatRecordingTimer(recordingElapsed) : '録音'}</span
    >
  </button>
</header>

<style>
  button {
    font: inherit;
    cursor: pointer;
    color: inherit;
    background: none;
    border: 0;
  }

  button:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: 2px;
    border-radius: 8px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 52px;
    padding: 8px 14px;
    background: var(--pearl);
    backdrop-filter: blur(20px) saturate(180%);
    border-bottom: 1px solid var(--divider);
    position: relative;
    z-index: 20;
  }

  .spacer {
    flex: 1;
  }

  .live-tools {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .seg {
    display: flex;
    background: var(--seg-track);
    border-radius: 9px;
    padding: 2px;
  }

  .seg button {
    padding: 5px 14px;
    border-radius: 7px;
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-2);
    transition: all 0.18s ease;
  }

  .seg button.active {
    background: var(--canvas);
    color: var(--ink);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
    font-weight: 600;
  }

  .seg button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .stream-health {
    display: flex;
    gap: 5px;
  }

  .stream-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
  }

  .stream-dot.dead {
    background: var(--red);
    animation: stream-dead-pulse 1s ease-in-out infinite;
  }

  .level-lanes {
    display: grid;
    gap: 4px;
    width: 168px;
    flex: 0 0 auto;
  }

  .level-lane {
    display: grid;
    grid-template-columns: 36px minmax(42px, 1fr) 54px;
    align-items: center;
    gap: 6px;
    min-height: 16px;
    color: var(--ink-2);
  }

  .level-lane.inactive {
    opacity: 0.45;
  }

  .level-label,
  .level-warning {
    overflow: hidden;
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .level-warning {
    color: var(--red);
    text-align: right;
  }

  .level-track {
    display: block;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: rgba(120, 120, 128, 0.18);
  }

  .level-track span {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--green), var(--blue));
    transform-origin: left center;
    transition: transform 0.08s linear;
  }

  .level-lane.silent .level-track span {
    background: var(--red);
  }

  @keyframes stream-dead-pulse {
    50% {
      opacity: 0.3;
    }
  }

  .menu-anchor {
    position: relative;
  }

  .pill-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 13px;
    border-radius: 999px;
    background: var(--canvas);
    border: 1px solid var(--hairline);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    transition: all 0.18s ease;
    white-space: nowrap;
  }

  .pill-btn:hover:not(:disabled) {
    border-color: rgba(0, 102, 204, 0.35);
  }

  .pill-btn:active:not(:disabled) {
    transform: scale(0.96);
  }

  .pill-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .pill-btn .chev {
    font-size: 9px;
    color: var(--muted);
  }

  .pill-btn.icon-only {
    padding: 6px 10px;
  }

  .pill-btn.on {
    background: var(--blue-soft);
    border-color: transparent;
    color: var(--blue);
  }

  .record-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 7px 15px;
    border-radius: 999px;
    background: var(--canvas);
    border: 1px solid var(--hairline);
    font-size: 13px;
    font-weight: 600;
    color: var(--ink);
    transition: all 0.2s ease;
  }

  .record-btn .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 3px solid var(--red);
    transition: all 0.2s ease;
  }

  .record-btn:active:not(:disabled) {
    transform: scale(0.96);
  }

  .record-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .record-btn.rec {
    background: var(--red);
    border-color: var(--red);
    color: #fff;
  }

  .record-btn.rec .dot {
    border-color: #fff;
    background: #fff;
    border-radius: 2px;
    width: 9px;
    height: 9px;
  }

  .record-btn .rec-label {
    font-variant-numeric: tabular-nums;
  }

  .menu {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    border-radius: 12px;
    background: var(--menu-bg);
    backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid var(--hairline);
    box-shadow: var(--shadow-menu);
    padding: 5px;
    min-width: 210px;
    z-index: 50;
    animation: menuIn 0.16s cubic-bezier(0.25, 0.1, 0.25, 1);
  }

  @keyframes menuIn {
    from {
      opacity: 0;
      transform: scale(0.97) translateY(-4px);
    }

    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }
  }

  .menu .mi {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    width: 100%;
    padding: 7px 11px;
    border-radius: 8px;
    font-size: 13px;
    text-align: left;
    color: var(--ink);
  }

  .menu .mi:hover:not(:disabled) {
    background: var(--blue);
    color: #fff;
  }

  .menu .mi:hover:not(:disabled) .kbd {
    color: rgba(255, 255, 255, 0.75);
  }

  .menu .mi:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .menu .mi .kbd {
    font-size: 12px;
    color: var(--muted);
  }

  .menu .sep {
    height: 1px;
    background: var(--divider);
    margin: 5px 8px;
  }

  .menu .mlabel {
    padding: 6px 11px 2px;
    font-size: 11px;
    color: var(--muted);
  }

  .menu .mi.checked::before {
    content: '✓  ';
  }

  .menu .mi:not(.checked):not(.no-check) {
    padding-left: 28px;
  }

  .menu .mi.no-check {
    padding-left: 11px;
  }
</style>
