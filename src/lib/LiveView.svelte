<script lang="ts">
  import AiPanel from '$lib/AiPanel.svelte';
  import CaptionThread from '$lib/CaptionThread.svelte';
  import type { ChatTurn } from '$lib/aiContext';
  import type { SpeechModelSelection } from '$lib/speechModels';
  import type { ChatMessage } from '$lib/transcripts';

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

  export let aiSummary: string;
  export let summaryError: string | null;
  export let isSummaryLoading: boolean;
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

<div class="conversation">
  <CaptionThread
    bind:this={thread}
    {visibleMessages}
    {hasFinalMessages}
    {mainLanguage}
    {subLanguage}
    {transcriptFontScale}
    {statusMessage}
    {actionNotice}
    {isRecording}
    {isStarting}
    {isMicRecording}
    {isSpeakerRecording}
    {speechModel}
    {confirmingClear}
    {onFontScaleChange}
    {onCopy}
    {onSave}
    {onClear}
  />

  <AiPanel
    {aiSummary}
    {summaryError}
    {isSummaryLoading}
    {chatTurns}
    bind:aiQuestion
    {isAnswerLoading}
    {onRefreshSummary}
    {onAsk}
  />
</div>

<style>
  .conversation {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 0;
    min-height: 0;
    padding: 0;
  }

  @media (max-width: 980px) {
    .conversation {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) minmax(280px, 38vh);
    }
  }

  @media (max-width: 640px) {
    .conversation {
      grid-template-rows: minmax(0, 1fr) minmax(280px, 34vh);
    }
  }
</style>
