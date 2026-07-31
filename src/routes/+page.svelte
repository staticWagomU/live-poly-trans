<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import {
    chooseDefaultLanguagePair,
    languageControlLabel,
    transcriptionCandidateLanguages,
    updateLanguagePair,
    type LanguageInfo
  } from '$lib/languages';
  import {
    captureModeFromStreams,
    streamsForCaptureMode,
    type AudioStream,
    type CaptureMode
  } from '$lib/audioMode';
  import { isScrolledToBottom } from '$lib/scroll';
  import { displayTranscriptMessage } from '$lib/transcriptDisplay';
  import {
    DEFAULT_TRANSCRIPT_FONT_SCALE,
    canDecreaseTranscriptFontScale,
    canIncreaseTranscriptFontScale,
    decreaseTranscriptFontScale,
    increaseTranscriptFontScale,
    parseTranscriptFontScale
  } from '$lib/transcriptFontSize';
  import { applyTranscriptMessage } from '$lib/transcriptInterim';
  import {
    applyTranslationEvent,
    transcriptEventToMessage,
    type ChatMessage,
    type HelperEvent,
    type StatusEvent,
    type TranscriptEvent
  } from '$lib/transcripts';
  import {
    boundMessagesByChars,
    planSummaryRequest,
    recentChatHistory,
    type ChatTurn
  } from '$lib/aiContext';
  import RecordingsView from '$lib/RecordingsView.svelte';
  import SettingsView from '$lib/SettingsView.svelte';
  import {
    parseSpeechModelPreference,
    speechModelPreferenceValue,
    streamEnginePayload,
    type SpeechModelSelection
  } from '$lib/speechModels';
  import { createAsyncCleanupRegistry } from '$lib/asyncCleanup';
  import { completeCaptureStop } from '$lib/captureLifecycle';

  type LanguageDetectionPayload = {
    installed: LanguageInfo[];
    supported: LanguageInfo[];
  };

  type CreatedRecording = {
    id: string;
    dir: string;
  };

  type HelperExitedPayload = {
    stream: AudioStream;
    sessionId: string;
    code: number | null;
  };

  const captureModeOptions: Array<{ mode: CaptureMode; label: string }> = [
    { mode: 'mic', label: 'Mic' },
    { mode: 'both', label: 'Both' },
    { mode: 'speaker', label: 'Speaker' }
  ];

  const recordingPreferenceKey = 'lpt-save-audio';
  const fontScalePreferenceKey = 'lpt-transcript-font-scale';
  const speechModelPreferenceKey = 'lpt-speech-model';
  const summaryRefreshDelayMs = 6000;
  const maxRestartAttempts = 3;

  let activeTab: 'live' | 'recordings' | 'settings' = 'live';
  let mainLanguage = 'en-US';
  let subLanguage = 'ja-JP';
  let installedLanguages: LanguageInfo[] = [
    { id: 'en-US', label: 'English' },
    { id: 'ja-JP', label: 'Japanese' }
  ];
  let messages: ChatMessage[] = [];
  let interimMessages: ChatMessage[] = [];
  let captureMode: CaptureMode = 'both';
  let activeStreams = new Set<AudioStream>();
  let streamSessionIds: Record<AudioStream, string | null> = {
    mic: null,
    speaker: null
  };
  let restartAttempts: Record<AudioStream, number> = { mic: 0, speaker: 0 };
  let recordingEnabled = false;
  let currentRecording: CreatedRecording | null = null;
  let captureTransition: 'starting' | 'stopping' | 'switching' | null = null;
  let messagesContainer: HTMLDivElement | null = null;
  let latestMessageAnchor: HTMLDivElement | null = null;
  let showJumpToLatest = false;
  let statusMessage: string | null = null;
  let actionNotice: string | null = null;
  let actionNoticeTimer: ReturnType<typeof setTimeout> | null = null;
  let aiSummary = '';
  let summaryCoveredCount = 0;
  let summaryError: string | null = null;
  let aiQuestion = '';
  let chatTurns: ChatTurn[] = [];
  let aiError: string | null = null;
  let isSummaryLoading = false;
  let isAnswerLoading = false;
  let summaryRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  let transcriptFontScale = DEFAULT_TRANSCRIPT_FONT_SCALE;
  let speechModel: SpeechModelSelection = { engine: 'builtin' };

  $: isRecording = activeStreams.size > 0;
  $: isMicRecording = activeStreams.has('mic');
  $: isSpeakerRecording = activeStreams.has('speaker');
  $: isCaptureBusy = captureTransition !== null;
  $: isStarting = captureTransition === 'starting';
  $: selectedCaptureMode = isRecording ? captureModeFromStreams(activeStreams) : captureMode;
  $: visibleMessages = [...messages, ...interimMessages];

  onMount(() => {
    recordingEnabled = localStorage.getItem(recordingPreferenceKey) === '1';
    transcriptFontScale = parseTranscriptFontScale(localStorage.getItem(fontScalePreferenceKey));
    speechModel = parseSpeechModelPreference(localStorage.getItem(speechModelPreferenceKey));

    const cleanupRegistry = createAsyncCleanupRegistry((error) => {
      console.error('Failed to remove an app event listener', error);
    });

    void Promise.all([
      cleanupRegistry.add(
        listen<HelperEvent>('transcript-event', (event) => {
          void handleHelperEvent(event.payload);
        })
      ),
      cleanupRegistry.add(
        listen<string>('helper-error', (event) => {
          aiError = event.payload;
        })
      ),
      cleanupRegistry.add(
        listen<HelperExitedPayload>('helper-exited', (event) => {
          void handleHelperExit(event.payload);
        })
      )
    ])
      .then(() => {
        if (!cleanupRegistry.isDisposed()) {
          return detectLanguages();
        }
      })
      .catch((error) => {
        if (!cleanupRegistry.isDisposed()) {
          aiError = `Could not initialize the app: ${String(error)}`;
        }
      });

    return () => {
      cleanupRegistry.dispose();
      if (summaryRefreshTimer) {
        clearTimeout(summaryRefreshTimer);
      }
      if (actionNoticeTimer) {
        clearTimeout(actionNoticeTimer);
      }
    };
  });

  async function detectLanguages(preserveSelection = false) {
    try {
      const payload = await invoke<LanguageDetectionPayload>('detect_languages');
      applyInstalledLanguages(payload.installed, preserveSelection);
    } catch (error) {
      aiError = String(error);
    }
  }

  function applyInstalledLanguages(installed: LanguageInfo[], preserveSelection = true) {
    installedLanguages = installed.length > 0 ? installed : installedLanguages;

    const stillInstalled = (id: string) =>
      installedLanguages.some((language) => language.id === id);
    if (preserveSelection && stillInstalled(mainLanguage) && stillInstalled(subLanguage)) {
      return;
    }

    const pair = chooseDefaultLanguagePair(installedLanguages, navigator.language);
    mainLanguage = pair.source;
    subLanguage = pair.target;
  }

  async function handleHelperEvent(payload: HelperEvent) {
    if (!isCurrentSessionEvent(payload)) {
      return;
    }

    if (payload.type === 'transcript') {
      await applyTranscriptEvent(payload);
    } else if (payload.type === 'translation') {
      messages = applyTranslationEvent(messages, payload);
    } else if (payload.type === 'status') {
      handleStatusEvent(payload);
    }
  }

  function handleStatusEvent(event: StatusEvent) {
    if (event.state === 'downloading-language') {
      statusMessage = `Downloading ${event.lang ?? ''} speech model…`;
    } else if (event.state === 'language-ready') {
      statusMessage = null;
    }
  }

  async function applyTranscriptEvent(event: TranscriptEvent) {
    const shouldScrollToLatest = shouldStickToLatest();
    const message = transcriptEventToMessage(event);
    const nextState = applyTranscriptMessage({ messages, interimMessages }, message);
    messages = nextState.messages;
    interimMessages = nextState.interimMessages;

    await tick();

    if (shouldScrollToLatest) {
      scrollToLatest('auto');
    } else {
      syncJumpToLatestButton();
    }

    if (event.isFinal) {
      scheduleSummaryRefresh();
    }
  }

  function isCurrentSessionEvent(event: HelperEvent) {
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

  function setTranscriptFontScale(scale: number) {
    transcriptFontScale = scale;
    localStorage.setItem(fontScalePreferenceKey, String(scale));
  }

  function setRecordingEnabled(enabled: boolean) {
    recordingEnabled = enabled;
    localStorage.setItem(recordingPreferenceKey, enabled ? '1' : '0');
  }

  function setSpeechModel(selection: SpeechModelSelection) {
    speechModel = selection;
    localStorage.setItem(speechModelPreferenceKey, speechModelPreferenceValue(selection));
  }

  async function toggleRecording() {
    if (isCaptureBusy) {
      return;
    }

    aiError = null;
    statusMessage = null;

    if (isRecording) {
      captureTransition = 'stopping';
      clearStreamSessions([...activeStreams]);
      activeStreams = new Set();
      interimMessages = [];
      const stopErrors = await completeCaptureStop(
        () => invoke('stop_all_sessions'),
        finishCurrentRecording
      );
      if (stopErrors.length > 0) {
        aiError = `Could not fully stop capture:\n${stopErrors.map(String).join('\n')}`;
      }
      captureTransition = null;
      return;
    }

    captureTransition = 'starting';
    try {
      restartAttempts = { mic: 0, speaker: 0 };
      const failures: string[] = [];

      if (recordingEnabled) {
        try {
          currentRecording = await invoke<CreatedRecording>('create_recording');
        } catch (error) {
          failures.push(`audio recording: ${String(error)}`);
          currentRecording = null;
        }
      }

      for (const stream of streamsForCaptureMode(captureMode)) {
        try {
          await startStream(stream);
        } catch (error) {
          failures.push(`${stream}: ${String(error)}`);
        }
      }

      if (activeStreams.size === 0) {
        try {
          await finishCurrentRecording();
        } catch (error) {
          failures.push(`audio recording finalize: ${String(error)}`);
        }
        aiError = failures.join('\n');
        return;
      }

      aiError = failures.length > 0 ? failures.join('\n') : null;
    } finally {
      captureTransition = null;
    }
  }

  async function finishCurrentRecording() {
    const recording = currentRecording;
    currentRecording = null;
    if (!recording) {
      return;
    }

    await invoke('finalize_recording', { id: recording.id });
  }

  async function selectCaptureMode(mode: CaptureMode) {
    if (isCaptureBusy) {
      return;
    }

    aiError = null;
    captureMode = mode;

    if (!isRecording) {
      return;
    }

    captureTransition = 'switching';

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
      aiError = String(error);
    } finally {
      captureTransition = null;
    }
  }

  async function startStream(stream: AudioStream) {
    const sessionId = createStreamSessionId(stream);
    setStreamSession(stream, sessionId);

    try {
      await invoke('start_stream_session', {
        stream,
        sourceLanguage: mainLanguage,
        targetLanguage: subLanguage,
        languages: selectedTranscriptionLanguages(),
        sessionId,
        recordingDir: currentRecording?.dir ?? null,
        ...streamEnginePayload(speechModel)
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

  async function handleHelperExit(payload: HelperExitedPayload) {
    if (streamSessionIds[payload.stream] !== payload.sessionId) {
      return;
    }

    setStreamSession(payload.stream, null);
    activeStreams.delete(payload.stream);
    activeStreams = new Set(activeStreams);

    const attempt = restartAttempts[payload.stream] + 1;
    if (attempt > maxRestartAttempts) {
      aiError = `${payload.stream} capture stopped unexpectedly (code ${payload.code ?? '?'}) and automatic restart gave up.`;
      statusMessage = null;
      return;
    }

    restartAttempts = { ...restartAttempts, [payload.stream]: attempt };
    statusMessage = `Restarting ${payload.stream} capture (attempt ${attempt}/${maxRestartAttempts})…`;

    await new Promise((resolve) => setTimeout(resolve, attempt * 1000));

    const stillWanted =
      streamsForCaptureMode(captureMode).includes(payload.stream) &&
      streamSessionIds[payload.stream] === null;
    if (!stillWanted) {
      statusMessage = null;
      return;
    }

    try {
      await startStream(payload.stream);
      statusMessage = null;
    } catch (error) {
      statusMessage = null;
      aiError = `Restarting ${payload.stream} capture failed: ${String(error)}`;
    }
  }

  function selectedTranscriptionLanguages() {
    return transcriptionCandidateLanguages(mainLanguage, subLanguage);
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

  function scheduleSummaryRefresh() {
    if (summaryRefreshTimer) {
      clearTimeout(summaryRefreshTimer);
    }

    summaryRefreshTimer = setTimeout(() => {
      void generateMeetingSummary(true);
    }, summaryRefreshDelayMs);
  }

  async function generateMeetingSummary(automatic = false) {
    if (isSummaryLoading) {
      return;
    }

    const plan = planSummaryRequest(messages, summaryCoveredCount, aiSummary);
    if (!plan) {
      return;
    }

    isSummaryLoading = true;
    if (!automatic) {
      aiError = null;
    }

    try {
      aiSummary = await invoke<string>('ai_generate_summary', {
        messages: plan.messages,
        sourceLanguage: mainLanguage,
        previousSummary: plan.previousSummary
      });
      summaryCoveredCount = plan.coveredCount;
      summaryError = null;
    } catch (error) {
      summaryError = String(error);
    } finally {
      isSummaryLoading = false;
    }
  }

  async function askMeetingQuestion() {
    const question = aiQuestion.trim();
    if (!question || isAnswerLoading) {
      return;
    }

    isAnswerLoading = true;
    aiError = null;
    const history = recentChatHistory(chatTurns);
    const bounded = boundMessagesByChars(messages, 6000);
    chatTurns = [...chatTurns, { question, answer: '' }];
    aiQuestion = '';

    try {
      const answer = await invoke<string>('ai_ask', {
        question,
        messages: bounded.kept,
        language: mainLanguage,
        history
      });
      chatTurns = chatTurns.map((turn, index) =>
        index === chatTurns.length - 1 ? { ...turn, answer } : turn
      );
    } catch (error) {
      aiError = String(error);
      chatTurns = chatTurns.slice(0, -1);
      aiQuestion = question;
    } finally {
      isAnswerLoading = false;
    }
  }

  function showActionNotice(text: string) {
    actionNotice = text;
    if (actionNoticeTimer) {
      clearTimeout(actionNoticeTimer);
    }
    actionNoticeTimer = setTimeout(() => {
      actionNotice = null;
    }, 5000);
  }

  async function copyTranscript() {
    const text = messages
      .map((message) => {
        const translation = message.translation ? `\n  => ${message.translation}` : '';
        return `[${message.timestamp}] ${message.speakerLabel} / ${message.language}: ${message.text}${translation}`;
      })
      .join('\n');

    try {
      await navigator.clipboard.writeText(text);
      showActionNotice('Transcript copied.');
    } catch (error) {
      aiError = String(error);
    }
  }

  async function saveTranscript() {
    if (messages.length === 0) {
      return;
    }

    try {
      const result = await invoke<{ json_path: string; text_path: string }>('save_transcript', {
        messages
      });
      showActionNotice(`Saved: ${result.json_path}`);
    } catch (error) {
      aiError = String(error);
    }
  }

  function clearConversation() {
    messages = [];
    interimMessages = [];
    aiSummary = '';
    summaryCoveredCount = 0;
    summaryError = null;
    chatTurns = [];
    aiError = null;
    actionNotice = null;
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<main class="stage">
  <section class="window" aria-label="LivePolyTrans">
    <header class="toolbar" data-tauri-drag-region>
      <div class="toolbar-lead">
        <div class="tab-switch" role="tablist" aria-label="View">
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'live'}
            class:active={activeTab === 'live'}
            on:click={() => (activeTab = 'live')}
          >
            Live
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'recordings'}
            class:active={activeTab === 'recordings'}
            on:click={() => (activeTab = 'recordings')}
          >
            Recordings
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'settings'}
            class:active={activeTab === 'settings'}
            on:click={() => (activeTab = 'settings')}
          >
            Settings
          </button>
        </div>

        <div class="capture-switch" data-mode={selectedCaptureMode} aria-label="Audio capture mode">
          {#each captureModeOptions as option}
            <button
              type="button"
              class:active={selectedCaptureMode === option.mode}
              aria-pressed={selectedCaptureMode === option.mode}
              disabled={isCaptureBusy}
              on:click={() => selectCaptureMode(option.mode)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="toolbar-actions">
        <div class="language-strip" aria-label="Main and sub languages">
          <label>
            <span>Main</span>
            <select
              value={mainLanguage}
              aria-label="Main language"
              disabled={isRecording || isCaptureBusy}
              on:change={(event) => {
                const pair = updateLanguagePair(
                  { source: mainLanguage, target: subLanguage },
                  'source',
                  event.currentTarget.value
                );
                mainLanguage = pair.source;
                subLanguage = pair.target;
              }}
            >
              {#each installedLanguages as language}
                <option value={language.id}>{languageControlLabel(language)}</option>
              {/each}
            </select>
          </label>
          <span class="arrow">􀄫</span>
          <label>
            <span>Sub</span>
            <select
              value={subLanguage}
              aria-label="Sub language"
              disabled={isRecording || isCaptureBusy}
              on:change={(event) => {
                const pair = updateLanguagePair(
                  { source: mainLanguage, target: subLanguage },
                  'target',
                  event.currentTarget.value
                );
                mainLanguage = pair.source;
                subLanguage = pair.target;
              }}
            >
              {#each installedLanguages as language}
                <option value={language.id}>{languageControlLabel(language)}</option>
              {/each}
            </select>
          </label>
          <button
            type="button"
            class="refresh-languages"
            title="Refresh installed languages"
            aria-label="Refresh installed languages"
            disabled={isRecording || isCaptureBusy}
            on:click={() => detectLanguages(true)}
          >
            ↻
          </button>
        </div>

        <label class="record-toggle" title="Save mic and speaker audio files while transcribing">
          <input
            type="checkbox"
            checked={recordingEnabled}
            disabled={isRecording}
            on:change={(event) => setRecordingEnabled(event.currentTarget.checked)}
          />
          <span>Save audio</span>
        </label>

        <button
          class="record"
          class:recording={isRecording || captureTransition === 'stopping'}
          disabled={isCaptureBusy}
          on:click={toggleRecording}
        >
          <span></span>{captureTransition === 'starting'
            ? 'Starting'
            : captureTransition === 'stopping'
              ? 'Stopping'
              : isRecording
                ? 'Stop'
                : 'Record'}
        </button>
      </div>
    </header>

    {#if activeTab === 'live'}
      <div class="conversation">
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
                  on:click={() =>
                    setTranscriptFontScale(decreaseTranscriptFontScale(transcriptFontScale))}
                >
                  A−
                </button>
                <button
                  type="button"
                  class="font-size-button"
                  title="Increase transcript text size"
                  aria-label="Increase transcript text size"
                  disabled={!canIncreaseTranscriptFontScale(transcriptFontScale)}
                  on:click={() =>
                    setTranscriptFontScale(increaseTranscriptFontScale(transcriptFontScale))}
                >
                  A＋
                </button>
                <button type="button" disabled={messages.length === 0} on:click={copyTranscript}>
                  Copy
                </button>
                <button type="button" disabled={messages.length === 0} on:click={saveTranscript}>
                  Save
                </button>
                <button
                  type="button"
                  disabled={visibleMessages.length === 0}
                  on:click={clearConversation}
                >
                  Clear
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
                Speak normally. The first words can take a few seconds while Apple Speech warms up.
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
              <button
                type="button"
                disabled={isAnswerLoading || !aiQuestion.trim()}
                on:click={askMeetingQuestion}
              >
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
      </div>
    {:else if activeTab === 'recordings'}
      <RecordingsView />
    {:else}
      <SettingsView
        {isRecording}
        onInstalledChanged={(installed) => applyInstalledLanguages(installed)}
        {speechModel}
        onSpeechModelChanged={setSpeechModel}
      />
    {/if}
  </section>
</main>

<style>
  :global(:root) {
    --apple-blue: #0066cc;
    --apple-blue-focus: #0071e3;
    --apple-blue-soft: rgba(0, 102, 204, 0.11);
    --apple-red: #ff3b30;
    --apple-green: #34c759;
    --ink: #1d1d1f;
    --ink-muted: #7a7a7a;
    --ink-secondary: #333333;
    --hairline: #e0e0e0;
    --divider-soft: #f0f0f0;
    --canvas: #ffffff;
    --canvas-parchment: #f5f5f7;
    --surface-pearl: #fafafc;
  }

  :global(body) {
    margin: 0;
    min-height: 100vh;
    overflow: hidden;
    background: var(--canvas-parchment);
    color: var(--ink);
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  }

  :global(*) {
    box-sizing: border-box;
  }

  button,
  select {
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  button:focus-visible,
  select:focus-visible,
  textarea:focus-visible {
    outline: 3px solid rgba(0, 102, 204, 0.24);
    outline-offset: 2px;
  }

  .stage {
    min-height: 100vh;
    background: var(--canvas-parchment);
  }

  .window {
    display: grid;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    grid-template-rows: auto 1fr;
    border: 0;
    border-radius: 0;
    background: var(--canvas);
  }

  .toolbar {
    display: grid;
    align-items: center;
    grid-template-columns: auto 1fr;
    gap: 12px;
    min-height: 54px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--hairline);
    background: rgba(245, 245, 247, 0.92);
    backdrop-filter: blur(18px);
  }

  .toolbar-lead {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-self: end;
  }

  .language-strip,
  .capture-switch,
  .tab-switch,
  .record,
  .record-toggle,
  .date-pill {
    border: 1px solid var(--hairline);
    background: var(--canvas);
  }

  .capture-switch,
  .tab-switch {
    position: relative;
    display: grid;
    isolation: isolate;
    overflow: hidden;
    border-radius: 11px;
    padding: 3px;
    background: #e9e9ed;
  }

  .capture-switch {
    width: 238px;
    grid-template-columns: repeat(3, 1fr);
  }

  .tab-switch {
    width: 276px;
    grid-template-columns: repeat(3, 1fr);
  }

  .capture-switch::before {
    position: absolute;
    z-index: 0;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / 3);
    border: 1px solid rgba(0, 0, 0, 0.05);
    border-radius: 8px;
    background: var(--canvas);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
    content: '';
    transform: translateX(var(--capture-pill-x, 0%));
    transition:
      transform 260ms cubic-bezier(0.22, 1, 0.36, 1),
      background 260ms ease;
  }

  .capture-switch[data-mode='both'] {
    --capture-pill-x: 100%;
  }

  .capture-switch[data-mode='speaker'] {
    --capture-pill-x: 200%;
  }

  .capture-switch button,
  .tab-switch button {
    position: relative;
    z-index: 1;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-muted);
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0;
    transition:
      color 180ms ease,
      opacity 180ms ease;
  }

  .capture-switch button:hover:not(:disabled),
  .tab-switch button:hover {
    color: var(--ink);
  }

  .capture-switch button.active,
  .tab-switch button.active {
    color: var(--ink);
  }

  .tab-switch button.active {
    background: var(--canvas);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  }

  .capture-switch button:disabled {
    cursor: wait;
    opacity: 0.58;
  }

  .language-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 11px;
    padding: 5px 9px;
  }

  .language-strip label {
    display: grid;
    gap: 1px;
  }

  .language-strip label span {
    padding-left: 1px;
    color: var(--ink-muted);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .language-strip select {
    width: 112px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ink);
    font-size: 12px;
    font-weight: 600;
  }

  .language-strip select:disabled {
    cursor: default;
    opacity: 0.56;
  }

  .refresh-languages {
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-muted);
    padding: 4px 6px;
    font-size: 14px;
  }

  .refresh-languages:hover:not(:disabled) {
    color: var(--apple-blue);
  }

  .refresh-languages:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .arrow {
    color: var(--ink-muted);
    font-size: 13px;
  }

  .record-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    border-radius: 999px;
    color: var(--ink-muted);
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    user-select: none;
  }

  .record-toggle input {
    accent-color: var(--apple-blue);
    margin: 0;
  }

  .record-toggle input:disabled + span {
    opacity: 0.6;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 7px;
    border-radius: 999px;
    border-color: rgba(255, 59, 48, 0.35);
    color: var(--apple-red);
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 600;
  }

  .record span {
    width: 10px;
    height: 10px;
    border: 2px solid var(--apple-red);
    border-radius: 999px;
  }

  .record.recording {
    border-color: var(--apple-red);
    background: var(--apple-red);
    color: white;
  }

  .record.recording span {
    border-color: white;
    border-radius: 3px;
    background: white;
  }

  .record:disabled {
    cursor: wait;
    opacity: 0.74;
  }

  .conversation {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 0;
    min-height: 0;
    padding: 0;
  }

  .thread {
    display: grid;
    min-height: 0;
    grid-template-rows: auto 1fr;
    overflow: hidden;
    border-right: 1px solid var(--hairline);
    background: var(--canvas);
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
    color: var(--ink);
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
    border-top: 1px solid var(--hairline);
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
    color: var(--ink);
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
    background: var(--canvas);
    color: var(--ink);
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
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
    color: var(--ink);
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
    border-bottom: 1px solid var(--divider-soft);
    padding: 20px 24px 14px;
  }

  .thread-head h1 {
    margin: 8px 0 0;
    color: var(--ink);
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
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--canvas);
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
    color: var(--ink);
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
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--canvas);
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
    color: var(--ink);
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

  @media (max-width: 1140px) {
    .toolbar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .toolbar-actions {
      justify-self: stretch;
      justify-content: end;
      flex-wrap: wrap;
    }
  }

  @media (max-width: 980px) {
    .conversation {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) minmax(280px, 38vh);
    }

    .thread {
      border-right: 0;
      border-bottom: 1px solid var(--hairline);
    }
  }

  @media (max-width: 900px) {
    .toolbar-lead {
      flex-wrap: wrap;
    }

    .toolbar-actions {
      display: grid;
      grid-template-columns: 1fr auto auto;
      justify-self: stretch;
    }

    .language-strip,
    .capture-switch,
    .record {
      justify-self: stretch;
    }

    .capture-switch {
      width: auto;
      flex: 1 1 auto;
    }
  }

  @media (max-width: 640px) {
    .conversation {
      grid-template-rows: minmax(0, 1fr) minmax(280px, 34vh);
    }

    .meeting-ai {
      overflow: auto;
    }

    .meeting-ai .ai-section {
      margin-top: 12px;
    }

    .ask-section textarea {
      min-height: 72px;
    }

    .toolbar-actions {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .language-strip {
      min-width: 0;
      overflow: hidden;
    }

    .language-strip label {
      min-width: 0;
      flex: 1 1 0;
    }

    .language-strip select {
      width: 100%;
      min-width: 0;
    }

    .record {
      width: auto;
      justify-self: end;
      white-space: nowrap;
    }

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
