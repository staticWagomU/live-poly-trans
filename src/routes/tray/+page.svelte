<script lang="ts">
  import { emit, listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import '$lib/theme.css';
  import { emptyAudioLevelHistory } from '$lib/audioLevels';
  import { buildTrayPanelView, type TrayPanelState } from '$lib/trayPanel';

  const barIndexes = Array.from({ length: 12 }, (_, index) => index);
  let state: TrayPanelState = {
    isRecording: false,
    isTranscribing: false,
    recordingElapsedSeconds: 0,
    captureMode: 'both',
    mainLanguage: 'ja-JP',
    subLanguage: 'en-US',
    overlayVisible: false,
    audioLevelHistory: emptyAudioLevelHistory()
  };

  $: view = buildTrayPanelView(state);

  onMount(() => {
    let disposeState: (() => void) | null = null;
    void listen<TrayPanelState>('tray-panel-state', (event) => {
      state = event.payload;
    }).then((unlisten) => {
      disposeState = unlisten;
    });

    return () => {
      disposeState?.();
    };
  });

  function barScale(value: number, index: number): number {
    const wave = 0.34 + ((index * 5) % 7) / 10;
    return Math.max(0.14, Math.min(1, value * wave));
  }

  async function sendCommand(command: string) {
    await emit('tray-command', command);
    await invoke('hide_tray_panel').catch(() => undefined);
  }
</script>

<svelte:head>
  <title>LivePolyTrans Tray</title>
</svelte:head>

<main class="tray-panel" aria-label="LivePolyTrans メニューバー">
  <header class="head">
    <div class="status">{view.statusLabel}</div>
    <div class="timer" class:recording={state.isRecording}>{view.timerLabel}</div>
    <div class="context">{view.contextLabel}</div>
  </header>

  <section class="lanes" aria-label="入力レベル">
    {#each view.lanes as lane (lane.stream)}
      <div class="lane {lane.stream}" class:silent={lane.state === 'silent'} class:inactive={lane.state === 'inactive'}>
        <span class="label">
          {lane.label}
          <span>{lane.subtitle}</span>
        </span>
        <span class="eq" aria-hidden="true">
          {#each barIndexes as index}
            <i style={`transform: scaleY(${barScale(lane.meterValue, index)})`}></i>
          {/each}
        </span>
        <span class:warn={lane.state === 'silent'} class:ok={lane.state === 'active'}>
          {lane.statusLabel}
        </span>
      </div>
    {/each}
  </section>

  <nav class="menu" aria-label="操作">
    <button type="button" onclick={() => sendCommand('toggle-recording')}>
      <span class="check"></span>
      {state.isRecording ? '録音を停止' : '録音を開始'}
      <span class="key">⌥⌘R</span>
    </button>
    <button type="button" onclick={() => sendCommand('toggle-pause')}>
      <span class="check"></span>
      {state.isTranscribing ? '一時停止' : '再開'}
    </button>
    <div class="sep"></div>
    <button type="button" onclick={() => sendCommand('toggle-overlay')}>
      <span class="check">{view.overlayChecked ? '✓' : ''}</span>
      字幕オーバーレイ
      <span class="key">⌥⌘L</span>
    </button>
    <button type="button" onclick={() => sendCommand('cycle-capture-mode')}>
      <span class="check"></span>
      音源: {view.contextLabel.split(' · ')[1] ?? 'Both'}
      <span class="sub">変更</span>
    </button>
    <div class="sep"></div>
    <button type="button" onclick={() => sendCommand('show-main')}>
      <span class="check"></span>
      LivePolyTrans を開く
    </button>
    <button type="button" onclick={() => sendCommand('open-settings')}>
      <span class="check"></span>
      設定…
      <span class="key">⌘,</span>
    </button>
  </nav>
</main>

<style>
  :global(html),
  :global(body) {
    width: 100%;
    min-height: 100%;
    margin: 0;
    overflow: hidden;
    background: transparent;
  }

  .tray-panel {
    width: 280px;
    min-height: 100vh;
    padding: 8px;
    color: var(--ink);
    background: var(--menu-bg);
    backdrop-filter: blur(34px) saturate(190%);
    border: 1px solid var(--hairline);
    border-radius: 12px;
    box-shadow: var(--shadow-menu), inset 0 0 0 0.5px rgba(255, 255, 255, 0.18);
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Hiragino Sans', sans-serif;
  }

  button {
    font: inherit;
    cursor: pointer;
    color: inherit;
    background: none;
    border: 0;
  }

  button:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: -2px;
  }

  .head {
    text-align: center;
    padding: 10px 0 8px;
  }

  .status {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.09em;
    color: var(--muted);
    text-transform: uppercase;
  }

  .timer {
    font-size: 30px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    line-height: 1.2;
    color: var(--ink);
  }

  .timer.recording::before {
    content: '●';
    color: var(--red);
    font-size: 10px;
    margin-right: 7px;
    vertical-align: 6px;
  }

  .context {
    font-size: 11.5px;
    color: var(--muted);
  }

  .lanes {
    background: rgba(120, 120, 128, 0.09);
    border-radius: 11px;
    padding: 3px;
    margin: 0 2px 6px;
  }

  .lane {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr) 54px;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 8px 10px;
    border-radius: 9px;
  }

  .lane + .lane {
    border-top: 1px solid var(--divider);
  }

  .lane.silent {
    background: rgba(255, 149, 0, 0.09);
  }

  .label {
    display: grid;
    gap: 1px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink);
  }

  .label span {
    font-size: 9.5px;
    font-weight: 500;
    color: var(--muted);
  }

  .eq {
    display: flex;
    align-items: center;
    gap: 2.5px;
    height: 22px;
  }

  .eq i {
    flex: 1;
    min-width: 3px;
    height: 100%;
    border-radius: 2px;
    transform-origin: center;
  }

  .mic .eq i {
    background: var(--blue);
    opacity: 0.85;
  }

  .speaker .eq i {
    background: var(--green);
    opacity: 0.85;
  }

  .silent .eq i,
  .inactive .eq i {
    background: var(--muted);
    opacity: 0.6;
  }

  .ok,
  .warn,
  .lane > span:last-child {
    font-size: 10px;
    font-weight: 700;
    text-align: right;
    white-space: nowrap;
    color: var(--muted);
  }

  .ok {
    color: var(--green);
  }

  .warn {
    color: #c77700;
  }

  :global(:root[data-theme='dark']) .warn {
    color: #ffb340;
  }

  .menu {
    display: grid;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    min-height: 28px;
    padding: 6px 10px;
    border-radius: 7px;
    font-size: 13px;
    text-align: left;
  }

  .menu button:hover {
    background: var(--blue);
    color: #fff;
  }

  .check {
    width: 12px;
    flex: 0 0 auto;
    font-size: 11px;
  }

  .key,
  .sub {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--muted);
  }

  .menu button:hover .key,
  .menu button:hover .sub {
    color: rgba(255, 255, 255, 0.75);
  }

  .sep {
    height: 1px;
    background: var(--divider);
    margin: 5px 10px;
  }

  @media (prefers-reduced-motion: reduce) {
    .eq i {
      transition: none;
    }
  }
</style>
