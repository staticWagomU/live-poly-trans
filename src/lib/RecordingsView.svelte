<script lang="ts">
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import {
    buildRecordingTranscript,
    formatFileSize,
    formatTimestampMs,
    type RecordingFileInfo,
    type RecordingSummary,
    type RecordingTranscriptItem,
    type RecordingWaveform
  } from '$lib/recordings';

  let recordings = $state<RecordingSummary[]>([]);
  let selectedRecording = $state<RecordingSummary | null>(null);
  let selectedFile = $state<RecordingFileInfo | null>(null);
  let transcript = $state<RecordingTranscriptItem[]>([]);
  let waveform = $state<RecordingWaveform | null>(null);
  let audioSrc = $state<string | null>(null);
  let audioElement = $state<HTMLAudioElement | null>(null);
  let waveformCanvas = $state<HTMLCanvasElement | null>(null);
  let currentTimeMs = $state(0);
  let error = $state<string | null>(null);
  let exportNotice = $state<string | null>(null);
  let isExporting = $state(false);

  onMount(() => {
    void refreshRecordings();
  });

  async function refreshRecordings() {
    error = null;
    try {
      recordings = await invoke<RecordingSummary[]>('list_recordings');
      if (selectedRecording) {
        const updated = recordings.find((entry) => entry.id === selectedRecording?.id) ?? null;
        selectedRecording = updated;
      }
      if (!selectedRecording && recordings.length > 0) {
        await selectRecording(recordings[0]);
      }
    } catch (loadError) {
      error = String(loadError);
    }
  }

  async function selectRecording(recording: RecordingSummary) {
    selectedRecording = recording;
    exportNotice = null;
    error = null;
    transcript = [];
    try {
      const events = await invoke<unknown[]>('read_recording_transcript', { id: recording.id });
      transcript = buildRecordingTranscript(events);
    } catch (transcriptError) {
      error = String(transcriptError);
    }

    await selectFile(recording.files[0] ?? null);
  }

  async function selectFile(file: RecordingFileInfo | null) {
    selectedFile = file;
    waveform = null;
    currentTimeMs = 0;
    audioSrc = file ? convertFileSrc(file.path) : null;

    if (!file || !selectedRecording) {
      drawWaveform();
      return;
    }

    try {
      waveform = await invoke<RecordingWaveform>('recording_waveform', {
        id: selectedRecording.id,
        fileName: file.name
      });
    } catch (waveError) {
      error = String(waveError);
    }

    drawWaveform();
  }

  function drawWaveform() {
    const canvas = waveformCanvas;
    if (!canvas) {
      return;
    }

    const ratio = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    canvas.width = width * ratio;
    canvas.height = height * ratio;

    const context = canvas.getContext('2d');
    if (!context) {
      return;
    }

    context.scale(ratio, ratio);
    context.clearRect(0, 0, width, height);

    if (!waveform || waveform.peaks.length === 0) {
      return;
    }

    const middle = height / 2;
    const barWidth = width / waveform.peaks.length;
    context.fillStyle = 'rgba(0, 102, 204, 0.55)';

    for (let index = 0; index < waveform.peaks.length; index++) {
      const amplitude = Math.min(1, waveform.peaks[index]);
      const barHeight = Math.max(1, amplitude * (height - 8));
      context.fillRect(index * barWidth, middle - barHeight / 2, Math.max(1, barWidth - 0.5), barHeight);
    }
  }

  $effect(() => {
    if (waveformCanvas && waveform) {
      drawWaveform();
    }
  });

  function handleTimeUpdate() {
    if (audioElement) {
      currentTimeMs = audioElement.currentTime * 1000;
    }
  }

  function seekTo(startMs: number) {
    if (!audioElement) {
      return;
    }

    audioElement.currentTime = startMs / 1000;
    void audioElement.play();
  }

  function seekFromWaveform(event: MouseEvent) {
    if (!waveform || !waveformCanvas || waveform.durationMs <= 0) {
      return;
    }

    const rect = waveformCanvas.getBoundingClientRect();
    const ratio = (event.clientX - rect.left) / rect.width;
    seekTo(ratio * waveform.durationMs);
  }

  async function exportVariant(variant: 'mic' | 'speaker' | 'mixed') {
    if (!selectedRecording || isExporting) {
      return;
    }

    isExporting = true;
    exportNotice = null;
    error = null;
    try {
      const destination = await invoke<string>('export_recording', {
        id: selectedRecording.id,
        variant
      });
      exportNotice = `Saved: ${destination}`;
    } catch (exportError) {
      error = String(exportError);
    } finally {
      isExporting = false;
    }
  }

  function activeItemKey(items: RecordingTranscriptItem[], timeMs: number): string | null {
    let active: RecordingTranscriptItem | null = null;
    for (const item of items) {
      if (item.startMs <= timeMs) {
        active = item;
      } else {
        break;
      }
    }

    return active?.key ?? null;
  }

  const playheadRatio = $derived(
    waveform && waveform.durationMs > 0 ? Math.min(1, currentTimeMs / waveform.durationMs) : 0
  );
  const activeKey = $derived(activeItemKey(transcript, currentTimeMs));

  function hasStream(stream: string) {
    return selectedRecording?.files.some((file) => file.stream === stream) ?? false;
  }

  function recordingTitle(recording: RecordingSummary) {
    const date = new Date(recording.startedAt);
    return Number.isNaN(date.getTime())
      ? recording.id
      : date.toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
  }
</script>

<div class="recordings">
  <section class="detail" aria-label="Recording detail">
    {#if selectedRecording}
      <header class="detail-head">
        <div>
          <span class="pill">Recording</span>
          <h2>{recordingTitle(selectedRecording)}</h2>
        </div>
        <div class="downloads" aria-label="Download recording">
          <button
            type="button"
            disabled={isExporting || !hasStream('mic')}
            onclick={() => exportVariant('mic')}
          >
            Mic
          </button>
          <button
            type="button"
            disabled={isExporting || !hasStream('speaker')}
            onclick={() => exportVariant('speaker')}
          >
            Speaker
          </button>
          <button
            type="button"
            disabled={isExporting || !hasStream('mic') || !hasStream('speaker')}
            onclick={() => exportVariant('mixed')}
          >
            Merged
          </button>
        </div>
      </header>

      {#if error}
        <p class="notice error">{error}</p>
      {/if}
      {#if exportNotice}
        <p class="notice">{exportNotice}</p>
      {/if}

      <div class="file-chips" aria-label="Recorded audio files">
        {#each selectedRecording.files as file (file.name)}
          <button
            type="button"
            class:active={selectedFile?.name === file.name}
            onclick={() => selectFile(file)}
          >
            {file.name} · {formatFileSize(file.sizeBytes)}
          </button>
        {/each}
        {#if selectedRecording.files.length === 0}
          <span class="empty-chip">No audio files</span>
        {/if}
      </div>

      <div class="waveform-shell">
        <canvas
          bind:this={waveformCanvas}
          class="waveform"
          onclick={seekFromWaveform}
          aria-label="Audio waveform"
        ></canvas>
        <div class="playhead" style={`left: ${playheadRatio * 100}%`} aria-hidden="true"></div>
      </div>

      {#if audioSrc}
        <audio
          controls
          bind:this={audioElement}
          src={audioSrc}
          ontimeupdate={handleTimeUpdate}
        ></audio>
      {/if}

      <section class="transcript" aria-label="Recording transcript">
        <h3>Transcript</h3>
        {#if transcript.length === 0}
          <p class="empty">No finalized transcript for this recording.</p>
        {:else}
          <ul>
            {#each transcript as item (item.key)}
              <li>
                <button
                  type="button"
                  class:active={item.key === activeKey}
                  onclick={() => seekTo(item.startMs)}
                >
                  <span class="time">{formatTimestampMs(item.startMs)}</span>
                  <span class="body">
                    <span class="speaker">{item.speakerLabel} · {item.language}</span>
                    <span class="text">{item.text}</span>
                    {#if item.translation}
                      <span class="translation">{item.translation}</span>
                    {/if}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {:else}
      <div class="empty-state">
        <h2>No recordings yet</h2>
        <p>Enable “Save audio” next to Record, then start a session to capture files here.</p>
      </div>
    {/if}
  </section>

  <aside class="list" aria-label="Recording files">
    <div class="list-head">
      <h3>Recordings</h3>
      <button type="button" onclick={refreshRecordings}>Reload</button>
    </div>
    <ul>
      {#each recordings as recording (recording.id)}
        <li>
          <button
            type="button"
            class:active={selectedRecording?.id === recording.id}
            onclick={() => selectRecording(recording)}
          >
            <span class="recording-title">{recordingTitle(recording)}</span>
            <span class="recording-meta">
              {recording.files.map((file) => file.stream).filter(
                (stream, index, streams) => streams.indexOf(stream) === index
              ).join(' + ') || 'no audio'}
            </span>
          </button>
        </li>
      {/each}
      {#if recordings.length === 0}
        <li class="empty">Nothing recorded yet.</li>
      {/if}
    </ul>
  </aside>
</div>

<style>
  .recordings {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    min-height: 0;
    height: 100%;
  }

  .detail {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    border-right: 1px solid var(--hairline);
    background: var(--canvas);
    padding: 20px 24px;
  }

  .detail-head {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 16px;
  }

  .detail-head h2 {
    margin: 8px 0 0;
    color: var(--ink);
    font-size: 21px;
    font-weight: 600;
  }

  .pill {
    width: fit-content;
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--surface-pearl);
    color: var(--ink-muted);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
  }

  .downloads {
    display: flex;
    gap: 6px;
  }

  .downloads button,
  .list-head button,
  .file-chips button {
    border: 1px solid rgba(0, 102, 204, 0.16);
    border-radius: 8px;
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 600;
  }

  .downloads button:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .file-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .file-chips button {
    border-color: var(--hairline);
    background: var(--canvas);
    color: var(--ink-muted);
  }

  .file-chips button.active {
    border-color: rgba(0, 102, 204, 0.4);
    background: var(--apple-blue-soft);
    color: var(--apple-blue);
  }

  .empty-chip {
    color: var(--ink-muted);
    font-size: 12px;
  }

  .notice {
    margin: 0;
    color: var(--ink-muted);
    font-size: 12px;
    word-break: break-all;
  }

  .notice.error {
    color: #b3261e;
  }

  .waveform-shell {
    position: relative;
    height: 120px;
    flex: 0 0 auto;
    overflow: hidden;
    border: 1px solid var(--divider-soft);
    border-radius: 12px;
    background: var(--surface-pearl);
  }

  .waveform {
    width: 100%;
    height: 100%;
    cursor: pointer;
    display: block;
  }

  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1.5px;
    background: var(--apple-red);
    pointer-events: none;
  }

  audio {
    width: 100%;
  }

  .transcript {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 8px;
  }

  .transcript h3,
  .list-head h3 {
    margin: 0;
    color: var(--ink-secondary);
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }

  .transcript ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .transcript li button {
    display: flex;
    width: 100%;
    gap: 12px;
    align-items: baseline;
    border: 0;
    border-radius: 10px;
    background: transparent;
    padding: 8px 10px;
    text-align: left;
  }

  .transcript li button:hover {
    background: var(--canvas-parchment);
  }

  .transcript li button.active {
    background: var(--apple-blue-soft);
  }

  .time {
    flex: 0 0 auto;
    color: var(--apple-blue);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .body {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .speaker {
    color: var(--ink-muted);
    font-size: 11px;
    font-weight: 600;
  }

  .text {
    color: var(--ink);
    font-size: 14px;
    line-height: 1.4;
    word-break: break-word;
  }

  .translation {
    color: var(--ink-muted);
    font-size: 12.5px;
    line-height: 1.35;
    word-break: break-word;
  }

  .empty,
  .empty-state {
    color: var(--ink-muted);
    font-size: 13px;
  }

  .empty-state {
    display: grid;
    place-content: center;
    height: 100%;
    text-align: center;
    gap: 4px;
  }

  .empty-state h2 {
    margin: 0;
    color: var(--ink);
    font-size: 17px;
  }

  .empty-state p {
    margin: 0;
    max-width: 360px;
  }

  .list {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 10px;
    overflow: auto;
    background: var(--canvas-parchment);
    padding: 16px;
  }

  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .list ul {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .list li button {
    display: grid;
    width: 100%;
    gap: 2px;
    border: 1px solid var(--hairline);
    border-radius: 10px;
    background: var(--canvas);
    padding: 10px 12px;
    text-align: left;
  }

  .list li button.active {
    border-color: rgba(0, 102, 204, 0.4);
    background: var(--apple-blue-soft);
  }

  .recording-title {
    color: var(--ink);
    font-size: 13px;
    font-weight: 600;
  }

  .recording-meta {
    color: var(--ink-muted);
    font-size: 11px;
  }

  .list .empty {
    padding: 8px 2px;
  }

  @media (max-width: 980px) {
    .recordings {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) 200px;
    }

    .detail {
      border-right: 0;
      border-bottom: 1px solid var(--hairline);
    }
  }
</style>
