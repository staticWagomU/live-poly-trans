<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import {
    chooseDefaultLanguagePair,
    languageControlLabel,
    transcriptionLanguagesForStream,
    type LanguageInfo
  } from '$lib/languages';
  import {
    captureModeFromStreams,
    streamsForCaptureMode,
    type AudioStream,
    type CaptureMode
  } from '$lib/audioMode';
  import { isScrolledToBottom } from '$lib/scroll';
  import { isCompetingTranscriptCandidate, mergeTranscriptMessages } from '$lib/transcriptSelection';
  import { transcriptEventToMessage, type ChatMessage, type TranscriptEvent } from '$lib/transcripts';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
  };

  const captureModeOptions: Array<{ mode: CaptureMode; label: string }> = [
    { mode: 'mic', label: 'Mic' },
    { mode: 'both', label: 'Both' },
    { mode: 'speaker', label: 'Speaker' }
  ];

  let sourceLanguage = 'en-US';
  let targetLanguage = 'ja-JP';
  let installedLanguages: LanguageInfo[] = [
    { id: 'en-US', label: 'English' },
    { id: 'ja-JP', label: 'Japanese' }
  ];
  let messages: ChatMessage[] = [];
  let captureMode: CaptureMode = 'both';
  let activeStreams = new Set<AudioStream>();
  let streamSessionIds: Record<AudioStream, string | null> = {
    mic: null,
    speaker: null
  };
  let errorMessage: string | null = null;
  let savedPath: string | null = null;
  let isStarting = false;
  let messagesContainer: HTMLDivElement | null = null;
  let latestMessageAnchor: HTMLDivElement | null = null;
  let showJumpToLatest = false;
  let aiSummary = '';
  let aiQuestions = '';
  let aiQuestion = '';
  let aiAnswer = '';
  let aiError: string | null = null;
  let isSummaryLoading = false;
  let isQuestionsLoading = false;
  let isAnswerLoading = false;
  let summaryRefreshTimer: ReturnType<typeof setTimeout> | null = null;

  const summaryRefreshDelayMs = 6000;

  $: isRecording = activeStreams.size > 0;
  $: isMicRecording = activeStreams.has('mic');
  $: isSpeakerRecording = activeStreams.has('speaker');
  $: selectedCaptureMode = isRecording ? captureModeFromStreams(activeStreams) : captureMode;

  onMount(async () => {
    const unlistenTranscript = await listen<TranscriptEvent>('transcript-event', (event) => {
      if (!isCurrentTranscriptEvent(event.payload)) {
        return;
      }

      void applyTranscriptEvent(event.payload);
    });

    const unlistenError = await listen<string>('helper-error', (event) => {
      errorMessage = event.payload;
    });

    await detectLanguages();

    return () => {
      unlistenTranscript();
      unlistenError();
      if (summaryRefreshTimer) {
        clearTimeout(summaryRefreshTimer);
      }
    };
  });

  async function detectLanguages() {
    try {
      const payload = await invoke<LanguageDetectionPayload>('detect_languages');
      installedLanguages = payload.installed.length > 0 ? payload.installed : installedLanguages;
      const pair = chooseDefaultLanguagePair(installedLanguages);
      sourceLanguage = pair.source;
      targetLanguage = pair.target;
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function applyTranscriptEvent(event: TranscriptEvent) {
    const shouldScrollToLatest = shouldStickToLatest();
    const message = transcriptEventToMessage(event);
    const existingIndex = messages.findIndex(
      (candidate) => candidate.id === message.id || isLikelySameUtterance(candidate, message)
    );

    if (existingIndex === -1) {
      messages = [...messages, message];
    } else {
      messages = messages.map((candidate, index) =>
        index === existingIndex
          ? mergeTranscriptMessages(candidate, message)
          : candidate
      );
    }

    await tick();

    if (shouldScrollToLatest) {
      scrollToLatest('auto');
      if (event.isFinal) {
        scheduleSummaryRefresh();
      }
      return;
    }

    syncJumpToLatestButton();
    if (event.isFinal) {
      scheduleSummaryRefresh();
    }
  }

  function isLikelySameUtterance(candidate: ChatMessage, message: ChatMessage) {
    return isCompetingTranscriptCandidate(candidate, message);
  }

  function isCurrentTranscriptEvent(event: TranscriptEvent) {
    return streamSessionIds[event.stream] === event.sessionId;
  }

  function createStreamSessionId(stream: AudioStream) {
    return `${stream}-${Date.now()}-${crypto.randomUUID()}`;
  }

  function setStreamSession(stream: AudioStream, sessionId: string | null) {
    streamSessionIds = {
      ...streamSessionIds,
      [stream]: sessionId
    };
  }

  function clearStreamSessions(streams: AudioStream[]) {
    const nextSessionIds = { ...streamSessionIds };
    for (const stream of streams) {
      nextSessionIds[stream] = null;
    }
    streamSessionIds = nextSessionIds;
  }

  async function toggleRecording() {
    errorMessage = null;
    savedPath = null;

    if (isRecording) {
      clearStreamSessions([...activeStreams]);
      activeStreams = new Set();
      await invoke('stop_all_sessions');
      return;
    }

    isStarting = true;

    const failures: string[] = [];

    for (const stream of streamsForCaptureMode(captureMode)) {
      try {
        await startStream(stream);
      } catch (error) {
        failures.push(`${stream}: ${String(error)}`);
      }
    }

    isStarting = false;

    if (activeStreams.size === 0) {
      errorMessage = failures.join('\n');
      return;
    }

    errorMessage = failures.length > 0 ? failures.join('\n') : null;
  }

  async function selectCaptureMode(mode: CaptureMode) {
    errorMessage = null;
    savedPath = null;
    captureMode = mode;

    if (!isRecording) {
      return;
    }

    isStarting = true;

    try {
      const nextStreams = new Set(streamsForCaptureMode(mode));
      const currentStreams = new Set(activeStreams);

      for (const stream of nextStreams) {
        if (!currentStreams.has(stream)) {
          await startStream(stream);
        }
      }

      for (const stream of currentStreams) {
        if (!nextStreams.has(stream)) {
          await stopStream(stream);
        }
      }
    } catch (error) {
      errorMessage = String(error);
    } finally {
      isStarting = false;
    }
  }

  async function startStream(stream: AudioStream) {
    const sessionId = createStreamSessionId(stream);
    setStreamSession(stream, sessionId);

    try {
      await invoke('start_stream_session', {
        stream,
        sourceLanguage,
        targetLanguage,
        languages: selectedTranscriptionLanguages(stream),
        sessionId
      });
    } catch (error) {
      if (streamSessionIds[stream] === sessionId) {
        setStreamSession(stream, null);
      }
      throw error;
    }

    activeStreams.add(stream);
    activeStreams = new Set(activeStreams);
  }

  async function stopStream(stream: AudioStream) {
    setStreamSession(stream, null);
    activeStreams.delete(stream);
    activeStreams = new Set(activeStreams);
    await invoke('stop_stream_session', { stream });
  }

  function selectedTranscriptionLanguages(stream: AudioStream) {
    return transcriptionLanguagesForStream(stream, sourceLanguage, targetLanguage);
  }

  function clearMessages() {
    messages = [];
    errorMessage = null;
    savedPath = null;
    showJumpToLatest = false;
    aiSummary = '';
    aiQuestions = '';
    aiQuestion = '';
    aiAnswer = '';
    aiError = null;
    if (summaryRefreshTimer) {
      clearTimeout(summaryRefreshTimer);
      summaryRefreshTimer = null;
    }
    void invoke('clear_meeting_ai_context').catch((error) => {
      aiError = String(error);
    });
  }

  function shouldStickToLatest() {
    if (!messagesContainer) {
      return true;
    }

    return isScrolledToBottom({
      scrollTop: messagesContainer.scrollTop,
      clientHeight: messagesContainer.clientHeight,
      scrollHeight: messagesContainer.scrollHeight
    });
  }

  function syncJumpToLatestButton() {
    showJumpToLatest = !!messagesContainer && !shouldStickToLatest();
  }

  function handleMessagesScroll() {
    syncJumpToLatestButton();
  }

  function scrollToLatest(behavior: ScrollBehavior = 'smooth') {
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

  function transcriptText() {
    return messages
      .map((message) => {
        const translation = message.translation ? `\n  => ${message.translation}` : '';
        return `[${message.timestamp}] ${message.speakerLabel} / ${message.language}: ${message.text}${translation}`;
      })
      .join('\n');
  }

  async function copyMessages() {
    await navigator.clipboard.writeText(transcriptText());
  }

  async function saveMessages() {
    const result = await invoke<{ json_path: string; text_path: string }>('save_transcript', {
      messages
    });
    savedPath = result.text_path;
  }

  function scheduleSummaryRefresh() {
    if (summaryRefreshTimer) {
      clearTimeout(summaryRefreshTimer);
    }

    summaryRefreshTimer = setTimeout(() => {
      void generateMeetingSummary(true);
    }, summaryRefreshDelayMs);
  }

  async function generateMeetingSummary(automatic = false) {
    if (isSummaryLoading || (automatic && messages.length === 0)) {
      return;
    }

    isSummaryLoading = true;
    aiError = null;

    try {
      aiSummary = await invoke<string>('ai_generate_summary', { messages });
    } catch (error) {
      if (!automatic) {
        aiError = String(error);
      }
    } finally {
      isSummaryLoading = false;
    }
  }

  async function suggestQuestions() {
    isQuestionsLoading = true;
    aiError = null;

    try {
      aiQuestions = await invoke<string>('ai_suggest_questions', { messages });
    } catch (error) {
      aiError = String(error);
    } finally {
      isQuestionsLoading = false;
    }
  }

  async function askMeetingQuestion() {
    if (!aiQuestion.trim()) {
      return;
    }

    isAnswerLoading = true;
    aiError = null;

    try {
      aiAnswer = await invoke<string>('ai_ask', { question: aiQuestion, messages });
    } catch (error) {
      aiError = String(error);
    } finally {
      isAnswerLoading = false;
    }
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<main class="stage">
  <section class="window" aria-label="LivePolyTrans">
    <header class="toolbar" data-tauri-drag-region>
      <div class="capture-switch" data-mode={selectedCaptureMode} aria-label="Audio capture mode">
        {#each captureModeOptions as option}
          <button
            type="button"
            class:active={selectedCaptureMode === option.mode}
            aria-pressed={selectedCaptureMode === option.mode}
            disabled={isStarting}
            on:click={() => selectCaptureMode(option.mode)}
          >
            {option.label}
          </button>
        {/each}
      </div>

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
            <span><i class="mic-dot"></i>Speaker A</span>
            <span><i class="speaker-dot"></i>Speaker B</span>
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
              {#each messages as message (message.id)}
                <article class="chat-row" class:self-row={message.role === 'self'}>
                  <div class="chat-bubble" class:outgoing={message.role === 'self'} class:incoming={message.role !== 'self'}>
                    <span>{message.speakerLabel} · {message.language}</span>
                    <p>{message.text}</p>
                    {#if message.translation}
                      <small>{message.translation}</small>
                    {/if}
                  </div>
                </article>
              {/each}
              <div class="messages-end-anchor" bind:this={latestMessageAnchor} aria-hidden="true"></div>
            </div>

            {#if showJumpToLatest}
              <button type="button" class="jump-to-latest" on:click={scrollToLatest}>
                Jump to latest
              </button>
            {/if}
          </div>
        {/if}
      </section>

      <aside class="meeting-ai" aria-label="Meeting AI">
        <div class="ai-head">
          <div>
            <span class="date-pill">AI</span>
            <h2>Meeting</h2>
          </div>
          <button
            type="button"
            disabled={isSummaryLoading}
            on:click={() => generateMeetingSummary(false)}
          >
            {isSummaryLoading ? 'Updating' : 'Refresh'}
          </button>
        </div>

        {#if aiError}
          <p class="ai-error">{aiError}</p>
        {/if}

        <section class="ai-section">
          <div class="ai-section-head">
            <h3>Summary</h3>
          </div>
          <pre>{aiSummary || 'No summary yet.'}</pre>
        </section>

        <section class="ai-section">
          <div class="ai-section-head">
            <h3>Questions</h3>
            <button type="button" disabled={isQuestionsLoading} on:click={suggestQuestions}>
              {isQuestionsLoading ? 'Updating' : 'Suggest'}
            </button>
          </div>
          <pre>{aiQuestions || 'No questions yet.'}</pre>
        </section>

        <section class="ai-section ask-section">
          <div class="ai-section-head">
            <h3>Ask</h3>
            <button type="button" disabled={isAnswerLoading || !aiQuestion.trim()} on:click={askMeetingQuestion}>
              {isAnswerLoading ? 'Asking' : 'Ask'}
            </button>
          </div>
          <textarea bind:value={aiQuestion} rows="3" aria-label="Meeting question"></textarea>
          <pre>{aiAnswer || 'No answer yet.'}</pre>
        </section>
      </aside>
    </div>

    <footer class="bottom-bar">
      {#if errorMessage}
        <p class="error">{errorMessage}</p>
      {:else if savedPath}
        <p class="saved">{savedPath}</p>
      {/if}
      <div class="actions">
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
    grid-template-columns: auto 1fr;
    gap: 12px;
    padding: 10px 18px;
    border-bottom: 1px solid rgba(120, 126, 132, 0.13);
    background: rgba(255, 255, 255, 0.38);
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-self: end;
  }

  .language-strip,
  .capture-switch,
  .record,
  .bottom-bar,
  .date-pill {
    border: 1px solid rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.62);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.82),
      0 12px 28px rgba(75, 83, 90, 0.12);
  }

  .capture-switch {
    position: relative;
    display: grid;
    width: 246px;
    grid-template-columns: repeat(3, 1fr);
    isolation: isolate;
    overflow: hidden;
    border-radius: 999px;
    padding: 3px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.70), rgba(232, 236, 239, 0.62)),
      rgba(255, 255, 255, 0.62);
  }

  .capture-switch::before {
    position: absolute;
    z-index: 0;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / 3);
    border-radius: 999px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(245, 248, 250, 0.88)),
      #ffffff;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.96),
      0 7px 18px rgba(29, 139, 255, 0.16),
      0 1px 3px rgba(34, 42, 50, 0.12);
    content: '';
    transform: translateX(var(--capture-pill-x, 0%));
    transition:
      transform 260ms cubic-bezier(0.22, 1, 0.36, 1),
      box-shadow 260ms ease,
      background 260ms ease;
  }

  .capture-switch[data-mode='both'] {
    --capture-pill-x: 100%;
  }

  .capture-switch[data-mode='speaker'] {
    --capture-pill-x: 200%;
  }

  .capture-switch button {
    position: relative;
    z-index: 1;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: #717980;
    padding: 7px 11px;
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.01em;
    transition:
      color 180ms ease,
      transform 180ms ease,
      opacity 180ms ease;
  }

  .capture-switch button:hover:not(:disabled) {
    color: #30363c;
    transform: translateY(-1px);
  }

  .capture-switch button.active {
    color: #075cad;
  }

  .capture-switch button:disabled {
    cursor: wait;
    opacity: 0.58;
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
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 16px;
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

  .meeting-ai {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-rows: auto auto minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1.15fr);
    gap: 12px;
    overflow: hidden;
    border: 1px solid rgba(183, 190, 196, 0.28);
    border-radius: 24px;
    background:
      linear-gradient(180deg, rgba(251, 252, 253, 0.90), rgba(238, 244, 248, 0.84)),
      #f7fafb;
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
    color: #202124;
    font-size: 20px;
    letter-spacing: 0;
  }

  .ai-head button,
  .ai-section-head button {
    border: 1px solid rgba(121, 128, 136, 0.34);
    border-radius: 8px;
    background: linear-gradient(180deg, #ffffff, #edf0f2);
    color: #2f353b;
    padding: 7px 10px;
    font-size: 12px;
    font-weight: 800;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.9),
      0 1px 2px rgba(34, 42, 50, 0.08);
  }

  .ai-head button:disabled,
  .ai-section-head button:disabled {
    cursor: wait;
    opacity: 0.56;
  }

  .ai-error {
    max-height: 58px;
    overflow: auto;
    margin: 0;
    color: #a2382d;
    font-size: 12px;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ai-section {
    display: grid;
    min-height: 0;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 8px;
    overflow: hidden;
    border-top: 1px solid rgba(120, 126, 132, 0.14);
    padding-top: 12px;
  }

  .ai-section h3 {
    margin: 0;
    color: #4b535a;
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .ai-section pre {
    min-height: 0;
    overflow: auto;
    margin: 0;
    color: #283038;
    font-family: inherit;
    font-size: 13px;
    font-weight: 620;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .ask-section {
    grid-template-rows: auto auto minmax(0, 1fr);
  }

  .ask-section textarea {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    resize: none;
    border: 1px solid rgba(121, 128, 136, 0.28);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.82);
    color: #202124;
    padding: 9px 10px;
    font: inherit;
    font-size: 13px;
    line-height: 1.35;
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
    border: 1px solid rgba(29, 139, 255, 0.2);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.94);
    color: #075cad;
    padding: 10px 14px;
    font-size: 12px;
    font-weight: 850;
    box-shadow:
      0 12px 24px rgba(52, 62, 70, 0.16),
      inset 0 1px 0 rgba(255, 255, 255, 0.92);
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
    grid-template-columns: 1fr auto;
    gap: 14px;
    margin: 0 22px;
    border: 0;
    border-top: 1px solid rgba(120, 126, 132, 0.16);
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    padding: 10px 0 12px;
  }

  @media (max-width: 980px) {
    .conversation {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) minmax(280px, 38vh);
    }
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
    .capture-switch,
    .record {
      justify-self: stretch;
    }

    .capture-switch {
      width: auto;
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
