<script lang="ts">
  import { formatRecordingTimer } from '$lib/captureState';
  import { isScrolledToBottom } from '$lib/scroll';
  import { resolveSpeakerName } from '$lib/speakers';
  import { displayTranscriptMessage } from '$lib/transcriptDisplay';
  import { isRecordingMarker, windowThreadItems, type ThreadItem } from '$lib/transcripts';
  import type { LiveSpeakerOverrides } from '$lib/settingsStore';
  import type { SpeechModelSelection } from '$lib/speechModels';

  export let threadItems: ThreadItem[];
  export let hasFinalMessages: boolean;
  export let mainLanguage: string;
  export let subLanguage: string;
  export let transcriptFontScale: number;
  export let statusMessage: string | null;
  export let actionNotice: string | null;
  export let isTranscribing: boolean;
  export let isStarting: boolean;
  export let isCaptureBusy: boolean;
  export let captureModeLabel: string;
  export let speechModel: SpeechModelSelection;
  export let confirmingClear: boolean;
  // Custom names from settings for the two live speakers; resolution stays
  // at display time so the underlying messages keep their original labels.
  export let speakerOverrides: LiveSpeakerOverrides | null = null;
  export let onTogglePause: () => void;
  export let onCopy: () => void;
  export let onSave: () => void;
  export let onClear: () => void;

  // Always-on transcription grows without bound; only the newest slice
  // stays in the DOM. "Show earlier" widens the window.
  const defaultWindowLimit = 150;
  const windowStep = 150;
  let windowLimit = defaultWindowLimit;

  $: windowed = windowThreadItems(threadItems, windowLimit);
  $: if (threadItems.length === 0 && windowLimit !== defaultWindowLimit) {
    windowLimit = defaultWindowLimit;
  }

  let messagesContainer: HTMLDivElement | null = null;
  let latestMessageAnchor: HTMLDivElement | null = null;
  let showJumpToLatest = false;

  export function shouldStickToLatest(): boolean {
    if (!messagesContainer) {
      return true;
    }

    return isScrolledToBottom({
      scrollTop: messagesContainer.scrollTop,
      clientHeight: messagesContainer.clientHeight,
      scrollHeight: messagesContainer.scrollHeight
    });
  }

  export function syncJumpToLatestButton() {
    showJumpToLatest = !!messagesContainer && !shouldStickToLatest();
  }

  export function scrollToLatest(behavior: ScrollBehavior = 'smooth') {
    if (!messagesContainer) {
      return;
    }

    if (latestMessageAnchor) {
      latestMessageAnchor.scrollIntoView({
        block: 'end',
        behavior
      });
    } else {
      messagesContainer.scrollTo({
        top: messagesContainer.scrollHeight,
        behavior
      });
    }

    showJumpToLatest = false;
  }

  function handleMessagesScroll() {
    syncJumpToLatestButton();
  }

  function markerTime(timestamp: string): string {
    const parsed = new Date(timestamp);
    if (Number.isNaN(parsed.getTime())) {
      return timestamp;
    }
    return parsed.toLocaleTimeString('ja-JP', { hour: '2-digit', minute: '2-digit' });
  }
</script>

<div class="captions-wrap" style="--fs: {transcriptFontScale}">
  <div
    class="captions"
    bind:this={messagesContainer}
    aria-live="polite"
    on:scroll={handleMessagesScroll}
  >
    {#if threadItems.length === 0}
      {#if isTranscribing}
        <div class="empty listening" aria-live="polite">
          <div class="ring"><span></span></div>
          <h2>{isStarting ? '音声の取り込みを準備中…' : '音声を待っています'}</h2>
          <p>
            {speechModel.engine === 'whisper'
              ? '話し終えた区切りごとに Whisper が文字にします。最初のテキストは一呼吸おいて表示されます。'
              : 'アプリを開くと自動で文字起こしが始まります。メイン言語の本文の下にサブ言語の訳文が流れます。残したい会話は「録音」を押すと Recordings に保存されます。'}
          </p>
        </div>
      {:else}
        <div class="empty paused" aria-live="polite">
          <div class="ring"><span>⏸</span></div>
          <h2>一時停止中</h2>
          <p>下の「再開」を押すと文字起こしを再開します。</p>
        </div>
      {/if}
    {:else}
      {#if windowed.hiddenCount > 0}
        <button
          type="button"
          class="show-earlier"
          on:click={() => (windowLimit += windowStep)}
        >
          以前の字幕を表示({windowed.hiddenCount}件)
        </button>
      {/if}

      {#each windowed.items as item (item.id)}
        {#if isRecordingMarker(item)}
          <div class="rec-marker" class:end={item.phase === 'stop'}>
            {item.phase === 'start'
              ? `⏺ 録音開始 ${markerTime(item.timestamp)}`
              : `⏹ 録音終了 ${markerTime(item.timestamp)} · ${formatRecordingTimer(item.durationSeconds ?? 0)}`}
          </div>
        {:else}
          {@const transcriptDisplay = displayTranscriptMessage(item, mainLanguage, subLanguage)}
          <div
            class="cap"
            class:mic={item.role === 'self'}
            class:interim={!item.isFinal}
            class:in-rec={item.inRecording}
          >
            <div class="who"><i></i>{resolveSpeakerName(item, speakerOverrides)}</div>
            <p class="txt">{transcriptDisplay.primaryText}</p>
            {#if transcriptDisplay.secondaryText}
              <p class="sub">{transcriptDisplay.secondaryText}</p>
            {/if}
          </div>
        {/if}
      {/each}
      <div class="messages-end-anchor" bind:this={latestMessageAnchor} aria-hidden="true"></div>
    {/if}
  </div>

  {#if showJumpToLatest}
    <button type="button" class="jump" on:click={() => scrollToLatest()}>↓ 最新へ</button>
  {/if}

  <footer class="live-footer">
    <div class="status-lamp" class:on={isTranscribing}>
      <i></i>
      <span aria-live="polite">
        {statusMessage ?? (isTranscribing ? `文字起こし中 · ${captureModeLabel}` : '一時停止中')}
      </span>
    </div>
    <button type="button" class="quiet-btn" disabled={isCaptureBusy} on:click={onTogglePause}>
      {isTranscribing ? '一時停止' : '再開'}
    </button>
    <div class="spacer"></div>
    <button type="button" class="quiet-btn" disabled={!hasFinalMessages} on:click={onCopy}>
      コピー
    </button>
    <button type="button" class="quiet-btn" disabled={!hasFinalMessages} on:click={onSave}>
      保存
    </button>
    <button
      type="button"
      class="quiet-btn danger"
      class:arm={confirmingClear}
      disabled={threadItems.length === 0}
      on:click={onClear}
    >
      {confirmingClear ? '本当にクリア?' : 'クリア'}
    </button>
  </footer>

  <div class="toast" class:show={actionNotice !== null} role="status">{actionNotice ?? ''}</div>
</div>

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

  .captions-wrap {
    position: relative;
    display: grid;
    grid-template-rows: 1fr auto;
    min-height: 0;
    background: var(--canvas);
  }

  .captions {
    overflow-y: auto;
    padding: 34px 44px 20px;
    display: flex;
    flex-direction: column;
    gap: calc(20px * var(--fs));
    scroll-behavior: smooth;
  }

  .show-earlier {
    align-self: center;
    padding: 6px 14px;
    border-radius: 999px;
    border: 1px solid var(--hairline);
    background: var(--canvas);
    color: var(--blue);
    font-size: 12px;
    font-weight: 600;
  }

  .show-earlier:hover {
    background: var(--blue-soft);
  }

  .cap {
    max-width: 760px;
    animation: fadeUp 0.35s cubic-bezier(0.25, 0.1, 0.25, 1);
    border-left: 2px solid transparent;
    padding-left: 14px;
    margin-left: -16px;
  }

  .cap.in-rec {
    border-left-color: rgba(255, 59, 48, 0.45);
  }

  .cap .who {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    letter-spacing: 0.02em;
    margin-bottom: 5px;
  }

  .cap .who i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }

  .cap.mic .who i {
    background: var(--blue);
  }

  .cap.mic .who {
    color: var(--blue);
  }

  .cap .txt {
    margin: 0;
    font-size: calc(19px * var(--fs));
    line-height: 1.45;
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--ink);
    word-break: break-word;
  }

  .cap .sub {
    margin: 4px 0 0;
    font-size: calc(14.5px * var(--fs));
    line-height: 1.45;
    color: var(--muted);
    font-weight: 400;
    word-break: break-word;
  }

  .cap.interim .txt,
  .cap.interim .sub {
    opacity: 0.45;
  }

  .cap.interim .txt::after {
    content: '';
    display: inline-block;
    width: 3px;
    height: 1em;
    background: var(--ink);
    margin-left: 3px;
    vertical-align: text-bottom;
    animation: blink 1s step-end infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0;
    }
  }

  @keyframes fadeUp {
    from {
      opacity: 0;
      transform: translateY(12px);
    }

    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .cap,
    .rec-marker {
      animation: none;
    }

    .captions {
      scroll-behavior: auto;
    }
  }

  .rec-marker {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--red);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    animation: fadeUp 0.3s ease;
  }

  .rec-marker::before,
  .rec-marker::after {
    content: '';
    flex: 1;
    height: 1px;
    background: rgba(255, 59, 48, 0.3);
  }

  .rec-marker.end {
    color: var(--muted);
  }

  .rec-marker.end::before,
  .rec-marker.end::after {
    background: var(--divider);
  }

  .messages-end-anchor {
    min-height: 1px;
  }

  .empty {
    margin: auto;
    text-align: center;
    max-width: 420px;
    padding: 40px 20px;
  }

  .empty .ring {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: var(--blue-soft);
    display: grid;
    place-items: center;
    margin: 0 auto 18px;
    font-size: 26px;
  }

  .empty.listening .ring {
    background: rgba(52, 199, 89, 0.13);
  }

  .empty.listening .ring span {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--green);
    animation: pulse 1.5s ease-out infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .empty.listening .ring span {
      animation: none;
    }
  }

  @keyframes pulse {
    0% {
      box-shadow: 0 0 0 0 rgba(52, 199, 89, 0.35);
    }

    100% {
      box-shadow: 0 0 0 20px rgba(52, 199, 89, 0);
    }
  }

  .empty h2 {
    margin: 0 0 6px;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--ink);
  }

  .empty p {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
    line-height: 1.55;
  }

  .live-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-top: 1px solid var(--divider);
    background: var(--canvas);
    min-height: 40px;
  }

  .status-lamp {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }

  .status-lamp i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
    opacity: 0.5;
  }

  .status-lamp.on i {
    background: var(--green);
    opacity: 1;
  }

  .quiet-btn {
    padding: 5px 11px;
    border-radius: 7px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
    transition: background 0.15s ease;
  }

  .quiet-btn:hover:not(:disabled) {
    background: var(--hover-wash);
    color: var(--ink);
  }

  .quiet-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .quiet-btn.danger:hover:not(:disabled) {
    background: rgba(255, 59, 48, 0.1);
    color: var(--red);
  }

  .quiet-btn.arm {
    background: var(--red);
    color: #fff;
  }

  .spacer {
    flex: 1;
  }

  .jump {
    position: absolute;
    right: 20px;
    bottom: 56px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--menu-bg);
    backdrop-filter: blur(16px);
    border: 1px solid var(--hairline);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.14);
    font-size: 12px;
    font-weight: 600;
    color: var(--blue);
  }

  .toast {
    position: absolute;
    left: 50%;
    bottom: 52px;
    transform: translateX(-50%) translateY(8px);
    background: var(--menu-bg);
    backdrop-filter: blur(18px) saturate(180%);
    border: 1px solid var(--hairline);
    box-shadow: var(--shadow-menu);
    border-radius: 999px;
    padding: 8px 18px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink);
    opacity: 0;
    pointer-events: none;
    transition: all 0.25s cubic-bezier(0.25, 0.1, 0.25, 1);
    z-index: 60;
  }

  .toast.show {
    opacity: 1;
    transform: translateX(-50%);
  }
</style>
