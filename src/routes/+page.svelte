<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { chooseDefaultLanguagePair, type LanguageInfo } from '$lib/languages';
  import { transcriptEventToMessage, type ChatMessage, type TranscriptEvent } from '$lib/transcripts';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
  };

  let selectedStream: 'speaker' | 'mic' = 'speaker';
  let sourceLanguage = 'en-US';
  let targetLanguage = 'ja-JP';
  let installedLanguages: LanguageInfo[] = [
    { id: 'en-US', label: 'English' },
    { id: 'ja-JP', label: 'Japanese' }
  ];
  let messages: ChatMessage[] = [];
  let isRecording = false;
  let status = 'Ready';
  let errorMessage: string | null = null;

  onMount(async () => {
    const unlistenTranscript = await listen<TranscriptEvent>('transcript-event', (event) => {
      messages = [...messages, transcriptEventToMessage(event.payload)];
      status = event.payload.isFinal ? 'Saved phrase' : 'Listening';
    });

    const unlistenError = await listen<string>('helper-error', (event) => {
      errorMessage = event.payload;
      status = 'Needs attention';
    });

    await detectLanguages();

    return () => {
      unlistenTranscript();
      unlistenError();
    };
  });

  async function detectLanguages() {
    try {
      const payload = await invoke<LanguageDetectionPayload>('detect_languages');
      installedLanguages = payload.installed.length > 0 ? payload.installed : installedLanguages;
      const pair = chooseDefaultLanguagePair(installedLanguages);
      sourceLanguage = pair.source;
      targetLanguage = pair.target;
      status = 'Languages ready';
    } catch (error) {
      errorMessage = String(error);
      status = 'Using fallback languages';
    }
  }

  async function toggleRecording() {
    errorMessage = null;

    if (isRecording) {
      await invoke('stop_microphone_session');
      isRecording = false;
      status = 'Paused';
      return;
    }

    await invoke('start_microphone_session', {
      sourceLanguage,
      targetLanguage
    });
    selectedStream = 'mic';
    isRecording = true;
    status = 'Listening';
  }

  function clearMessages() {
    messages = [];
    errorMessage = null;
    status = isRecording ? 'Listening' : 'Ready';
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<main class="shell">
  <section class="window" aria-label="LivePolyTrans">
    <div class="titlebar" data-tauri-drag-region>
      <div class="traffic-lights" aria-hidden="true">
        <span class="close"></span>
        <span class="minimize"></span>
        <span class="zoom"></span>
      </div>

      <div class="segmented" aria-label="Audio stream">
        <button class:active={selectedStream === 'speaker'} on:click={() => (selectedStream = 'speaker')}>
          Speaker
        </button>
        <button class:active={selectedStream === 'mic'} on:click={() => (selectedStream = 'mic')}>
          Mic
        </button>
      </div>

      <select bind:value={sourceLanguage} aria-label="Source language">
        {#each installedLanguages as language}
          <option value={language.id}>{language.label}</option>
        {/each}
      </select>

      <select bind:value={targetLanguage} aria-label="Target language">
        {#each installedLanguages as language}
          <option value={language.id}>{language.label}</option>
        {/each}
      </select>

      <button class="record" class:recording={isRecording} on:click={toggleRecording}>
        <span></span>{isRecording ? 'Stop Recording' : 'Start Recording'}
      </button>
    </div>

    <div class="content">
      {#if messages.length === 0}
        <div class="empty-state">
          <div class="bubble-icon">􀌤</div>
          <p>Listening for {selectedStream} audio.</p>
          <small>Choose two installed languages, then start recording.</small>
        </div>
      {:else}
        <div class="messages" aria-live="polite">
          {#each messages as message (message.id)}
            <article class="message" class:self={message.role === 'self'}>
              <div class="bubble">
                <div class="meta">{message.role === 'self' ? 'Mic' : 'Speaker'} · {message.language}</div>
                <p>{message.text}</p>
                {#if message.translation}
                  <p class="translation">{message.translation}</p>
                {/if}
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </div>

    <footer>
      <div class="status"><span class:idle={!isRecording}></span>{status}</div>
      <div class="actions">
        {#if errorMessage}
          <p class="error">{errorMessage}</p>
        {/if}
        <button on:click={detectLanguages}>Refresh Languages</button>
        <button on:click={clearMessages}>Clear</button>
      </div>
    </footer>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    min-height: 100vh;
    overflow: hidden;
    background:
      radial-gradient(circle at 50% 34%, rgba(137, 137, 132, 0.92) 0, rgba(78, 79, 77, 0.94) 48%, #3b3c3b 100%),
      #4f504f;
    color: #1f1f1f;
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  }

  button,
  select {
    font: inherit;
  }

  .shell {
    display: grid;
    min-height: 100vh;
    place-items: center;
    padding: 32px;
  }

  .window {
    position: relative;
    display: grid;
    grid-template-rows: auto 1fr auto;
    width: min(1160px, 94vw);
    height: min(760px, 86vh);
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.38);
    border-radius: 34px;
    background:
      linear-gradient(135deg, rgba(255, 255, 255, 0.86), rgba(226, 226, 224, 0.9)),
      #ececeb;
    box-shadow:
      0 34px 80px rgba(0, 0, 0, 0.38),
      inset 0 1px 0 rgba(255, 255, 255, 0.82);
    backdrop-filter: blur(28px);
  }

  .titlebar {
    display: grid;
    grid-template-columns: 128px auto 150px 164px 1fr;
    align-items: center;
    gap: 14px;
    padding: 20px 20px 16px;
  }

  .traffic-lights {
    display: flex;
    gap: 12px;
    padding-left: 10px;
  }

  .traffic-lights span {
    width: 18px;
    height: 18px;
    border-radius: 999px;
  }

  .close {
    background: #ff5f57;
    border: 1px solid #e0443e;
  }

  .minimize {
    background: #febc2e;
    border: 1px solid #d89a15;
  }

  .zoom {
    background: #28c840;
    border: 1px solid #1faa34;
  }

  .segmented,
  select,
  .record,
  footer button {
    border: 1px solid rgba(255, 255, 255, 0.78);
    border-radius: 22px;
    background: rgba(255, 255, 255, 0.66);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.8),
      0 18px 34px rgba(62, 62, 62, 0.14);
  }

  .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: 4px;
  }

  .segmented button,
  footer button {
    border: 0;
    background: transparent;
    color: #535353;
    cursor: pointer;
  }

  .segmented button {
    border-radius: 18px;
    padding: 10px 22px;
    font-size: 18px;
    font-weight: 650;
  }

  .segmented button.active {
    background: rgba(215, 215, 214, 0.94);
    color: #333;
  }

  select {
    min-width: 0;
    padding: 12px 14px;
    color: #4c4c4c;
    font-size: 17px;
    font-weight: 620;
  }

  .record {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 24px;
    border-radius: 999px;
    color: #111;
    cursor: pointer;
    font-size: 17px;
    font-weight: 720;
  }

  .record span {
    width: 14px;
    height: 14px;
    border: 4px solid #ff174d;
    border-radius: 999px;
  }

  .record.recording span {
    border-radius: 4px;
    background: #ff174d;
  }

  .content {
    min-height: 0;
    padding: 10px 34px 20px;
  }

  .empty-state {
    display: grid;
    height: 100%;
    place-content: center;
    color: #777;
    text-align: center;
  }

  .bubble-icon {
    font-size: 32px;
    line-height: 1;
  }

  .empty-state p {
    margin: 16px 0 4px;
    font-size: 19px;
    font-weight: 600;
  }

  .empty-state small {
    color: #8b8b8b;
    font-size: 14px;
  }

  .messages {
    display: flex;
    max-height: 100%;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    padding: 8px 4px 24px;
  }

  .message {
    display: flex;
  }

  .message.self {
    justify-content: flex-end;
  }

  .bubble {
    max-width: min(620px, 72%);
    border-radius: 24px;
    padding: 14px 16px;
    background: rgba(255, 255, 255, 0.72);
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.08);
  }

  .self .bubble {
    background: linear-gradient(135deg, #2387ff, #0f6fe5);
    color: white;
  }

  .meta {
    margin-bottom: 6px;
    opacity: 0.66;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .bubble p {
    margin: 0;
    font-size: 18px;
    line-height: 1.45;
  }

  .translation {
    margin-top: 8px !important;
    opacity: 0.72;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 0 30px 24px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #707070;
    font-size: 17px;
    font-weight: 650;
  }

  .status span {
    width: 10px;
    height: 10px;
    border-radius: 999px;
    background: #35c759;
  }

  .status span.idle {
    background: #a0a0a0;
  }

  .actions {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }

  .error {
    max-width: 360px;
    overflow: hidden;
    color: #a2382d;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  footer button {
    padding: 8px 14px;
    font-size: 14px;
    font-weight: 650;
  }

  @media (max-width: 780px) {
    .shell {
      padding: 16px;
    }

    .window {
      height: 92vh;
      border-radius: 26px;
    }

    .titlebar {
      grid-template-columns: 1fr;
      gap: 10px;
    }

    .traffic-lights {
      display: none;
    }

    .record {
      justify-self: stretch;
      justify-content: center;
    }

    footer {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>
