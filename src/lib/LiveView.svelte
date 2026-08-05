<script lang="ts">
  import AiPanel from '$lib/AiPanel.svelte';
  import CaptionThread from '$lib/CaptionThread.svelte';
  import type { ChatTurn } from '$lib/aiContext';
  import type { LiveSpeakerOverrides } from '$lib/settingsStore';
  import type { SpeechModelSelection } from '$lib/speechModels';
  import type { ThreadItem } from '$lib/transcripts';

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
  export let speakerOverrides: LiveSpeakerOverrides | null = null;
  export let onTogglePause: () => void;
  export let onCopy: () => void;
  export let onSave: () => void;
  export let onClear: () => void;

  export let aiOpen: boolean;
  export let aiSummary: string;
  export let summaryError: string | null;
  export let isSummaryLoading: boolean;
  export let aiUnavailable: boolean;
  export let chatTurns: ChatTurn[];
  export let aiQuestion: string;
  export let isAnswerLoading: boolean;
  export let onRefreshSummary: () => void;
  export let onAsk: () => void;

  let thread: CaptionThread | undefined;

  export function shouldStickToLatest(): boolean {
    return thread?.shouldStickToLatest() ?? true;
  }

  export function syncJumpToLatestButton() {
    thread?.syncJumpToLatestButton();
  }

  export function scrollToLatest(behavior: ScrollBehavior = 'smooth') {
    thread?.scrollToLatest(behavior);
  }
</script>

<div class="conversation" class:ai-open={aiOpen}>
  <CaptionThread
    bind:this={thread}
    {threadItems}
    {hasFinalMessages}
    {mainLanguage}
    {subLanguage}
    {transcriptFontScale}
    {statusMessage}
    {actionNotice}
    {isTranscribing}
    {isStarting}
    {isCaptureBusy}
    {captureModeLabel}
    {speechModel}
    {confirmingClear}
    {speakerOverrides}
    {onTogglePause}
    {onCopy}
    {onSave}
    {onClear}
  />

  {#if aiOpen}
    <AiPanel
      {aiSummary}
      {summaryError}
      {isSummaryLoading}
      {aiUnavailable}
      {chatTurns}
      bind:aiQuestion
      {isAnswerLoading}
      {onRefreshSummary}
      {onAsk}
    />
  {/if}
</div>

<style>
  .conversation {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
  }

  .conversation.ai-open {
    grid-template-columns: minmax(0, 1fr) 300px;
  }

  @media (max-width: 900px) {
    .conversation.ai-open {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) minmax(240px, 36vh);
    }
  }
</style>
