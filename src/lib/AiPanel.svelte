<script lang="ts">
  import type { ChatTurn } from '$lib/aiContext';

  export let aiSummary: string;
  export let summaryError: string | null;
  export let isSummaryLoading: boolean;
  export let chatTurns: ChatTurn[];
  export let aiQuestion: string;
  export let isAnswerLoading: boolean;
  export let onRefreshSummary: () => void;
  export let onAsk: () => void;
</script>

<aside class="meeting-ai" aria-label="Meeting AI">
  <div class="ai-head">
    <div>
      <span class="date-pill">AI</span>
      <h2>Meeting</h2>
    </div>
    <button type="button" disabled={isSummaryLoading} on:click={onRefreshSummary}>
      {isSummaryLoading ? 'Updating' : 'Refresh'}
    </button>
  </div>

  <section class="ai-section">
    <div class="ai-section-head">
      <h3>Summary</h3>
      {#if isSummaryLoading}
        <span class="ai-working">updating…</span>
      {/if}
    </div>
    {#if summaryError}
      <p class="ai-error">{summaryError}</p>
    {/if}
    <pre>{aiSummary || 'No summary yet.'}</pre>
  </section>

  <section class="ai-section ask-section">
    <div class="ai-section-head">
      <h3>Ask</h3>
      <button type="button" disabled={isAnswerLoading || !aiQuestion.trim()} on:click={onAsk}>
        {isAnswerLoading ? 'Asking' : 'Ask'}
      </button>
    </div>
    <div class="chat-turns" aria-live="polite">
      {#if chatTurns.length === 0}
        <p class="chat-empty">Ask anything about the meeting so far.</p>
      {/if}
      {#each chatTurns as turn}
        <div class="chat-turn">
          <p class="chat-question">{turn.question}</p>
          {#if turn.answer}
            <p class="chat-answer">{turn.answer}</p>
          {:else}
            <p class="chat-answer pending">Thinking…</p>
          {/if}
        </div>
      {/each}
    </div>
    <textarea bind:value={aiQuestion} rows="3" aria-label="Meeting question"></textarea>
  </section>
</aside>

<style>
  button {
    font: inherit;
    cursor: pointer;
  }

  button:focus-visible,
  textarea:focus-visible {
    outline: 3px solid rgba(0, 102, 204, 0.24);
    outline-offset: 2px;
  }

  .meeting-ai {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    gap: 12px;
    overflow: auto;
    background: var(--canvas-parchment);
    padding: 16px;
  }

  .ai-head,
  .ai-section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .ai-head h2 {
    margin: 7px 0 0;
    color: var(--legacy-ink);
    font-size: 21px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .ai-head button,
  .ai-section-head button {
    border: 1px solid rgba(0, 102, 204, 0.16);
    border-radius: 8px;
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
  }

  .ai-head button:hover:not(:disabled),
  .ai-section-head button:hover:not(:disabled) {
    background: rgba(0, 102, 204, 0.15);
    color: var(--apple-blue-focus);
  }

  .ai-head button:disabled,
  .ai-section-head button:disabled {
    cursor: wait;
    opacity: 0.56;
  }

  .ai-working {
    color: var(--ink-muted);
    font-size: 11px;
  }

  .ai-error {
    max-height: 58px;
    overflow: auto;
    margin: 0;
    color: #b3261e;
    font-size: 12px;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ai-section {
    display: flex;
    flex: 0 0 auto;
    min-height: 0;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid var(--legacy-hairline);
    padding-top: 12px;
  }

  .ai-section h3 {
    margin: 0;
    color: var(--ink-secondary);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0;
  }

  .ai-section pre {
    min-height: 0;
    max-height: 240px;
    overflow: auto;
    margin: 0;
    color: var(--legacy-ink);
    font-family: inherit;
    font-size: 13px;
    font-weight: 400;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ask-section {
    padding-bottom: 12px;
  }

  .chat-turns {
    display: flex;
    max-height: 260px;
    flex-direction: column;
    gap: 10px;
    overflow: auto;
  }

  .chat-empty {
    margin: 0;
    color: var(--ink-muted);
    font-size: 12px;
  }

  .chat-turn {
    display: grid;
    gap: 5px;
  }

  .chat-question {
    justify-self: end;
    max-width: 90%;
    margin: 0;
    border-radius: 12px 12px 4px 12px;
    background: var(--apple-blue);
    color: white;
    padding: 7px 10px;
    font-size: 12.5px;
    line-height: 1.4;
    word-break: break-word;
  }

  .chat-answer {
    justify-self: start;
    max-width: 95%;
    margin: 0;
    border: 1px solid var(--divider-soft);
    border-radius: 12px 12px 12px 4px;
    background: var(--legacy-canvas);
    color: var(--legacy-ink);
    padding: 7px 10px;
    font-size: 12.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .chat-answer.pending {
    color: var(--ink-muted);
  }

  .ask-section textarea {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    resize: none;
    border: 1px solid var(--legacy-hairline);
    border-radius: 8px;
    background: var(--legacy-canvas);
    color: var(--legacy-ink);
    padding: 9px 10px;
    font: inherit;
    font-size: 13px;
    line-height: 1.35;
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

  @media (max-width: 640px) {
    .meeting-ai {
      overflow: auto;
    }

    .meeting-ai .ai-section {
      margin-top: 12px;
    }

    .ask-section textarea {
      min-height: 72px;
    }
  }
</style>
