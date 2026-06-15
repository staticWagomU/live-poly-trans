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
  let activeStreams = new Set<'mic' | 'speaker'>();
  let status = 'Ready';
  let errorMessage: string | null = null;
  let savedPath: string | null = null;
  let isStarting = false;

  $: sourceLabel =
    installedLanguages.find((language) => language.id === sourceLanguage)?.label ?? sourceLanguage;
  $: targetLabel =
    installedLanguages.find((language) => language.id === targetLanguage)?.label ?? targetLanguage;
  $: isRecording = activeStreams.size > 0;
  $: isMicRecording = activeStreams.has('mic');
  $: isSpeakerRecording = activeStreams.has('speaker');

  onMount(async () => {
    const unlistenTranscript = await listen<TranscriptEvent>('transcript-event', (event) => {
      applyTranscriptEvent(event.payload);
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

  function applyTranscriptEvent(event: TranscriptEvent) {
    const message = transcriptEventToMessage(event);
    const existingIndex = messages.findIndex((candidate) => candidate.id === message.id);

    if (existingIndex === -1) {
      messages = [...messages, message];
      return;
    }

    messages = messages.map((candidate, index) =>
      index === existingIndex
        ? {
            ...candidate,
            ...message,
            isFinal: candidate.isFinal || message.isFinal
          }
        : candidate
    );
  }

  async function toggleRecording() {
    errorMessage = null;
    savedPath = null;

    if (isRecording) {
      await invoke('stop_all_sessions');
      activeStreams = new Set();
      status = 'Paused';
      return;
    }

    isStarting = true;
    status = 'Starting microphone and speaker';

    const failures: string[] = [];

    for (const stream of ['mic', 'speaker'] as const) {
      try {
        await startStream(stream);
      } catch (error) {
        failures.push(`${stream}: ${String(error)}`);
      }
    }

    isStarting = false;

    if (activeStreams.size === 0) {
      errorMessage = failures.join('\n');
      status = 'Could not start recording';
      return;
    }

    errorMessage = failures.length > 0 ? failures.join('\n') : null;
    status =
      activeStreams.size === 2
        ? 'Listening to mic and speaker'
        : activeStreams.has('mic')
          ? 'Mic is live'
          : 'Speaker is live';
  }

  async function toggleStream(stream: 'mic' | 'speaker') {
    errorMessage = null;
    savedPath = null;

    try {
      if (activeStreams.has(stream)) {
        await invoke('stop_stream_session', { stream });
        activeStreams.delete(stream);
        activeStreams = new Set(activeStreams);
        status = activeStreams.size > 0 ? 'Listening live' : 'Paused';
        return;
      }

      isStarting = true;
      status = stream === 'mic' ? 'Starting mic' : 'Starting speaker';
      await startStream(stream);
      status = stream === 'mic' ? 'Mic is live' : 'Speaker is live';
    } catch (error) {
      errorMessage = String(error);
      status = `Could not start ${stream}`;
    } finally {
      isStarting = false;
    }
  }

  async function startStream(stream: 'mic' | 'speaker') {
    await invoke('start_stream_session', {
      stream,
      sourceLanguage,
      targetLanguage,
      languages: selectedTranscriptionLanguages()
    });
    activeStreams.add(stream);
    activeStreams = new Set(activeStreams);
  }

  function selectedTranscriptionLanguages() {
    return [sourceLanguage, targetLanguage].filter(
      (language, index, languages) => language && languages.indexOf(language) === index
    );
  }

  function clearMessages() {
    messages = [];
    errorMessage = null;
    savedPath = null;
    status = isRecording ? 'Listening live' : 'Ready';
  }

  function transcriptText() {
    return messages
      .map((message) => {
        const translation = message.translation ? `\n  => ${message.translation}` : '';
        return `[${message.timestamp}] ${message.role} / ${message.language}: ${message.text}${translation}`;
      })
      .join('\n');
  }

  async function copyMessages() {
    await navigator.clipboard.writeText(transcriptText());
    status = 'Copied transcript';
  }

  async function saveMessages() {
    const result = await invoke<{ json_path: string; text_path: string }>('save_transcript', {
      messages
    });
    savedPath = result.text_path;
    status = 'Saved transcript';
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<main class="stage">
  <section class="window" aria-label="LivePolyTrans">
    <header class="toolbar" data-tauri-drag-region>
      <div class="toolbar-spacer"></div>

      <div class="toolbar-actions">
        <div class="language-strip" aria-label="Main and sub languages">
          <label>
            <span>Main</span>
            <select bind:value={sourceLanguage} aria-label="Main language">
              {#each installedLanguages as language}
                <option value={language.id}>{languageControlLabel(language)}</option>
              {/each}
            </select>
          </label>
        <span class="arrow">􀄫</span>
          <label>
            <span>Sub</span>
            <select bind:value={targetLanguage} aria-label="Sub language">
              {#each installedLanguages as language}
                <option value={language.id}>{languageControlLabel(language)}</option>
              {/each}
            </select>
          </label>
        </div>

        <button class="record" class:recording={isRecording} disabled={isStarting} on:click={toggleRecording}>
          <span></span>{isStarting ? 'Starting' : isRecording ? 'Stop' : 'Record'}
        </button>
      </div>
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

        {#if messages.length === 0 && isRecording}
          <div class="listening-empty" aria-live="polite">
            <div class="pulse-ring">
              <span></span>
            </div>
            <h2>{isStarting ? 'Preparing audio capture...' : 'Listening for speech'}</h2>
            <p>
              Speak normally. The first words can take a few seconds while Apple Speech warms up.
            </p>
            <div class="stream-chips" aria-label="Active streams">
              <span class:active={isSpeakerRecording}>Speaker</span>
              <span class:active={isMicRecording}>Mic</span>
            </div>
          </div>
        {:else if messages.length === 0}
          <div class="starter" aria-live="polite">
            <article class="chat-row speaker-row">
              <div class="chat-bubble incoming">
                <span>Preview · Speaker</span>
                <p>The other person’s audio will appear here.</p>
                <small>Press Record to start listening.</small>
              </div>
            </article>
            <article class="chat-row self-row">
              <div class="chat-bubble outgoing">
                <span>Preview · Mic</span>
                <p>Your spoken replies will appear in the same conversation.</p>
                <small>This is a preview, not a captured transcript.</small>
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
        <em>{sourceLabel} -> {targetLabel}</em>
      </div>
      {#if errorMessage}
        <p class="error">{errorMessage}</p>
      {:else if savedPath}
        <p class="saved">{savedPath}</p>
      {/if}
      <div class="actions">
        <button class:active={isMicRecording} on:click={() => toggleStream('mic')}>Mic</button>
        <button class:active={isSpeakerRecording} on:click={() => toggleStream('speaker')}>Speaker</button>
        <button on:click={detectLanguages}>Refresh Languages</button>
        <button disabled={messages.length === 0} on:click={copyMessages}>Copy</button>
        <button disabled={messages.length === 0} on:click={saveMessages}>Save</button>
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
    grid-template-columns: 1fr auto;
    gap: 12px;
    padding: 10px 18px;
    border-bottom: 1px solid rgba(120, 126, 132, 0.13);
    background: rgba(255, 255, 255, 0.38);
  }

  .toolbar-spacer {
    min-width: 1px;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-self: end;
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
    border-radius: 999px;
    padding: 5px 10px;
  }

  .language-strip label {
    display: grid;
    gap: 1px;
  }

  .language-strip label span {
    padding-left: 1px;
    color: #8b9298;
    font-size: 9px;
    font-weight: 850;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .language-strip select {
    width: 112px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: #2f3337;
    font-size: 12px;
    font-weight: 750;
  }

  .arrow {
    color: #8b9196;
    font-size: 13px;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 7px;
    border-radius: 999px;
    padding: 9px 13px;
    border: 0;
    color: #111;
    font-size: 13px;
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

  .record:disabled {
    cursor: wait;
    opacity: 0.74;
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
  .listening-empty,
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
    background: rgba(48, 209, 88, 0.10);
    box-shadow: 0 0 0 16px rgba(48, 209, 88, 0.05);
  }

  .pulse-ring span {
    width: 22px;
    height: 22px;
    border-radius: 999px;
    background: #30d158;
    box-shadow: 0 0 0 0 rgba(48, 209, 88, 0.36);
    animation: listening-pulse 1.4s ease-out infinite;
  }

  .listening-empty h2 {
    margin: 22px 0 6px;
    color: #202124;
    font-size: 24px;
    letter-spacing: -0.035em;
  }

  .listening-empty p {
    max-width: 430px;
    margin: 0;
    color: #737b82;
    font-size: 14px;
    font-weight: 650;
    line-height: 1.5;
  }

  .stream-chips {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }

  .stream-chips span {
    border: 1px solid rgba(130, 140, 150, 0.22);
    border-radius: 999px;
    color: #858d95;
    padding: 7px 11px;
    font-size: 12px;
    font-weight: 850;
  }

  .stream-chips span.active {
    border-color: rgba(29, 139, 255, 0.32);
    background: rgba(29, 139, 255, 0.10);
    color: #176fcf;
  }

  @keyframes listening-pulse {
    0% {
      box-shadow: 0 0 0 0 rgba(48, 209, 88, 0.36);
    }

    100% {
      box-shadow: 0 0 0 18px rgba(48, 209, 88, 0);
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
    font-weight: 720;
    line-height: 1.42;
    letter-spacing: -0.015em;
  }

  .chat-bubble small {
    display: block;
    margin-top: 8px;
    opacity: 0.64;
    font-size: 14px;
    font-weight: 520;
    line-height: 1.35;
  }

  .bottom-bar {
    display: grid;
    align-items: center;
    grid-template-columns: auto 1fr auto;
    gap: 14px;
    margin: 0 22px;
    border: 0;
    border-top: 1px solid rgba(120, 126, 132, 0.16);
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    padding: 10px 0 12px;
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

  .status em {
    color: #8a9198;
    font-size: 12px;
    font-style: normal;
  }

  .error {
    max-height: 78px;
    overflow: auto;
    color: #a2382d;
    font-size: 12px;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .saved {
    overflow: hidden;
    color: #4f6d53;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .actions button {
    border: 1px solid rgba(121, 128, 136, 0.34);
    border-radius: 8px;
    background: linear-gradient(180deg, #ffffff, #edf0f2);
    color: #2f353b;
    padding: 7px 11px;
    font-size: 12px;
    font-weight: 800;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.9),
      0 1px 2px rgba(34, 42, 50, 0.08);
  }

  .actions button:active {
    background: linear-gradient(180deg, #dfe4e7, #f7f8f9);
    box-shadow: inset 0 1px 2px rgba(34, 42, 50, 0.12);
  }

  .actions button.active {
    border-color: rgba(29, 139, 255, 0.54);
    color: #075cad;
  }

  .actions button:disabled {
    cursor: default;
    opacity: 0.45;
  }

  @media (max-width: 900px) {
    .toolbar {
      grid-template-columns: 1fr;
    }

    .toolbar-actions {
      display: grid;
      justify-self: stretch;
    }

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
