<script lang="ts">
  import '$lib/theme.css';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import {
    chooseDefaultLanguagePair,
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
  import {
    canDecreaseTranscriptFontScale,
    canIncreaseTranscriptFontScale,
    decreaseTranscriptFontScale,
    DEFAULT_TRANSCRIPT_FONT_SCALE,
    increaseTranscriptFontScale
  } from '$lib/transcriptFontSize';
  import {
    getAutoStart,
    getIncludeAudio,
    getLiveSpeakerOverrides,
    getSpeechModel,
    getTranscriptFontScale,
    SETTINGS_KEYS,
    setAutoStart as storeAutoStart,
    setIncludeAudio as storeIncludeAudio,
    setSpeechModel as storeSpeechModel,
    setTranscriptFontScale as storeTranscriptFontScale,
    subscribeSettings
  } from '$lib/settingsStore';
  import { resolveSpeakerName } from '$lib/speakers';
  import { applyTranscriptMessage } from '$lib/transcriptInterim';
  import { chatMessagesToTranscriptEntries, type TranscriptEntry } from '$lib/export/types';
  import { toPlainText } from '$lib/export/plainText';
  import {
    buildTextExport,
    saveTextExportToFile,
    timestampLabel,
    uniqueSpeakerLabels,
    type TextExportFormat
  } from '$lib/export/saveTextExport';
  import {
    applyTranslationEvent,
    interleaveThreadItems,
    recordingStartMarker,
    recordingStopMarker,
    transcriptEventToMessage,
    type ChatMessage,
    type HelperEvent,
    type RecordingMarker,
    type StatusEvent,
    type TranscriptEvent
  } from '$lib/transcripts';
  import {
    boundMessagesByChars,
    planSummaryRequest,
    recentChatHistory,
    type ChatTurn
  } from '$lib/aiContext';
  import AppToolbar from '$lib/AppToolbar.svelte';
  import LiveView from '$lib/LiveView.svelte';
  import MimiView from '$lib/MimiView.svelte';
  import RecordingsView from '$lib/RecordingsView.svelte';
  import SettingsView from '$lib/SettingsView.svelte';
  import {
    missingPermissions,
    permissionStartupNotice,
    requiredPermissions,
    type PermissionStatus
  } from '$lib/permissions';
  import { streamEnginePayload, type SpeechModelSelection } from '$lib/speechModels';
  import {
    emptyStreamSessions,
    isCurrentSessionEvent,
    recordingElapsedSeconds,
    shouldRestartStream,
    withStreamSession,
    withoutStreamSessions,
    type RecordingSession,
    type StreamSessions
  } from '$lib/captureState';
  import { createAsyncCleanupRegistry } from '$lib/asyncCleanup';
  import { completeCaptureStop } from '$lib/captureLifecycle';
  import {
    maxRestartAttempts,
    remainingRestartAttempts,
    restartBackoffMs
  } from '$lib/streamRestart';

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

  const summaryRefreshDelayMs = 6000;

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
  let streamSessionIds: StreamSessions = emptyStreamSessions();
  let restartAttempts: Record<AudioStream, number> = { mic: 0, speaker: 0 };
  let streamStartedAt: Record<AudioStream, number | null> = { mic: null, speaker: null };
  // Bumped on every user-initiated start/stop; a pending auto-restart from
  // before the bump must not resurrect a session the user already stopped.
  let captureGeneration = 0;
  let recordingSession: RecordingSession | null = null;
  let recordingElapsed = 0;
  let recordingTimer: ReturnType<typeof setInterval> | null = null;
  let isRecordingBusy = false;
  let markers: RecordingMarker[] = [];
  let captureTransition: 'starting' | 'stopping' | 'switching' | null = null;
  let liveView: LiveView | undefined;
  let statusMessage: string | null = null;
  let actionNotice: string | null = null;
  let actionNoticeTimer: ReturnType<typeof setTimeout> | null = null;
  let aiSummary = '';
  let summaryCoveredCount = 0;
  let summaryError: string | null = null;
  let aiQuestion = '';
  let chatTurns: ChatTurn[] = [];
  let appError: string | null = null;
  let isSummaryLoading = false;
  let isAnswerLoading = false;
  let summaryRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  let transcriptFontScale = DEFAULT_TRANSCRIPT_FONT_SCALE;
  let speechModel: SpeechModelSelection = { engine: 'builtin' };
  let confirmingClear = false;
  let confirmClearTimer: ReturnType<typeof setTimeout> | null = null;
  let aiOpen = false;
  let aiUnavailable = false;
  let autoStartEnabled = true;
  let includeAudioEnabled = true;
  let mimiActive = false;
  let mimiPreviousMode: CaptureMode = 'both';
  let mimiPreviousRunning = false;
  let mimiStartCount = 0;
  let pttHeld = false;
  let pttReconciling = false;
  let permissionNotice: string | null = null;
  let settingsPane: 'general' | 'privacy' = 'general';
  let liveSpeakerOverrides: Record<string, { name: string }> | null = null;

  $: isTranscribing = activeStreams.size > 0;
  $: isMicCapturing = activeStreams.has('mic');
  $: isSpeakerCapturing = activeStreams.has('speaker');
  $: isCaptureBusy = captureTransition !== null;
  $: isStarting = captureTransition === 'starting';
  $: selectedCaptureMode = isTranscribing ? captureModeFromStreams(activeStreams) : captureMode;
  $: threadItems = [...interleaveThreadItems(messages, markers), ...interimMessages];
  $: captureModeLabel =
    selectedCaptureMode === 'both'
      ? 'Speaker + Mic'
      : selectedCaptureMode === 'mic'
        ? 'Mic'
        : 'Speaker';
  // Only utterances spoken after entering the mode; translations and
  // speaker labels are intentionally not shown there.
  $: mimiLines = mimiActive
    ? [...messages.slice(mimiStartCount), ...interimMessages]
        .filter((message) => message.role === 'self')
        .map((message) => message.text)
    : [];

  onMount(() => {
    transcriptFontScale = getTranscriptFontScale();
    speechModel = getSpeechModel();
    autoStartEnabled = getAutoStart();
    includeAudioEnabled = getIncludeAudio();
    liveSpeakerOverrides = getLiveSpeakerOverrides();
    // A name edited in Settings shows up in the live captions right away;
    // the incoming events and stored transcripts keep their original labels.
    const unsubscribeSpeakerNames = [
      SETTINGS_KEYS.selfSpeakerName,
      SETTINGS_KEYS.otherSpeakerName
    ].map((key) =>
      subscribeSettings(key, () => {
        liveSpeakerOverrides = getLiveSpeakerOverrides();
      })
    );
    const autoStart = autoStartEnabled;

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
          appError = event.payload;
        })
      ),
      cleanupRegistry.add(
        listen<HelperExitedPayload>('helper-exited', (event) => {
          void handleHelperExit(event.payload);
        })
      )
    ])
      .then(async () => {
        if (cleanupRegistry.isDisposed()) {
          return;
        }

        await detectLanguages();

        // Launching used to fire every privacy dialog at once, because asking
        // was the only way to learn the answer. Reading the grants first keeps
        // the first launch quiet: if something is missing we explain it and
        // let the user grant it one at a time from Settings.
        const missing = await missingCapturePermissions();

        // The app transcribes from launch (the new base behavior); the
        // preference only decides whether we start paused instead.
        if (autoStart && missing.length === 0 && !cleanupRegistry.isDisposed()) {
          await startTranscription();
        }
      })
      .catch((error) => {
        if (!cleanupRegistry.isDisposed()) {
          appError = `Could not initialize the app: ${String(error)}`;
          cleanupRegistry.dispose();
        }
      });

    return () => {
      cleanupRegistry.dispose();
      unsubscribeSpeakerNames.forEach((unsubscribe) => unsubscribe());
      if (summaryRefreshTimer) {
        clearTimeout(summaryRefreshTimer);
      }
      if (actionNoticeTimer) {
        clearTimeout(actionNoticeTimer);
      }
      if (confirmClearTimer) {
        clearTimeout(confirmClearTimer);
      }
      if (recordingTimer) {
        clearInterval(recordingTimer);
      }
    };
  });

  /// Reads the grants without prompting and turns whatever is missing into a
  /// banner. A probe failure returns nothing missing on purpose — capture then
  /// runs as before and reports the real error rather than being held back by
  /// an inconclusive check.
  async function missingCapturePermissions() {
    let status: PermissionStatus | null = null;
    try {
      status = await invoke<PermissionStatus>('permission_status');
    } catch (error) {
      console.error('Could not read the privacy permissions', error);
    }

    const missing = missingPermissions(status, requiredPermissions(captureMode));
    permissionNotice = permissionStartupNotice(missing);
    return missing;
  }

  function openPrivacySettings() {
    settingsPane = 'privacy';
    activeTab = 'settings';
  }

  /// Re-evaluated whenever Settings reports a change, so granting the last
  /// missing permission clears the banner without a relaunch.
  function applyPermissionStatus(status: PermissionStatus) {
    permissionNotice = permissionStartupNotice(
      missingPermissions(status, requiredPermissions(captureMode))
    );
  }

  async function detectLanguages(preserveSelection = false) {
    try {
      const payload = await invoke<LanguageDetectionPayload>('detect_languages');
      applyInstalledLanguages(payload.installed, preserveSelection);
    } catch (error) {
      appError = String(error);
    }
  }

  function applyInstalledLanguages(installed: LanguageInfo[], preserveSelection = true) {
    installedLanguages = installed.length > 0 ? installed : installedLanguages;

    const stillInstalled = (id: string) =>
      installedLanguages.some((language) => language.id === id);
    // An empty sub language means "翻訳しない" and is always valid.
    if (
      preserveSelection &&
      stillInstalled(mainLanguage) &&
      (subLanguage === '' || stillInstalled(subLanguage))
    ) {
      return;
    }

    const pair = chooseDefaultLanguagePair(installedLanguages, navigator.language);
    mainLanguage = pair.source;
    subLanguage = pair.target;
  }

  function handleLanguageChange(which: 'source' | 'target', value: string) {
    const pair = updateLanguagePair(
      { source: mainLanguage, target: subLanguage },
      which,
      value
    );
    mainLanguage = pair.source;
    subLanguage = pair.target;
    void restartTranscriptionIfRunning();
  }

  /// Language/engine changes need fresh helper processes. With transcription
  /// always on, that restart happens in place instead of asking the user to
  /// stop and start manually. An open recording session survives: restarted
  /// streams reattach via the recordingDir spawn argument.
  async function restartTranscriptionIfRunning() {
    if (!isTranscribing || isCaptureBusy) {
      return;
    }

    captureTransition = 'switching';
    try {
      captureGeneration += 1;
      clearStreamSessions([...activeStreams]);
      activeStreams = new Set();
      interimMessages = [];
      await invoke('stop_all_sessions');
      for (const stream of streamsForCaptureMode(captureMode)) {
        await startStream(stream);
      }
    } catch (error) {
      appError = String(error);
    } finally {
      captureTransition = null;
    }
  }

  async function handleHelperEvent(payload: HelperEvent) {
    if (!isCurrentSessionEvent(streamSessionIds, payload)) {
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
    const shouldScrollToLatest = liveView?.shouldStickToLatest() ?? true;
    const message = transcriptEventToMessage(event);
    if (recordingSession) {
      message.inRecording = true;
    }
    const nextState = applyTranscriptMessage({ messages, interimMessages }, message);
    messages = nextState.messages;
    interimMessages = nextState.interimMessages;

    await tick();

    if (shouldScrollToLatest) {
      liveView?.scrollToLatest('auto');
    } else {
      liveView?.syncJumpToLatestButton();
    }

    if (event.isFinal) {
      scheduleSummaryRefresh();
    }
  }

  function createStreamSessionId(stream: AudioStream) {
    return `${stream}-${Date.now()}-${crypto.randomUUID()}`;
  }

  function setStreamSession(stream: AudioStream, sessionId: string | null) {
    streamSessionIds = withStreamSession(streamSessionIds, stream, sessionId);
  }

  function clearStreamSessions(streams: AudioStream[]) {
    streamSessionIds = withoutStreamSessions(streamSessionIds, streams);
  }

  function setTranscriptFontScale(scale: number) {
    transcriptFontScale = scale;
    storeTranscriptFontScale(scale);
  }

  function setAutoStartEnabled(enabled: boolean) {
    autoStartEnabled = enabled;
    storeAutoStart(enabled);
  }

  function setIncludeAudioEnabled(enabled: boolean) {
    includeAudioEnabled = enabled;
    storeIncludeAudio(enabled);
  }

  function setSpeechModel(selection: SpeechModelSelection) {
    speechModel = selection;
    storeSpeechModel(selection);
    void restartTranscriptionIfRunning();
  }

  /// Rewords raw helper errors for the two launch-time failures a person
  /// can actually fix themselves (privacy permissions).
  function captureStartGuidance(error: string): string {
    if (/microphone/i.test(error)) {
      return `Microphone access is not allowed. Enable LivePolyTrans under System Settings > Privacy & Security > Microphone, then press Resume.\n${error}`;
    }
    if (/screen\s*(capture|recording)/i.test(error)) {
      return `Screen Recording access (needed for system audio) is not allowed. Enable LivePolyTrans under System Settings > Privacy & Security > Screen & System Audio Recording, then press Resume.\n${error}`;
    }
    return error;
  }

  async function startTranscription() {
    if (isCaptureBusy || isTranscribing) {
      return;
    }

    appError = null;
    statusMessage = null;
    captureTransition = 'starting';
    try {
      captureGeneration += 1;
      restartAttempts = { mic: 0, speaker: 0 };
      const failures: string[] = [];

      for (const stream of streamsForCaptureMode(captureMode)) {
        try {
          await startStream(stream);
        } catch (error) {
          failures.push(`${stream}: ${String(error)}`);
        }
      }

      if (failures.length > 0) {
        appError = captureStartGuidance(failures.join('\n'));
      }
    } finally {
      captureTransition = null;
    }
  }

  /// Stops capture without any user interaction. Interim (unconfirmed)
  /// captions are discarded on purpose — in push-to-talk this prevents a
  /// half-heard phrase from lingering as if it were said.
  async function pauseCapture() {
    if (isCaptureBusy || !isTranscribing) {
      return;
    }

    appError = null;
    statusMessage = null;
    captureTransition = 'stopping';
    captureGeneration += 1;
    clearStreamSessions([...activeStreams]);
    activeStreams = new Set();
    interimMessages = [];
    const stopErrors = await completeCaptureStop(() => invoke('stop_all_sessions'));
    if (stopErrors.length > 0) {
      appError = `Could not fully pause capture:\n${stopErrors.map(String).join('\n')}`;
    }
    captureTransition = null;
  }

  /// Stop reinterpreted as pause: capture goes quiet but the conversation,
  /// summary, and (after confirmation) any recording session survive.
  async function pauseTranscription() {
    if (isCaptureBusy || !isTranscribing) {
      return;
    }

    if (recordingSession) {
      const alsoStopRecording = window.confirm(
        '一時停止すると録音も終了します。録音は Recordings に保存されます。続けますか?'
      );
      if (!alsoStopRecording) {
        return;
      }
      await stopRecordingSession();
    }

    await pauseCapture();
  }

  async function toggleTranscription() {
    if (isTranscribing) {
      await pauseTranscription();
    } else {
      await startTranscription();
    }
  }

  /// Face-to-face (mimi) mode: mic only, push-to-talk. Speaker capture is
  /// stopped on entry so the other person reading captions aloud cannot be
  /// re-transcribed into an endless loop; the previous source configuration
  /// comes back on exit.
  async function enterMimi() {
    if (mimiActive) {
      return;
    }

    mimiPreviousMode = captureMode;
    mimiPreviousRunning = isTranscribing;
    mimiStartCount = messages.length;
    pttHeld = false;
    mimiActive = true;
    activeTab = 'live';
    captureMode = 'mic';

    if (recordingSession) {
      await stopRecordingSession();
    }
    await pauseCapture();
  }

  async function exitMimi() {
    if (!mimiActive) {
      return;
    }

    mimiActive = false;
    pttHeld = false;
    captureMode = mimiPreviousMode;
    await pauseCapture();
    if (mimiPreviousRunning) {
      await startTranscription();
    }
  }

  function setPtt(held: boolean) {
    if (!mimiActive) {
      return;
    }

    pttHeld = held;
    void reconcilePtt();
  }

  /// Press/release races capture start/stop (helper spawn takes a moment).
  /// Instead of acting on each event, converge capture state onto the
  /// current held state until they match.
  async function reconcilePtt() {
    if (pttReconciling) {
      return;
    }

    pttReconciling = true;
    try {
      while (mimiActive && pttHeld !== isTranscribing) {
        if (isCaptureBusy) {
          await new Promise((resolve) => setTimeout(resolve, 100));
          continue;
        }

        if (pttHeld) {
          await startTranscription();
        } else {
          await pauseCapture();
        }
      }
    } finally {
      pttReconciling = false;
    }
  }

  async function toggleRecordingSession() {
    if (isRecordingBusy) {
      return;
    }

    isRecordingBusy = true;
    try {
      if (recordingSession) {
        await stopRecordingSession();
      } else {
        await startRecordingSession();
      }
    } finally {
      isRecordingBusy = false;
    }
  }

  async function startRecordingSession() {
    if (!isTranscribing) {
      appError = 'Recording needs live transcription. Press Resume first.';
      return;
    }

    try {
      const created = await invoke<CreatedRecording>('start_recording_session', {
        includeAudio: includeAudioEnabled
      });
      recordingSession = { id: created.id, dir: created.dir, startedAtMs: Date.now() };
      recordingElapsed = 0;
      recordingTimer = setInterval(() => {
        if (recordingSession) {
          recordingElapsed = recordingElapsedSeconds(recordingSession, Date.now());
        }
      }, 1000);
      markers = [...markers, recordingStartMarker(created.id, new Date().toISOString(), messages.length)];
      showActionNotice('Recording started — transcription keeps running.');
    } catch (error) {
      appError = String(error);
    }
  }

  async function stopRecordingSession() {
    const session = recordingSession;
    if (!session) {
      return;
    }

    recordingSession = null;
    if (recordingTimer) {
      clearInterval(recordingTimer);
      recordingTimer = null;
    }
    const durationSeconds = recordingElapsedSeconds(session, Date.now());

    try {
      await invoke('stop_recording_session', { id: session.id });
      markers = [
        ...markers,
        recordingStopMarker(session.id, new Date().toISOString(), messages.length, durationSeconds)
      ];
      showActionNotice('Saved to Recordings.');
    } catch (error) {
      appError = String(error);
    }
  }

  async function selectCaptureMode(mode: CaptureMode) {
    if (isCaptureBusy) {
      return;
    }

    appError = null;
    captureMode = mode;

    if (!isTranscribing) {
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
      appError = String(error);
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
        // A stream (re)started while a recording session is open attaches
        // its recorder at spawn; mid-stream start/stop rides the control
        // channel instead.
        recordingDir: recordingSession?.dir ?? null,
        recordingAudio: includeAudioEnabled,
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
    streamStartedAt = { ...streamStartedAt, [stream]: Date.now() };
  }

  async function stopStream(stream: AudioStream) {
    setStreamSession(stream, null);
    activeStreams.delete(stream);
    activeStreams = new Set(activeStreams);
    await invoke('stop_stream_session', { stream });
  }

  async function handleHelperExit(payload: HelperExitedPayload) {
    if (!isCurrentSessionEvent(streamSessionIds, payload)) {
      return;
    }

    setStreamSession(payload.stream, null);
    activeStreams.delete(payload.stream);
    activeStreams = new Set(activeStreams);

    const startedAt = streamStartedAt[payload.stream];
    const uptimeMs = startedAt !== null ? Date.now() - startedAt : null;
    const generation = captureGeneration;

    for (const attempt of remainingRestartAttempts(restartAttempts[payload.stream], uptimeMs)) {
      restartAttempts = { ...restartAttempts, [payload.stream]: attempt };
      statusMessage = `Restarting ${payload.stream} capture (attempt ${attempt}/${maxRestartAttempts})…`;

      await new Promise((resolve) => setTimeout(resolve, restartBackoffMs(attempt)));

      const stillWanted = shouldRestartStream({
        generationAtExit: generation,
        currentGeneration: captureGeneration,
        captureMode,
        stream: payload.stream,
        sessions: streamSessionIds
      });
      if (!stillWanted) {
        statusMessage = null;
        return;
      }

      try {
        await startStream(payload.stream);
        statusMessage = null;
        return;
      } catch (error) {
        appError = `Restarting ${payload.stream} capture failed: ${String(error)}`;
      }
    }

    statusMessage = null;
    appError = `${payload.stream} capture stopped unexpectedly (code ${payload.code ?? '?'}) and automatic restart gave up. Press Resume to start again.`;

    // The crash path never goes through pauseTranscription, so an open
    // recording session would otherwise stay unfinalized and be orphaned.
    if (activeStreams.size === 0) {
      interimMessages = [];
      try {
        await stopRecordingSession();
      } catch (error) {
        appError = `${appError}\nCould not finalize the recording: ${String(error)}`;
      }
    }
  }

  function selectedTranscriptionLanguages() {
    return transcriptionCandidateLanguages(mainLanguage, subLanguage);
  }

  function scheduleSummaryRefresh() {
    // A Mac without Apple Intelligence fails every request the same way;
    // retrying on each utterance would just spam the error.
    if (aiUnavailable) {
      return;
    }

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
      appError = null;
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
      if (/Apple Intelligence is unavailable/i.test(summaryError)) {
        aiUnavailable = true;
      }
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
    appError = null;
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
      appError = String(error);
      if (/Apple Intelligence is unavailable/i.test(appError)) {
        aiUnavailable = true;
      }
      chatTurns = chatTurns.slice(0, -1);
      aiQuestion = question;
    } finally {
      isAnswerLoading = false;
    }
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (!event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }

    if (event.shiftKey && event.key.toLowerCase() === 'c') {
      event.preventDefault();
      void copyTranscript();
      return;
    }

    if (event.shiftKey) {
      return;
    }

    switch (event.key) {
      case 's':
        event.preventDefault();
        void saveTranscript();
        break;
      case '+':
      case '=':
        event.preventDefault();
        if (canIncreaseTranscriptFontScale(transcriptFontScale)) {
          setTranscriptFontScale(increaseTranscriptFontScale(transcriptFontScale));
        }
        break;
      case '-':
        event.preventDefault();
        if (canDecreaseTranscriptFontScale(transcriptFontScale)) {
          setTranscriptFontScale(decreaseTranscriptFontScale(transcriptFontScale));
        }
        break;
      case '0':
        event.preventDefault();
        setTranscriptFontScale(DEFAULT_TRANSCRIPT_FONT_SCALE);
        break;
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

  // Custom live speaker names live in settings, not the messages, so
  // frontend-generated text (clipboard / save-as) resolves labels here.
  // The ⌘S JSON path stays raw on purpose: it is the faithful record.
  function resolveLiveEntries(entries: TranscriptEntry[]): TranscriptEntry[] {
    if (!liveSpeakerOverrides) {
      return entries;
    }
    return entries.map((entry) => ({
      ...entry,
      speakerLabel: resolveSpeakerName(entry, liveSpeakerOverrides)
    }));
  }

  async function copyTranscript() {
    const text = toPlainText(resolveLiveEntries(chatMessagesToTranscriptEntries(messages)));

    try {
      await navigator.clipboard.writeText(text);
      showActionNotice('Transcript copied.');
    } catch (error) {
      appError = String(error);
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
      const fileName = result.json_path.split('/').pop() ?? result.json_path;
      showActionNotice(`Saved to Downloads: ${fileName}`);
    } catch (error) {
      appError = String(error);
    }
  }

  // Format-picking save (Phase 1-4): unlike ⌘S it generates the text on the
  // frontend and lets the user choose the destination in a save dialog.
  async function saveTranscriptAs(format: TextExportFormat) {
    if (messages.length === 0) {
      return;
    }

    const entries = resolveLiveEntries(chatMessagesToTranscriptEntries(messages));
    const now = new Date();
    try {
      const file = buildTextExport(format, entries, {
        baseName: `LivePolyTrans-transcript-${timestampLabel(now)}`,
        markdownMeta: {
          dateLabel: now.toLocaleString('ja-JP', { dateStyle: 'medium', timeStyle: 'short' }),
          participants: uniqueSpeakerLabels(entries)
        }
      });
      const destination = await saveTextExportToFile(file);
      if (destination !== null) {
        const fileName = destination.split('/').pop() ?? destination;
        showActionNotice(`Saved: ${fileName}`);
      }
    } catch (error) {
      appError = String(error);
    }
  }

  // Clear wipes the whole meeting (transcript, summary, chat) with no undo
  // and sits right next to Save, so it asks for a second click and disarms
  // by itself.
  function requestClearConversation() {
    if (confirmingClear) {
      clearConversation();
      return;
    }

    confirmingClear = true;
    if (confirmClearTimer) {
      clearTimeout(confirmClearTimer);
    }
    confirmClearTimer = setTimeout(() => {
      confirmingClear = false;
    }, 4000);
  }

  function clearConversation() {
    if (confirmClearTimer) {
      clearTimeout(confirmClearTimer);
    }
    confirmingClear = false;
    messages = [];
    interimMessages = [];
    markers = [];
    aiSummary = '';
    summaryCoveredCount = 0;
    summaryError = null;
    chatTurns = [];
    appError = null;
    actionNotice = null;
  }
</script>

<svelte:head>
  <title>LivePolyTrans</title>
</svelte:head>

<svelte:window on:keydown={handleGlobalKeydown} />

<main class="stage">
  <section class="window" aria-label="LivePolyTrans" style="position: relative;">
    <AppToolbar
      bind:activeTab
      {selectedCaptureMode}
      {isCaptureBusy}
      {isTranscribing}
      {mainLanguage}
      {subLanguage}
      {installedLanguages}
      {activeStreams}
      isRecordingSession={recordingSession !== null}
      {recordingElapsed}
      {isRecordingBusy}
      {aiOpen}
      {transcriptFontScale}
      onSelectCaptureMode={selectCaptureMode}
      onLanguageChange={handleLanguageChange}
      onRefreshLanguages={() => detectLanguages(true)}
      onToggleAiPanel={() => (aiOpen = !aiOpen)}
      onToggleRecordingSession={toggleRecordingSession}
      onCopy={copyTranscript}
      onSave={saveTranscript}
      onSaveAs={saveTranscriptAs}
      onFontScaleChange={setTranscriptFontScale}
      onEnterMimi={() => void enterMimi()}
    />

    {#if mimiActive}
      <MimiView
        lines={mimiLines}
        {pttHeld}
        onPttChange={setPtt}
        onExit={() => void exitMimi()}
      />
    {/if}

    <div class="content-shell">
      <div class="app-alert-slot">
        {#if permissionNotice && activeTab !== 'settings'}
          <div class="app-notice" role="status">
            <p>{permissionNotice}</p>
            <button type="button" on:click={openPrivacySettings}>設定を開く</button>
          </div>
        {/if}
        {#if appError}
          <div class="app-alert" role="alert">
            <p>{appError}</p>
            <button type="button" aria-label="Dismiss error" on:click={() => (appError = null)}>
              &times;
            </button>
          </div>
        {/if}
      </div>

      {#if activeTab === 'live'}
        <LiveView
          bind:this={liveView}
          {threadItems}
          hasFinalMessages={messages.length > 0}
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
          speakerOverrides={liveSpeakerOverrides}
          onTogglePause={toggleTranscription}
          onCopy={copyTranscript}
          onSave={saveTranscript}
          onClear={requestClearConversation}
          {aiOpen}
          {aiSummary}
          {summaryError}
          {isSummaryLoading}
          {aiUnavailable}
          {chatTurns}
          bind:aiQuestion
          {isAnswerLoading}
          onRefreshSummary={() => generateMeetingSummary(false)}
          onAsk={askMeetingQuestion}
        />
      {:else if activeTab === 'recordings'}
        <RecordingsView />
      {:else}
        <SettingsView
          isRecording={isTranscribing}
          onInstalledChanged={(installed) => applyInstalledLanguages(installed)}
          {speechModel}
          onSpeechModelChanged={setSpeechModel}
          {autoStartEnabled}
          onAutoStartChange={setAutoStartEnabled}
          {includeAudioEnabled}
          onIncludeAudioChange={setIncludeAudioEnabled}
          initialPane={settingsPane}
          onPermissionsChanged={applyPermissionStatus}
        />
      {/if}
    </div>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    min-height: 100vh;
    overflow: hidden;
    background: var(--canvas-parchment);
    color: var(--legacy-ink);
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  }

  :global(*) {
    box-sizing: border-box;
  }

  button {
    font: inherit;
    cursor: pointer;
  }

  button:focus-visible {
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
    background: var(--legacy-canvas);
  }

  .content-shell {
    display: grid;
    min-height: 0;
    overflow: hidden;
    grid-template-rows: auto minmax(0, 1fr);
  }

  .app-alert-slot {
    min-height: 0;
  }

  .app-alert {
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid rgba(179, 38, 30, 0.18);
    background: #fff4f3;
    color: #9f211b;
    padding: 8px 16px;
  }

  .app-alert p {
    max-height: 54px;
    flex: 1;
    overflow: auto;
    margin: 0;
    font-size: 12px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .app-alert button {
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: currentColor;
    font-size: 20px;
    line-height: 1;
  }

  .app-alert button:hover {
    background: rgba(179, 38, 30, 0.08);
  }

  /* Missing permissions are a normal first-launch state, not a failure, so
     this reads as guidance rather than the red error alert above. */
  .app-notice {
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid rgba(0, 102, 204, 0.2);
    background: var(--blue-soft);
    color: var(--blue);
    padding: 8px 16px;
  }

  .app-notice p {
    flex: 1;
    margin: 0;
    font-size: 12px;
    line-height: 1.4;
  }

  .app-notice button {
    flex: 0 0 auto;
    border: 0;
    border-radius: 7px;
    background: var(--blue);
    color: #fff;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
</style>
