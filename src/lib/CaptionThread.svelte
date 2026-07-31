<script lang="ts">
  import { isScrolledToBottom } from '$lib/scroll';
  import { displayTranscriptMessage } from '$lib/transcriptDisplay';
  import {
    canDecreaseTranscriptFontScale,
    canIncreaseTranscriptFontScale,
    decreaseTranscriptFontScale,
    increaseTranscriptFontScale
  } from '$lib/transcriptFontSize';
  import type { ChatMessage } from '$lib/transcripts';
  import type { SpeechModelSelection } from '$lib/speechModels';

  export let visibleMessages: ChatMessage[];
  export let hasFinalMessages: boolean;
  export let mainLanguage: string;
  export let subLanguage: string;
  export let transcriptFontScale: number;
  export let statusMessage: string | null;
  export let actionNotice: string | null;
  export let isRecording: boolean;
  export let isStarting: boolean;
  export let isMicRecording: boolean;
  export let isSpeakerRecording: boolean;
  export let speechModel: SpeechModelSelection;
  export let confirmingClear: boolean;
  export let onFontScaleChange: (scale: number) => void;
  export let onCopy: () => void;
  export let onSave: () => void;
  export let onClear: () => void;

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
</script>

<section
  class="thread"
  aria-label="Translation chat"
  style="--transcript-font-scale: {transcriptFontScale}"
>
  <div class="thread-head">
    <div>
      <span class="date-pill">Today</span>
      <h1>Live translation log</h1>
      {#if statusMessage}
        <p class="status-chip" aria-live="polite">{statusMessage}</p>
      {/if}
      {#if actionNotice}
        <p class="action-notice">{actionNotice}</p>
      {/if}
    </div>
    <div class="thread-side">
      <div class="legend" aria-label="Message lanes">
        <span><i class="mic-dot"></i>Speaker A</span>
        <span><i class="speaker-dot"></i>Speaker B</span>
      </div>
      <div class="thread-actions" aria-label="Transcript actions">
        <button
          type="button"
          class="font-size-button"
          title="Decrease transcript text size"
          aria-label="Decrease transcript text size"
          disabled={!canDecreaseTranscriptFontScale(transcriptFontScale)}
          on:click={() => onFontScaleChange(decreaseTranscriptFontScale(transcriptFontScale))}
        >
          A−
        </button>
        <button
          type="button"
          class="font-size-button"
          title="Increase transcript text size"
          aria-label="Increase transcript text size"
          disabled={!canIncreaseTranscriptFontScale(transcriptFontScale)}
          on:click={() => onFontScaleChange(increaseTranscriptFontScale(transcriptFontScale))}
        >
          A＋
        </button>
        <button type="button" disabled={!hasFinalMessages} on:click={onCopy}> Copy </button>
        <button type="button" disabled={!hasFinalMessages} on:click={onSave}> Save </button>
        <button
          type="button"
          class="clear-button"
          class:confirming={confirmingClear}
          disabled={visibleMessages.length === 0}
          on:click={onClear}
        >
          {confirmingClear ? 'Really clear?' : 'Clear'}
        </button>
      </div>
    </div>
  </div>

  {#if visibleMessages.length === 0 && isRecording}
    <div class="listening-empty" aria-live="polite">
      <div class="pulse-ring">
        <span></span>
      </div>
      <h2>{isStarting ? 'Preparing audio capture...' : 'Listening for speech'}</h2>
      <p>
        {speechModel.engine === 'whisper'
          ? 'Speak normally. Whisper transcribes each phrase after you pause, so the first text appears once you finish a sentence.'
          : 'Speak normally. The first words can take a few seconds while Apple Speech warms up.'}
      </p>
      <div class="stream-chips" aria-label="Active streams">
        <span class:active={isSpeakerRecording}>Speaker</span>
        <span class:active={isMicRecording}>Mic</span>
      </div>
    </div>
  {:else if visibleMessages.length === 0}
    <div class="starter" aria-live="polite">
      <article class="chat-row speaker-row">
        <div class="chat-bubble incoming">
          <span>Preview · Speaker B</span>
          <p>The other person’s audio will appear here.</p>
          <small>Press Record to start listening.</small>
        </div>
      </article>
      <article class="chat-row self-row">
        <div class="chat-bubble outgoing">
          <span>Preview · Speaker A</span>
          <p>Your spoken replies will appear in the same conversation.</p>
          <small>This is a preview, not a captured transcript.</small>
        </div>
      </article>
    </div>
  {:else}
    <div class="messages-shell">
      <div
        class="messages"
        bind:this={messagesContainer}
        aria-live="polite"
        on:scroll={handleMessagesScroll}
      >
        {#each visibleMessages as message (message.id)}
          {@const transcriptDisplay = displayTranscriptMessage(message, mainLanguage, subLanguage)}
          <article class="chat-row" class:self-row={message.role === 'self'}>
            <div
              class="chat-bubble"
              class:outgoing={message.role === 'self'}
              class:incoming={message.role !== 'self'}
              class:pending={!message.isFinal}
            >
              <span>{message.speakerLabel} · {transcriptDisplay.primaryLanguage}</span>
              <p>{transcriptDisplay.primaryText}</p>
              {#if transcriptDisplay.secondaryText}
                <small>{transcriptDisplay.secondaryText}</small>
              {/if}
            </div>
          </article>
        {/each}
        <div class="messages-end-anchor" bind:this={latestMessageAnchor} aria-hidden="true"></div>
      </div>

      {#if showJumpToLatest}
        <button type="button" class="jump-to-latest" on:click={() => scrollToLatest()}>
          Jump to latest
        </button>
      {/if}
    </div>
  {/if}
</section>

<style>
  button {
    font: inherit;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 3px solid rgba(0, 102, 204, 0.24);
    outline-offset: 2px;
  }

  .thread {
    display: grid;
    min-height: 0;
    grid-template-rows: auto 1fr;
    overflow: hidden;
    border-right: 1px solid var(--legacy-hairline);
    background: var(--legacy-canvas);
  }

  .thread-head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--divider-soft);
    padding: 20px 24px 14px;
  }

  .thread-head h1 {
    margin: 8px 0 0;
    color: var(--legacy-ink);
    font-size: 21px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .status-chip {
    width: fit-content;
    margin: 8px 0 0;
    border: 1px solid rgba(0, 102, 204, 0.24);
    border-radius: 999px;
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
  }

  .action-notice {
    margin: 8px 0 0;
    color: var(--ink-muted);
    font-size: 11px;
    word-break: break-all;
  }

  .thread-side {
    display: grid;
    justify-items: end;
    gap: 8px;
  }

  .thread-actions {
    display: flex;
    gap: 6px;
  }

  .thread-actions button {
    border: 1px solid var(--legacy-hairline);
    border-radius: 8px;
    background: var(--legacy-canvas);
    color: var(--ink-muted);
    padding: 5px 10px;
    font-size: 12px;
    font-weight: 600;
  }

  .thread-actions button:hover:not(:disabled) {
    border-color: rgba(0, 102, 204, 0.3);
    color: var(--apple-blue);
  }

  .thread-actions button:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .thread-actions .clear-button.confirming {
    border-color: rgba(179, 38, 30, 0.4);
    background: #b3261e;
    color: #ffffff;
  }

  .thread-actions .clear-button.confirming:hover:not(:disabled) {
    border-color: rgba(179, 38, 30, 0.6);
    color: #ffffff;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--ink-muted);
    font-size: 12px;
    font-weight: 500;
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .legend i {
    width: 9px;
    height: 9px;
    border-radius: 999px;
  }

  .speaker-dot {
    background: var(--ink-muted);
  }

  .mic-dot {
    background: var(--apple-blue);
  }

  .date-pill {
    width: fit-content;
    border: 1px solid var(--legacy-hairline);
    border-radius: 999px;
    background: var(--surface-pearl);
    color: var(--ink-muted);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
  }

  .starter,
  .listening-empty,
  .messages {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 16px;
    overflow: auto;
    padding: 28px 24px 52px;
  }

  .messages {
    scroll-padding-bottom: 52px;
  }

  .messages-shell {
    display: grid;
    position: relative;
    min-height: 0;
    overflow: hidden;
  }

  .messages-shell .messages {
    min-height: 0;
    height: 100%;
  }

  .jump-to-latest {
    position: absolute;
    right: 24px;
    bottom: 20px;
    border: 1px solid rgba(0, 102, 204, 0.18);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.94);
    color: var(--apple-blue);
    padding: 9px 13px;
    font-size: 12px;
    font-weight: 600;
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
    backdrop-filter: blur(14px);
  }

  .jump-to-latest:hover {
    transform: translateY(-1px);
  }

  .jump-to-latest:active {
    transform: translateY(0);
  }

  .messages-end-anchor {
    min-height: 1px;
  }

  .starter {
    justify-content: center;
  }

  .listening-empty {
    align-items: center;
    justify-content: center;
    text-align: center;
  }

  .pulse-ring {
    display: grid;
    width: 72px;
    height: 72px;
    place-items: center;
    border-radius: 999px;
    background: rgba(52, 199, 89, 0.11);
  }

  .pulse-ring span {
    width: 22px;
    height: 22px;
    border-radius: 999px;
    background: var(--apple-green);
    box-shadow: 0 0 0 0 rgba(52, 199, 89, 0.32);
    animation: listening-pulse 1.4s ease-out infinite;
  }

  .listening-empty h2 {
    margin: 22px 0 6px;
    color: var(--legacy-ink);
    font-size: 21px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .listening-empty p {
    max-width: 430px;
    margin: 0;
    color: var(--ink-muted);
    font-size: 14px;
    font-weight: 400;
    line-height: 1.5;
  }

  .stream-chips {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }

  .stream-chips span {
    border: 1px solid var(--legacy-hairline);
    border-radius: 999px;
    background: var(--legacy-canvas);
    color: var(--ink-muted);
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
  }

  .stream-chips span.active {
    border-color: rgba(0, 102, 204, 0.24);
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
  }

  @keyframes listening-pulse {
    0% {
      box-shadow: 0 0 0 0 rgba(52, 199, 89, 0.32);
    }

    100% {
      box-shadow: 0 0 0 18px rgba(52, 199, 89, 0);
    }
  }

  .chat-row {
    display: flex;
    align-items: end;
    gap: 10px;
  }

  .self-row {
    justify-content: flex-end;
  }

  .chat-bubble {
    max-width: min(620px, 72%);
    border-radius: 18px;
    padding: 10px 13px 11px;
    word-break: break-word;
  }

  .chat-bubble.incoming {
    border: 1px solid var(--divider-soft);
    border-bottom-left-radius: 5px;
    background: var(--canvas-parchment);
    color: var(--legacy-ink);
  }

  .chat-bubble.outgoing {
    border-bottom-right-radius: 5px;
    background: var(--apple-blue);
    color: white;
  }

  .chat-bubble span {
    display: block;
    margin-bottom: 5px;
    opacity: 0.62;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .chat-bubble p {
    margin: 0;
    font-size: calc(17px * var(--transcript-font-scale, 1));
    font-weight: 400;
    line-height: 1.42;
    letter-spacing: 0;
  }

  .chat-bubble small {
    display: block;
    margin-top: 8px;
    opacity: 0.64;
    font-size: calc(14px * var(--transcript-font-scale, 1));
    font-weight: 400;
    line-height: 1.35;
  }

  .chat-bubble.pending p,
  .chat-bubble.pending small {
    opacity: 0.45;
  }

  .chat-bubble.pending span {
    opacity: 0.42;
  }

  @media (max-width: 980px) {
    .thread {
      border-right: 0;
      border-bottom: 1px solid var(--legacy-hairline);
    }
  }

  @media (max-width: 640px) {
    .thread-head {
      display: grid;
    }

    .legend {
      flex-wrap: wrap;
    }

    .chat-bubble {
      max-width: 82%;
    }

    .starter {
      justify-content: flex-start;
      gap: 12px;
      padding: 14px 24px 28px;
    }
  }
</style>
