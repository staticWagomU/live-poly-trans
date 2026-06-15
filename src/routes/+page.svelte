<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    chooseDefaultLanguagePair,
    languageControlLabel,
    type LanguageInfo
  } from '$lib/languages';
  import { transcriptEventToMessage, type ChatMessage, type TranscriptEvent } from '$lib/transcripts';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
  };

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

  $: sourceLabel =
    installedLanguages.find((language) => language.id === sourceLanguage)?.label ?? sourceLanguage;
  $: targetLabel =
    installedLanguages.find((language) => language.id === targetLanguage)?.label ?? targetLanguage;

  onMount(async () => {
    const unlistenTranscript = await listen<TranscriptEvent>('transcript-event', (event) => {
      messages = [...messages, transcriptEventToMessage(event.payload)];
      status = event.payload.isFinal ? 'Saved phrase' : 'Listening live';
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
    isRecording = true;
    status = 'Listening live';
  }

  function clearMessages() {
    messages = [];
    errorMessage = null;
    status = isRecording ? 'Listening live' : 'Ready';
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<main class="stage">
  <section class="window" aria-label="LivePolyTrans">
    <header class="toolbar" data-tauri-drag-region>
      <div class="identity">
        <strong>LivePolyTrans</strong>
        <span>Speaker and mic appear in one conversation</span>
      </div>

      <div class="language-strip" aria-label="Translation languages">
        <label>
          <span>From</span>
          <select bind:value={sourceLanguage} aria-label="Source language">
            {#each installedLanguages as language}
              <option value={language.id}>{languageControlLabel(language)}</option>
            {/each}
          </select>
        </label>
        <span class="arrow">􀄫</span>
        <label>
          <span>To</span>
          <select bind:value={targetLanguage} aria-label="Target language">
            {#each installedLanguages as language}
              <option value={language.id}>{languageControlLabel(language)}</option>
            {/each}
          </select>
        </label>
      </div>

      <button class="record" class:recording={isRecording} on:click={toggleRecording}>
        <span></span>{isRecording ? 'Stop' : 'Record'}
      </button>
    </header>

    <div class="conversation">
      <section class="thread" aria-label="Translation chat">
        <div class="thread-head">
          <div>
            <span class="date-pill">Today</span>
            <h1>Live translation log</h1>
          </div>
          <div class="legend" aria-label="Message lanes">
            <span><i class="speaker-dot"></i>Speaker</span>
            <span><i class="mic-dot"></i>Mic</span>
          </div>
        </div>

        {#if messages.length === 0}
          <div class="starter" aria-live="polite">
            <article class="chat-row speaker-row">
              <div class="chat-bubble incoming">
                <span>Speaker</span>
                <p>The other person’s audio appears here.</p>
                <small>Original text appears first. Translation is shown below it.</small>
              </div>
            </article>
            <article class="chat-row self-row">
              <div class="chat-bubble outgoing">
                <span>Mic</span>
                <p>Your spoken replies appear in the same conversation.</p>
                <small>Both sources stay visible without switching views.</small>
              </div>
            </article>
          </div>
        {:else}
          <div class="messages" aria-live="polite">
            {#each messages as message (message.id)}
              <article class="chat-row" class:self-row={message.role === 'self'}>
                <div class="chat-bubble" class:outgoing={message.role === 'self'} class:incoming={message.role !== 'self'}>
                  <span>{message.role === 'self' ? 'Mic' : 'Speaker'} · {message.language}</span>
                  <p>{message.text}</p>
                  {#if message.translation}
                    <small>{message.translation}</small>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </section>
    </div>

    <footer class="bottom-bar">
      <div class="status">
        <span class:live={isRecording}></span>
        <strong>{status}</strong>
      </div>
      {#if errorMessage}
        <p class="error">{errorMessage}</p>
      {/if}
      <div class="actions">
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
    background: #f2f4f5;
    color: #202124;
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  }

  button,
  select {
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  .stage {
    min-height: 100vh;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(235, 239, 242, 0.96)),
      #eef2f4;
  }

  .window {
    display: grid;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    grid-template-rows: auto 1fr auto;
    border: 0;
    border-radius: 0;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(242, 245, 247, 0.92)),
      #f4f5f5;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.9);
  }

  .toolbar {
    display: grid;
    align-items: center;
    grid-template-columns: 1fr auto auto;
    gap: 14px;
    padding: 18px 22px 14px;
    border-bottom: 1px solid rgba(120, 126, 132, 0.13);
    background: rgba(255, 255, 255, 0.38);
  }

  .identity {
    display: grid;
    min-width: 0;
  }

  .identity strong {
    font-size: 15px;
    letter-spacing: -0.02em;
  }

  .identity span {
    overflow: hidden;
    color: #70757b;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .language-strip,
  .record,
  .bottom-bar,
  .date-pill {
    border: 1px solid rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.62);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.82),
      0 12px 28px rgba(75, 83, 90, 0.12);
  }

  .language-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 18px;
    padding: 6px 8px;
  }

  .language-strip label {
    display: grid;
    gap: 1px;
  }

  .language-strip label span {
    padding-left: 6px;
    color: #8a8f94;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .language-strip select {
    width: 118px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: #2f3337;
    font-size: 13px;
    font-weight: 750;
  }

  .arrow {
    color: #8b9196;
    font-size: 13px;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 9px;
    border-radius: 999px;
    padding: 11px 16px;
    border: 0;
    color: #111;
    font-weight: 800;
  }

  .record span {
    width: 13px;
    height: 13px;
    border: 4px solid #f23b56;
    border-radius: 999px;
  }

  .record.recording {
    background: rgba(255, 240, 242, 0.82);
  }

  .record.recording span {
    border-radius: 4px;
    background: #f23b56;
  }

  .conversation {
    display: grid;
    min-height: 0;
    padding: 18px 22px;
  }

  .thread {
    display: grid;
    min-height: 0;
    grid-template-rows: auto 1fr;
    overflow: hidden;
    border: 1px solid rgba(183, 190, 196, 0.28);
    border-radius: 24px;
    background:
      linear-gradient(180deg, rgba(251, 252, 253, 0.88), rgba(238, 244, 248, 0.82)),
      #f7fafb;
  }

  .thread-head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 22px 8px;
  }

  .thread-head h1 {
    margin: 8px 0 0;
    color: #202124;
    font-size: 22px;
    letter-spacing: -0.04em;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 12px;
    color: #7d8389;
    font-size: 12px;
    font-weight: 800;
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
    background: #8a929a;
  }

  .mic-dot {
    background: #1d8bff;
  }

  .date-pill {
    width: fit-content;
    border-radius: 999px;
    color: #757b81;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 800;
  }

  .starter,
  .messages {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 16px;
    overflow: auto;
    padding: 28px 24px 34px;
  }

  .starter {
    justify-content: center;
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
    max-width: min(600px, 70%);
    border-radius: 24px;
    padding: 13px 15px 12px;
    box-shadow: 0 12px 28px rgba(52, 62, 70, 0.08);
  }

  .chat-bubble.incoming {
    border-bottom-left-radius: 8px;
    background: rgba(255, 255, 255, 0.94);
  }

  .chat-bubble.outgoing {
    border-bottom-right-radius: 8px;
    background: linear-gradient(135deg, #1d8bff, #0472e9);
    color: white;
  }

  .chat-bubble span {
    display: block;
    margin-bottom: 5px;
    opacity: 0.62;
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .chat-bubble p {
    margin: 0;
    font-size: 17px;
    font-weight: 560;
    line-height: 1.42;
    letter-spacing: -0.015em;
  }

  .chat-bubble small {
    display: block;
    margin-top: 8px;
    opacity: 0.7;
    font-size: 14px;
    line-height: 1.35;
  }

  .bottom-bar {
    display: grid;
    align-items: center;
    grid-template-columns: auto 1fr auto;
    gap: 14px;
    margin: 0 22px 18px;
    border-radius: 22px;
    padding: 10px 12px 10px 14px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 9px;
    color: #626970;
    font-size: 13px;
  }

  .status span {
    width: 9px;
    height: 9px;
    border-radius: 999px;
    background: #a8adb2;
  }

  .status span.live {
    background: #30d158;
    box-shadow: 0 0 0 6px rgba(48, 209, 88, 0.12);
  }

  .error {
    overflow: hidden;
    color: #a2382d;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .actions button {
    border: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.66);
    color: #565d64;
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 800;
  }

  @media (max-width: 900px) {
    .toolbar {
      grid-template-columns: 1fr;
    }

    .identity,
    .language-strip,
    .record {
      justify-self: stretch;
    }
  }

  @media (max-width: 640px) {
    .bottom-bar {
      grid-template-columns: 1fr;
    }

    .chat-bubble {
      max-width: 82%;
    }
  }
</style>
