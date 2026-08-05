<script lang="ts">
  import type { ActionItem } from '$lib/actionItems';
  import type { ChatTurn } from '$lib/aiContext';
  import { formatTimestampMs } from '$lib/recordings';

  export let aiSummary: string;
  export let summaryError: string | null;
  export let isSummaryLoading: boolean;
  export let aiUnavailable: boolean;
  export let actionItems: ActionItem[] = [];
  export let actionNewCount = 0;
  export let actionsError: string | null = null;
  export let isActionsLoading = false;
  export let chatTurns: ChatTurn[];
  export let aiQuestion: string;
  export let isAnswerLoading: boolean;
  export let onRefreshSummary: () => void;
  export let onRefreshActions: () => void;
  export let onToggleAction: (id: string, done: boolean) => void;
  export let onJumpAction: (item: ActionItem) => void;
  export let onOpenActions: () => void;
  export let onAsk: () => void;

  let activeTab: 'summary' | 'actions' | 'ask' = 'summary';

  function handleAskKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      onAsk();
    }
  }

  function selectTab(tab: 'summary' | 'actions' | 'ask') {
    activeTab = tab;
    if (tab === 'actions') {
      onOpenActions();
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
    <div class="ai-tabs" role="tablist" aria-label="Meeting AI">
      <button
        type="button"
        class:active={activeTab === 'summary'}
        role="tab"
        aria-selected={activeTab === 'summary'}
        on:click={() => selectTab('summary')}
      >
        要約
      </button>
      <button
        type="button"
        class:active={activeTab === 'actions'}
        role="tab"
        aria-selected={activeTab === 'actions'}
        on:click={() => selectTab('actions')}
      >
        アクション
        {#if actionNewCount > 0}
          <span class="badge-new">{actionNewCount}</span>
        {/if}
      </button>
      <button
        type="button"
        class:active={activeTab === 'ask'}
        role="tab"
        aria-selected={activeTab === 'ask'}
        on:click={() => selectTab('ask')}
      >
        Ask
      </button>
    </div>

    <div class="ai-body">
      {#if activeTab === 'summary'}
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
      {:else if activeTab === 'actions'}
        <h3>
          アクション
          <button type="button" class="link-btn" disabled={isActionsLoading} on:click={onRefreshActions}>
            {isActionsLoading ? '抽出中…' : '更新'}
          </button>
        </h3>
        {#if actionsError}
          <p class="ai-error">{actionsError}</p>
        {/if}
        {#if actionItems.length === 0}
          <div class="ai-card placeholder">
            発話が進むと、担当と発言元つきのアクションアイテムがここに表示されます。
          </div>
        {:else}
          <div class="todo" aria-live="polite">
            {#each actionItems as item (item.id)}
              <label class="td" class:done={item.done}>
                <input
                  type="checkbox"
                  checked={item.done}
                  on:change={(event) => onToggleAction(item.id, event.currentTarget.checked)}
                />
                <span>
                  <span class="tt">{item.text}</span>
                  <span class="tm">
                    <span class="who">{item.assignee ?? '担当未定'}</span>
                    {#if item.timestampMs !== null}
                      <button type="button" class="ts" on:click={() => onJumpAction(item)}>
                        {formatTimestampMs(item.timestampMs)} ↗
                      </button>
                    {/if}
                  </span>
                </span>
              </label>
            {/each}
          </div>
        {/if}
      {:else}
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
      {/if}
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
    gap: 12px;
    padding: 14px 16px 18px;
    min-width: 300px;
  }

  .ai-tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--divider);
  }

  .ai-tabs button {
    flex: 1;
    padding: 7px 0 9px;
    color: var(--muted);
    font-size: 12.5px;
    font-weight: 600;
    border-bottom: 2px solid transparent;
  }

  .ai-tabs button.active {
    color: var(--ink);
    border-bottom-color: var(--blue);
  }

  .badge-new {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 17px;
    height: 17px;
    margin-left: 4px;
    border-radius: 999px;
    background: var(--blue);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
  }

  .ai-body {
    display: grid;
    gap: 12px;
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

  .todo {
    display: grid;
    gap: 8px;
  }

  .td {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--canvas);
    border: 1px solid var(--divider);
    align-items: start;
  }

  .td input {
    margin-top: 2px;
    accent-color: var(--blue);
  }

  .td.done {
    opacity: 0.62;
  }

  .td.done .tt {
    text-decoration: line-through;
    color: var(--muted);
  }

  .tt {
    display: block;
    color: var(--ink);
    font-size: 12.5px;
    line-height: 1.45;
    word-break: break-word;
  }

  .tm {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 4px;
    color: var(--muted);
    font-size: 11px;
  }

  .tm .who {
    font-weight: 700;
  }

  .tm .ts {
    color: var(--blue);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
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
