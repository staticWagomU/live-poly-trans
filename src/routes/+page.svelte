<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  type TranscriptPayload = { committedDelta: string; volatile: string };

  let committed = $state('');
  let volatileTail = $state('');
  let status = $state('idle');
  let running = $state(false);
  let busy = $state(false);

  onMount(() => {
    const unlistenTranscript = listen<TranscriptPayload>('transcript', (event) => {
      committed += event.payload.committedDelta;
      volatileTail = event.payload.volatile;
    });
    const unlistenStatus = listen<string>('status', (event) => {
      status = event.payload;
    });
    return () => {
      unlistenTranscript.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  });

  async function toggle() {
    busy = true;
    try {
      if (running) {
        await invoke('stop_capture');
        running = false;
      } else {
        await invoke('start_capture');
        running = true;
      }
    } catch (error) {
      status = `error: ${error}`;
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
      <span class="status">{status}</span>
      <button onclick={clear} disabled={busy}>Clear</button>
      <button class="record" class:running onclick={toggle} disabled={busy}>
        {running ? 'Stop' : 'Record'}
      </button>
    </div>
  </header>
  <section class="transcript">
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
