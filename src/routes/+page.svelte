<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  type TranscriptPayload = {
    committedDelta: string;
    volatile: string;
    utteranceFinal: string | null;
  };
  type StatusPayload = {
    state: 'idle' | 'loading' | 'listening' | 'error';
    message: string | null;
  };

  // Cap the transcript so an hours-long session doesn't grow the DOM
  // without bound; the oldest text scrolls away first anyway.
  const MAX_TRANSCRIPT_CHARS = 50_000;

  let committed = $state('');
  let volatileTail = $state('');
  let status = $state<StatusPayload>({ state: 'idle', message: null });
  let busy = $state(false);

  // The backend is the source of truth for running: a capture error flips
  // it back to idle/error even though the Record invoke itself succeeded.
  const running = $derived(status.state === 'loading' || status.state === 'listening');
  const statusText = $derived(
    status.message ? `${status.state}: ${status.message}` : status.state
  );

  onMount(() => {
    const unlistenTranscript = listen<TranscriptPayload>('transcript', (event) => {
      committed += event.payload.committedDelta;
      if (event.payload.utteranceFinal) {
        committed += '\n';
      }
      if (committed.length > MAX_TRANSCRIPT_CHARS) {
        committed = committed.slice(-MAX_TRANSCRIPT_CHARS);
      }
      volatileTail = event.payload.volatile;
    });
    const unlistenStatus = listen<StatusPayload>('status', (event) => {
      status = event.payload;
    });
    // Status events only fire on change; ask for the current snapshot so a
    // (re)loaded webview doesn't show "idle" while the backend is listening.
    invoke<StatusPayload>('get_status').then((s) => {
      status = s;
    });
    return () => {
      unlistenTranscript.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  });

  // Follow the live text like a teleprompter: re-runs whenever the
  // transcript text changes because the attachment reads both states.
  function followTail(el: HTMLElement) {
    void committed;
    void volatileTail;
    el.scrollTo({ top: el.scrollHeight });
  }

  async function toggle() {
    busy = true;
    const wasRunning = running;
    try {
      await invoke(wasRunning ? 'stop_capture' : 'start_capture');
      if (!wasRunning && !running) {
        // The backend's status event may lag the accepted Start; reflect it
        // now so a quick second click means Stop, not another Start.
        status = { state: 'loading', message: null };
      }
    } catch (error) {
      status = { state: 'error', message: String(error) };
    } finally {
      busy = false;
    }
  }

  function clear() {
    committed = '';
    volatileTail = '';
  }
</script>

<main>
  <header>
    <h1>LivePolyTrans v2</h1>
    <div class="controls">
      <span class="status">{statusText}</span>
      <button onclick={clear} disabled={busy}>Clear</button>
      <button class="record" class:running onclick={toggle} disabled={busy}>
        {running ? 'Stop' : 'Record'}
      </button>
    </div>
  </header>
  <section class="transcript" {@attach followTail}>
    <p>
      <span class="committed">{committed}</span><span class="volatile">{volatileTail}</span>
    </p>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family:
      system-ui,
      -apple-system,
      'Hiragino Sans',
      sans-serif;
    background: #101418;
    color: #e8edf2;
  }
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #2a3138;
  }
  h1 {
    font-size: 1rem;
    margin: 0;
    font-weight: 600;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .status {
    font-size: 0.8rem;
    color: #8b98a5;
  }
  button {
    border: 1px solid #3a434c;
    background: #1a2026;
    color: #e8edf2;
    border-radius: 6px;
    padding: 0.4rem 0.9rem;
    cursor: pointer;
  }
  button.record.running {
    background: #b3261e;
    border-color: #b3261e;
  }
  .transcript {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.25rem;
    font-size: 1.05rem;
    line-height: 1.9;
  }
  .transcript p {
    margin: 0;
    white-space: pre-wrap;
  }
  .volatile {
    color: #8b98a5;
  }
</style>
