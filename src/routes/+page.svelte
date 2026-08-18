<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  type Lane = 'mic' | 'speaker';
  /// A finished utterance, positioned in this session's recording.
  type Utterance = { text: string; startMs: number; endMs: number };
  type TranscriptPayload = {
    lane: Lane;
    committedDelta: string;
    volatile: string;
    utteranceFinal: Utterance | null;
  };
  type StatusPayload = {
    state: 'idle' | 'loading' | 'listening' | 'error';
    message: string | null;
    /// Directory this session's WAV files are being written to; stays put
    /// after a stop so the files can still be found.
    recordingDir: string | null;
  };

  // Cap the transcript so an hours-long session doesn't grow the DOM
  // without bound; the oldest lines scroll away first anyway.
  const MAX_LINES = 500;

  // Mic and speaker get separate columns: the two lanes are transcribed
  // independently and interleaving them would misattribute who said what.
  const LANES: { id: Lane; label: string }[] = [
    { id: 'mic', label: 'マイク' },
    { id: 'speaker', label: 'スピーカー' }
  ];

  /// A settled utterance. `id` only keeps Svelte's keyed each honest as old
  /// lines drop off the front.
  type Line = { id: number; startMs: number; text: string };
  /// `pending` is the text committed for the utterance still being spoken;
  /// it becomes a line once the utterance ends and gets its timestamp.
  type LaneState = { lines: Line[]; pending: string; volatile: string };

  let nextLineId = 0;
  const emptyLane = (): LaneState => ({ lines: [], pending: '', volatile: '' });
  let lanes = $state<Record<Lane, LaneState>>({ mic: emptyLane(), speaker: emptyLane() });
  let status = $state<StatusPayload>({ state: 'idle', message: null, recordingDir: null });
  let busy = $state(false);

  // The backend is the source of truth for running: a capture error flips
  // it back to idle/error even though the Record invoke itself succeeded.
  const running = $derived(status.state === 'loading' || status.state === 'listening');
  const statusText = $derived(
    status.message ? `${status.state}: ${status.message}` : status.state
  );
  // The folder name alone: it is the session's timestamp, and the full path
  // is a tooltip away.
  const recordingName = $derived(status.recordingDir?.split(/[\\/]/).pop() ?? null);

  onMount(() => {
    const unlistenTranscript = listen<TranscriptPayload>('transcript', (event) => {
      const lane = lanes[event.payload.lane];
      lane.pending += event.payload.committedDelta;
      const finished = event.payload.utteranceFinal;
      if (finished) {
        // The final text is the authoritative version of the same words —
        // and the only one that comes with a position in the recording.
        lane.lines.push({ id: nextLineId++, startMs: finished.startMs, text: finished.text });
        lane.lines.splice(0, lane.lines.length - MAX_LINES);
        lane.pending = '';
      }
      lane.volatile = event.payload.volatile;
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

  // Follow the live text like a teleprompter — but only while the reader
  // is at the tail. Scrolling up to reread history pauses the follow;
  // returning near the bottom resumes it. Tracked per lane so reading back
  // one column doesn't freeze the other.
  const follow: Record<Lane, boolean> = { mic: true, speaker: true };
  const FOLLOW_SLACK_PX = 48;

  function onTranscriptScroll(lane: Lane) {
    return (event: Event) => {
      const el = event.currentTarget as HTMLElement;
      follow[lane] = el.scrollHeight - el.scrollTop - el.clientHeight < FOLLOW_SLACK_PX;
    };
  }

  // Re-runs whenever this lane's text changes because the attachment reads
  // both of its states.
  function followTail(lane: Lane) {
    return (el: HTMLElement) => {
      void lanes[lane].lines.length;
      void lanes[lane].pending;
      void lanes[lane].volatile;
      if (follow[lane]) {
        el.scrollTo({ top: el.scrollHeight });
      }
    };
  }

  async function toggle() {
    busy = true;
    const wasRunning = running;
    try {
      await invoke(wasRunning ? 'stop_capture' : 'start_capture');
      if (!wasRunning && !running) {
        // The backend's status event may lag the accepted Start; reflect it
        // now so a quick second click means Stop, not another Start.
        status = { ...status, state: 'loading', message: null };
      }
    } catch (error) {
      status = { ...status, state: 'error', message: String(error) };
    } finally {
      busy = false;
    }
  }

  function clear() {
    for (const { id } of LANES) {
      lanes[id] = emptyLane();
    }
  }

  /// Position in the recording, as the audio player would show it.
  function clock(ms: number) {
    const total = Math.floor(ms / 1000);
    const mm = String(Math.floor(total / 60) % 60).padStart(2, '0');
    const ss = String(total % 60).padStart(2, '0');
    const hours = Math.floor(total / 3600);
    return hours ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
  }
</script>

<main>
  <header>
    <h1>LivePolyTrans v2</h1>
    <div class="controls">
      {#if recordingName}
        <span class="rec-dir" title={status.recordingDir}>録音 {recordingName}</span>
      {/if}
      <span class="status" role="status">{statusText}</span>
      <button onclick={clear} disabled={busy}>Clear</button>
      <button class="record" class:running onclick={toggle} disabled={busy}>
        {running ? 'Stop' : 'Record'}
      </button>
    </div>
  </header>
  <div class="lanes">
    {#each LANES as lane (lane.id)}
      <section class="lane">
        <h2>{lane.label}</h2>
        <div
          class="transcript"
          onscroll={onTranscriptScroll(lane.id)}
          {@attach followTail(lane.id)}
        >
          {#each lanes[lane.id].lines as line (line.id)}
            <p><span class="at">{clock(line.startMs)}</span>{line.text}</p>
          {/each}
          {#if lanes[lane.id].pending || lanes[lane.id].volatile}
            <p>
              <span class="at pending">··:··</span><span class="committed"
                >{lanes[lane.id].pending}</span
              ><span class="volatile">{lanes[lane.id].volatile}</span>
            </p>
          {/if}
        </div>
      </section>
    {/each}
  </div>
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
  .rec-dir {
    font-size: 0.8rem;
    color: #8b98a5;
    font-variant-numeric: tabular-nums;
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
  .lanes {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    min-height: 0; /* let the columns scroll instead of growing the page */
  }
  .lane {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .lane + .lane {
    border-left: 1px solid #2a3138;
  }
  .lane h2 {
    margin: 0;
    padding: 0.5rem 1.25rem;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #8b98a5;
    border-bottom: 1px solid #2a3138;
  }
  .transcript {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.25rem;
    font-size: 1.05rem;
    line-height: 1.9;
  }
  .transcript p {
    margin: 0 0 0.35rem;
    white-space: pre-wrap;
    /* hang the timestamp in its own gutter so the text lines up */
    padding-left: 3.6rem;
    text-indent: -3.6rem;
  }
  .at {
    display: inline-block;
    width: 3.6rem;
    text-indent: 0;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    color: #5d6a76;
    user-select: none;
  }
  /* the line still being spoken has no settled position yet */
  .at.pending {
    color: #3a434c;
  }
  .volatile {
    color: #8b98a5;
  }
</style>
