<script lang="ts">
  import type { ChatTurn } from '$lib/aiContext';

  export let aiSummary: string;
  export let summaryError: string | null;
  export let isSummaryLoading: boolean;
  export let aiUnavailable: boolean;
  export let chatTurns: ChatTurn[];
  export let aiQuestion: string;
  export let isAnswerLoading: boolean;
  export let onRefreshSummary: () => void;
  export let onAsk: () => void;

  function handleAskKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      onAsk();
    }
  }
</script>

<aside class="ai-panel" aria-label="Meeting AI">
  {#if aiUnavailable}
    <h3>Meeting AI</h3>
    <div class="ai-card placeholder">
      この Mac では Apple Intelligence を利用できないため、要約と質問は使えません。
      macOS のアップデートと Apple Intelligence の有効化をご確認ください。
    </div>
  {:else}
    <h3>
      要約
      <button type="button" class="link-btn" disabled={isSummaryLoading} on:click={onRefreshSummary}>
        {isSummaryLoading ? '更新中…' : '更新'}
      </button>
    </h3>
    {#if summaryError}
      <p class="ai-error">{summaryError}</p>
    {/if}
    <div class="ai-card" class:placeholder={!aiSummary}>
      {aiSummary || '発話が進むと、会話の要約が自動でここに表示されます。'}
    </div>

    <h3>質問</h3>
    <div class="ai-turns" aria-live="polite">
      {#each chatTurns as turn}
        <div class="ai-q">{turn.question}</div>
        {#if turn.answer}
          <div class="ai-a">{turn.answer}</div>
        {:else}
          <div class="ai-a pending">考え中…</div>
        {/if}
      {/each}
    </div>
    <div class="ai-ask">
      <textarea
        rows="2"
        bind:value={aiQuestion}
        placeholder="この会議について質問…"
        aria-label="Meeting question"
        disabled={isAnswerLoading}
        on:keydown={handleAskKeydown}
      ></textarea>
    </div>
  {/if}
</aside>

<style>
  button {
    font: inherit;
    cursor: pointer;
    color: inherit;
    background: none;
    border: 0;
  }

  button:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: 2px;
    border-radius: 8px;
  }

  .ai-panel {
    border-left: 1px solid var(--divider);
    background: var(--parchment);
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 16px;
    min-width: 300px;
  }

  .ai-panel h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .link-btn {
    color: var(--blue);
    font-size: 12px;
    font-weight: 500;
  }

  .link-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ai-card {
    background: var(--canvas);
    border: 1px solid var(--divider);
    border-radius: 12px;
    padding: 12px 14px;
    font-size: 13px;
    line-height: 1.55;
    color: var(--ink);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ai-card.placeholder {
    color: var(--muted);
  }

  .ai-error {
    max-height: 58px;
    overflow: auto;
    margin: 0;
    color: var(--red);
    font-size: 12px;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ai-turns {
    display: grid;
    gap: 8px;
  }

  .ai-q {
    justify-self: end;
    background: var(--blue);
    color: #fff;
    border-radius: 13px 13px 4px 13px;
    padding: 7px 11px;
    font-size: 12.5px;
    max-width: 88%;
    word-break: break-word;
  }

  .ai-a {
    justify-self: start;
    background: var(--canvas);
    border: 1px solid var(--divider);
    border-radius: 13px 13px 13px 4px;
    padding: 7px 11px;
    font-size: 12.5px;
    max-width: 92%;
    line-height: 1.5;
    color: var(--ink);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ai-a.pending {
    color: var(--muted);
  }

  .ai-ask {
    margin-top: auto;
    display: grid;
    gap: 8px;
  }

  .ai-ask textarea {
    resize: none;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--canvas);
    color: var(--ink);
    padding: 9px 11px;
    font: inherit;
    font-size: 13px;
  }
</style>
