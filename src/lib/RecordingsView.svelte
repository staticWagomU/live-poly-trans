<script lang="ts">
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import {
    buildRecordingTranscript,
    formatFileSize,
    formatTimestampMs,
    groupRecordingsByDate,
    trimTranscript,
    type RecordingFileInfo,
    type RecordingSummary,
    type RecordingTranscriptItem,
    type RecordingWaveform
  } from '$lib/recordings';
  import { createLatestRequestGuard } from '$lib/latestRequest';

  let recordings = $state<RecordingSummary[]>([]);
  let selectedRecording = $state<RecordingSummary | null>(null);
  let selectedFile = $state<RecordingFileInfo | null>(null);
  let transcript = $state<RecordingTranscriptItem[]>([]);
  let waveform = $state<RecordingWaveform | null>(null);
  let audioSrc = $state<string | null>(null);
  let audioElement = $state<HTMLAudioElement | null>(null);
  let waveformCanvas = $state<HTMLCanvasElement | null>(null);
  let currentTimeMs = $state(0);
  let isPlaying = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let isExporting = $state(false);
  let isDeleting = $state(false);
  let isImporting = $state(false);
  let isTrimming = $state(false);
  let isLoadingRecordings = $state(false);
  let isLoadingTranscript = $state(false);
  let isLoadingWaveform = $state(false);
  let actionsOpen = $state(false);
  let isDropTarget = $state(false);

  // Trim selection in source-file milliseconds; null = trim mode off.
  let trimRange = $state<{ startMs: number; endMs: number } | null>(null);
  let trimDragging: 'start' | 'end' | null = null;

  const recordingRequest = createLatestRequestGuard();
  const fileRequest = createLatestRequestGuard();

  const groups = $derived(groupRecordingsByDate(recordings, new Date()));
  const playheadRatio = $derived(
    waveform && waveform.durationMs > 0 ? Math.min(1, currentTimeMs / waveform.durationMs) : 0
  );
  // A trimmed file's transcript is the source transcript cut and shifted to
  // the trim range recorded in meta.
  const activeTrim = $derived(
    selectedFile && selectedRecording?.trims ? (selectedRecording.trims[selectedFile.name] ?? null) : null
  );
  const displayTranscript = $derived(
    activeTrim ? trimTranscript(transcript, activeTrim.startMs, activeTrim.endMs) : transcript
  );
  const activeKey = $derived(activeItemKey(displayTranscript, currentTimeMs));

  onMount(() => {
    void refreshRecordings();

    const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'over') {
        isDropTarget = true;
      } else if (event.payload.type === 'drop') {
        isDropTarget = false;
        void importPaths(event.payload.paths);
      } else {
        isDropTarget = false;
      }
    });

    return () => {
      recordingRequest.invalidate();
      fileRequest.invalidate();
      void unlistenDrop.then((unlisten) => unlisten());
    };
  });

  async function refreshRecordings(selectId: string | null = null) {
    isLoadingRecordings = true;
    error = null;
    try {
      recordings = await invoke<RecordingSummary[]>('list_recordings');
      const wantedId = selectId ?? selectedRecording?.id ?? null;
      const wanted = recordings.find((entry) => entry.id === wantedId) ?? null;
      if (wanted) {
        await selectRecording(wanted);
      } else if (recordings.length > 0) {
        await selectRecording(recordings[0]);
      } else {
        selectedRecording = null;
        selectedFile = null;
        transcript = [];
        waveform = null;
        audioSrc = null;
      }
    } catch (loadError) {
      error = String(loadError);
    } finally {
      isLoadingRecordings = false;
    }
  }

  async function selectRecording(recording: RecordingSummary) {
    const isLatest = recordingRequest.begin();
    fileRequest.invalidate();
    selectedRecording = recording;
    notice = null;
    error = null;
    actionsOpen = false;
    trimRange = null;
    transcript = [];
    isLoadingTranscript = true;
    const fileSelection = selectFile(recording.files[0] ?? null);

    try {
      const events = await invoke<unknown[]>('read_recording_transcript', { id: recording.id });
      if (!isLatest()) {
        return;
      }
      transcript = buildRecordingTranscript(events);
    } catch (transcriptError) {
      if (isLatest()) {
        error = String(transcriptError);
      }
    } finally {
      if (isLatest()) {
        isLoadingTranscript = false;
      }
    }

    await fileSelection;
  }

  async function selectFile(file: RecordingFileInfo | null) {
    const isLatest = fileRequest.begin();
    const recording = selectedRecording;
    selectedFile = file;
    waveform = null;
    currentTimeMs = 0;
    isPlaying = false;
    trimRange = null;
    audioSrc = file ? convertFileSrc(file.path) : null;

    if (!file || !recording) {
      isLoadingWaveform = false;
      drawWaveform();
      return;
    }

    isLoadingWaveform = true;
    try {
      const nextWaveform = await invoke<RecordingWaveform>('recording_waveform', {
        id: recording.id,
        fileName: file.name
      });
      if (!isLatest() || selectedRecording?.id !== recording.id) {
        return;
      }
      waveform = nextWaveform;
    } catch (waveError) {
      if (isLatest()) {
        error = String(waveError);
      }
    } finally {
      if (isLatest()) {
        isLoadingWaveform = false;
      }
    }

    if (isLatest()) {
      drawWaveform();
    }
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
    const playedBars = Math.floor(playheadRatio * waveform.peaks.length);

    for (let index = 0; index < waveform.peaks.length; index++) {
      const amplitude = Math.min(1, waveform.peaks[index]);
      const barHeight = Math.max(2, amplitude * (height - 8));
      context.fillStyle = index <= playedBars ? 'rgba(0, 102, 204, 0.85)' : 'rgba(120, 120, 128, 0.35)';
      context.fillRect(index * barWidth, middle - barHeight / 2, Math.max(1, barWidth - 1), barHeight);
    }
  }

  $effect(() => {
    if (waveformCanvas && waveform && currentTimeMs >= 0) {
      drawWaveform();
    }
  });

  function handleTimeUpdate() {
    if (audioElement) {
      currentTimeMs = audioElement.currentTime * 1000;
    }
  }

  function togglePlayback() {
    if (!audioElement) {
      return;
    }

    if (isPlaying) {
      audioElement.pause();
    } else {
      void audioElement.play();
    }
  }

  function seekTo(startMs: number) {
    if (!audioElement) {
      return;
    }

    audioElement.currentTime = startMs / 1000;
    void audioElement.play();
  }

  function waveformRatioFromEvent(event: PointerEvent | MouseEvent): number {
    if (!waveformCanvas) {
      return 0;
    }
    const rect = waveformCanvas.getBoundingClientRect();
    return Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
  }

  function seekFromWaveform(event: MouseEvent) {
    if (!waveform || waveform.durationMs <= 0 || trimRange) {
      return;
    }
    seekTo(waveformRatioFromEvent(event) * waveform.durationMs);
  }

  // ---- trim ----

  function startTrim() {
    if (!waveform || waveform.durationMs <= 0) {
      return;
    }
    actionsOpen = false;
    trimRange = { startMs: 0, endMs: waveform.durationMs };
  }

  function beginHandleDrag(handle: 'start' | 'end', event: PointerEvent) {
    event.preventDefault();
    trimDragging = handle;
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function moveHandleDrag(event: PointerEvent) {
    if (!trimDragging || !trimRange || !waveform) {
      return;
    }

    const ms = waveformRatioFromEvent(event) * waveform.durationMs;
    if (trimDragging === 'start') {
      trimRange = { ...trimRange, startMs: Math.min(ms, trimRange.endMs - 1000) };
    } else {
      trimRange = { ...trimRange, endMs: Math.max(ms, trimRange.startMs + 1000) };
    }
  }

  function endHandleDrag() {
    trimDragging = null;
  }

  async function applyTrim() {
    const recording = selectedRecording;
    const file = selectedFile;
    const range = trimRange;
    if (!recording || !file || !range || isTrimming) {
      return;
    }

    const confirmed = window.confirm(
      `選択した範囲(${formatTimestampMs(range.startMs)} – ${formatTimestampMs(range.endMs)})だけを残した新しいファイルを作成します。元のファイルはそのまま残ります。続けますか?`
    );
    if (!confirmed) {
      return;
    }

    isTrimming = true;
    error = null;
    try {
      const outputName = await invoke<string>('trim_recording', {
        id: recording.id,
        fileName: file.name,
        startMs: Math.round(range.startMs),
        endMs: Math.round(range.endMs)
      });
      trimRange = null;
      notice = `トリムしたファイルを作成しました: ${outputName}`;
      await refreshRecordings(recording.id);
      const created = selectedRecording?.files.find((entry) => entry.name === outputName);
      if (created) {
        await selectFile(created);
      }
    } catch (trimError) {
      error = String(trimError);
    } finally {
      isTrimming = false;
    }
  }

  // ---- import ----

  async function pickAndImport() {
    const picked = await openFileDialog({
      multiple: true,
      filters: [{ name: '音声ファイル', extensions: ['m4a', 'wav', 'mp3', 'aac'] }]
    });
    if (!picked) {
      return;
    }
    await importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  async function importPaths(paths: string[]) {
    const audioPaths = paths.filter((path) => /\.(m4a|wav|mp3|aac)$/i.test(path));
    if (audioPaths.length === 0 || isImporting) {
      return;
    }

    isImporting = true;
    error = null;
    try {
      let lastId: string | null = null;
      for (const path of audioPaths) {
        const created = await invoke<{ id: string; dir: string }>('import_audio_file', { path });
        lastId = created.id;
      }
      notice = '音声ファイルを読み込みました。「再処理」で文字起こしを作成できます。';
      await refreshRecordings(lastId);
    } catch (importError) {
      error = String(importError);
    } finally {
      isImporting = false;
    }
  }

  // ---- export / delete ----

  async function exportVariant(variant: 'mic' | 'speaker' | 'mixed') {
    if (!selectedRecording || isExporting) {
      return;
    }

    actionsOpen = false;
    isExporting = true;
    notice = null;
    error = null;
    try {
      const destination = await invoke<string>('export_recording', {
        id: selectedRecording.id,
        variant
      });
      notice = `書き出しました: ${destination}`;
    } catch (exportError) {
      error = String(exportError);
    } finally {
      isExporting = false;
    }
  }

  async function deleteSelectedRecording() {
    const recording = selectedRecording;
    if (!recording || isDeleting) {
      return;
    }

    actionsOpen = false;
    const confirmed = window.confirm(
      'この録音と音声・文字起こしファイルを完全に削除します。この操作は取り消せません。削除しますか?'
    );
    if (!confirmed) {
      return;
    }

    isDeleting = true;
    error = null;
    try {
      await invoke('delete_recording', { id: recording.id });
      recordingRequest.invalidate();
      fileRequest.invalidate();
      selectedRecording = null;
      await refreshRecordings();
    } catch (deleteError) {
      error = String(deleteError);
    } finally {
      isDeleting = false;
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

  function hasStream(stream: string) {
    return selectedRecording?.files.some((file) => file.stream === stream) ?? false;
  }

  function recordingTitle(recording: RecordingSummary) {
    const date = new Date(recording.startedAt);
    const title = Number.isNaN(date.getTime())
      ? recording.id
      : date.toLocaleString('ja-JP', { dateStyle: 'medium', timeStyle: 'short' });
    return recording.source === 'external' ? `${title}(読み込み)` : title;
  }

  function recordingStreamsLabel(recording: RecordingSummary) {
    const streams = recording.files
      .map((file) => file.stream)
      .filter((stream, index, all) => all.indexOf(stream) === index);
    return streams.join(' + ') || '音声なし';
  }
</script>

<svelte:window on:click={() => (actionsOpen = false)} />

<div class="recordings" class:drop-target={isDropTarget}>
  <aside class="rec-side" aria-label="録音一覧">
    <button type="button" class="upload-btn" disabled={isImporting} onclick={pickAndImport}>
      {isImporting ? '読み込み中…' : '＋ 音声ファイルを読み込む'}
    </button>
    {#each groups as group (group.label)}
      <div class="group">{group.label}</div>
      {#each group.recordings as recording (recording.id)}
        <button
          type="button"
          class="rec-item"
          class:active={selectedRecording?.id === recording.id}
          onclick={() => selectRecording(recording)}
        >
          <span class="t">{recordingTitle(recording)}</span>
          <span class="m">
            <span>{recordingStreamsLabel(recording)}</span>
          </span>
        </button>
      {/each}
    {/each}
    {#if recordings.length === 0}
      <p class="side-empty">
        {isLoadingRecordings
          ? '読み込み中…'
          : error
            ? '一覧を読み込めませんでした。'
            : 'まだ録音がありません。'}
      </p>
    {/if}
  </aside>

  <section class="rec-detail" aria-label="録音の詳細">
    {#if selectedRecording}
      <div class="rec-head-row">
        <div>
          <h2>{recordingTitle(selectedRecording)}</h2>
          <p class="meta">
            {waveform ? `${formatTimestampMs(waveform.durationMs)} · ` : ''}
            {recordingStreamsLabel(selectedRecording)} · このMacに保存
          </p>
        </div>
        <div class="menu-anchor">
          <button
            type="button"
            class="pill-btn"
            aria-haspopup="menu"
            aria-expanded={actionsOpen}
            onclick={(event) => {
              event.stopPropagation();
              actionsOpen = !actionsOpen;
            }}
          >
            アクション <span class="chev">▾</span>
          </button>
          {#if actionsOpen}
            <nav class="menu">
              <button
                type="button"
                class="mi"
                disabled={isExporting || !hasStream('mic')}
                onclick={() => exportVariant('mic')}
              >
                書き出し: Mic 音声…
              </button>
              <button
                type="button"
                class="mi"
                disabled={isExporting || !hasStream('speaker')}
                onclick={() => exportVariant('speaker')}
              >
                書き出し: Speaker 音声…
              </button>
              <button
                type="button"
                class="mi"
                disabled={isExporting || !hasStream('mic') || !hasStream('speaker')}
                onclick={() => exportVariant('mixed')}
              >
                書き出し: ミックス音声…
              </button>
              <div class="sep"></div>
              <button
                type="button"
                class="mi"
                disabled={!waveform || waveform.durationMs <= 0 || isTrimming}
                onclick={startTrim}
              >
                ✂︎ トリム…
              </button>
              <div class="sep"></div>
              <button
                type="button"
                class="mi danger"
                disabled={isDeleting || selectedRecording.endedAt === null}
                onclick={deleteSelectedRecording}
              >
                削除…
              </button>
            </nav>
          {/if}
        </div>
      </div>

      {#if error}
        <p class="notice error">{error}</p>
      {/if}
      {#if notice}
        <p class="notice">{notice}</p>
      {/if}

      {#if selectedRecording.files.length > 1}
        <div class="file-chips" aria-label="録音ファイル">
          {#each selectedRecording.files as file (file.name)}
            <button
              type="button"
              class:active={selectedFile?.name === file.name}
              onclick={() => selectFile(file)}
            >
              {file.name} · {formatFileSize(file.sizeBytes)}
            </button>
          {/each}
        </div>
      {/if}

      <div class="player">
        <button
          type="button"
          class="play-btn"
          disabled={!audioSrc}
          aria-label={isPlaying ? '一時停止' : '再生'}
          onclick={togglePlayback}
        >
          {isPlaying ? '❚❚' : '▶'}
        </button>
        <!-- svelte-ignore a11y_no_static_element_interactions -- pointer
             handlers here only forward drags started on the slider handles -->
        <div
          class="wave"
          onpointermove={moveHandleDrag}
          onpointerup={endHandleDrag}
        >
          <canvas
            bind:this={waveformCanvas}
            class="waveform"
            onclick={seekFromWaveform}
            aria-label="音声波形"
          ></canvas>
          {#if !trimRange}
            <div class="playhead" style={`left: ${playheadRatio * 100}%`} aria-hidden="true"></div>
          {/if}
          {#if trimRange && waveform && waveform.durationMs > 0}
            <div
              class="trim-shade"
              style={`left: 0; width: ${(trimRange.startMs / waveform.durationMs) * 100}%`}
            ></div>
            <div
              class="trim-shade"
              style={`left: ${(trimRange.endMs / waveform.durationMs) * 100}%; right: 0`}
            ></div>
            <div
              class="trim-handle"
              role="slider"
              aria-label="トリム開始位置"
              aria-valuenow={Math.round(trimRange.startMs / 1000)}
              tabindex="0"
              style={`left: ${(trimRange.startMs / waveform.durationMs) * 100}%`}
              onpointerdown={(event) => beginHandleDrag('start', event)}
            ></div>
            <div
              class="trim-handle"
              role="slider"
              aria-label="トリム終了位置"
              aria-valuenow={Math.round(trimRange.endMs / 1000)}
              tabindex="0"
              style={`left: ${(trimRange.endMs / waveform.durationMs) * 100}%`}
              onpointerdown={(event) => beginHandleDrag('end', event)}
            ></div>
          {/if}
          {#if isLoadingWaveform}
            <span class="waveform-loading" role="status">波形を読み込み中…</span>
          {/if}
        </div>
        <span class="wave-time">
          {formatTimestampMs(currentTimeMs)} / {waveform ? formatTimestampMs(waveform.durationMs) : '--:--'}
        </span>
      </div>

      {#if trimRange}
        <div class="trim-bar">
          <span class="trim-title">✂︎ トリム</span>
          <span class="trim-range">
            {formatTimestampMs(trimRange.startMs)} – {formatTimestampMs(trimRange.endMs)}
          </span>
          <span>選択した範囲だけを残します</span>
          <div class="spacer"></div>
          <button type="button" class="quiet-btn" onclick={() => (trimRange = null)}>
            キャンセル
          </button>
          <button type="button" class="btn-apply" disabled={isTrimming} onclick={applyTrim}>
            {isTrimming ? '適用中…' : '適用'}
          </button>
        </div>
      {/if}

      {#if audioSrc}
        <audio
          bind:this={audioElement}
          src={audioSrc}
          ontimeupdate={handleTimeUpdate}
          onplay={() => (isPlaying = true)}
          onpause={() => (isPlaying = false)}
          onended={() => (isPlaying = false)}
        ></audio>
      {/if}

      <section class="rec-transcript" aria-label="文字起こし">
        <div class="tr-head">
          <h3>文字起こし</h3>
        </div>
        {#if isLoadingTranscript}
          <p class="empty" role="status">文字起こしを読み込み中…</p>
        {:else if displayTranscript.length === 0}
          <p class="empty">
            {selectedRecording.source === 'external'
              ? 'この音声にはまだ文字起こしがありません。'
              : 'この録音には確定した文字起こしがありません。'}
          </p>
        {:else}
          <div class="tr-list">
            {#each displayTranscript as item (item.key)}
              <button
                type="button"
                class="tr-row"
                class:now={item.key === activeKey}
                onclick={() => seekTo(item.startMs)}
              >
                <span class="ts">{formatTimestampMs(item.startMs)}</span>
                <span class="body">
                  <span class="spk"><i class:mic={item.stream === 'mic'}></i>{item.speakerLabel}</span>
                  <span class="text">{item.text}</span>
                  {#if item.translation}
                    <span class="sub">{item.translation}</span>
                  {/if}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </section>
    {:else}
      <div class="empty-state">
        {#if error}
          <!-- A failed list must not masquerade as "no recordings": it would
               send the user to change a setting they already have on. -->
          <h2>録音を読み込めませんでした</h2>
          <p class="notice error">{error}</p>
          <button type="button" class="retry" onclick={() => refreshRecordings()}>再試行</button>
        {:else if isLoadingRecordings}
          <h2>読み込み中…</h2>
        {:else}
          <h2>まだ録音がありません</h2>
          <p>
            Live 画面の「録音」を押すと会話がここに保存されます。スマホやボイスレコーダーの
            音声ファイルをドラッグ&ドロップして読み込むこともできます。
          </p>
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  button {
    font: inherit;
    cursor: pointer;
    color: inherit;
    background: none;
    border: 0;
  }

  button:focus-visible {
    outline: 2px solid var(--blue-focus);
    outline-offset: 2px;
    border-radius: 8px;
  }

  .recordings {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    min-height: 0;
    height: 100%;
    background: var(--canvas);
  }

  .recordings.drop-target {
    outline: 2px dashed var(--blue);
    outline-offset: -4px;
  }

  .rec-side {
    border-right: 1px solid var(--divider);
    background: var(--parchment);
    overflow-y: auto;
    padding: 12px 10px;
  }

  .upload-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: calc(100% - 8px);
    margin: 4px 4px 10px;
    padding: 9px 11px;
    border-radius: 9px;
    border: 1px dashed var(--hairline);
    color: var(--blue);
    font-size: 12.5px;
    font-weight: 600;
    transition: all 0.18s ease;
  }

  .upload-btn:hover:not(:disabled) {
    background: var(--blue-soft);
    border-color: rgba(0, 102, 204, 0.35);
  }

  .upload-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .group {
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    padding: 10px 8px 5px;
  }

  .rec-item {
    width: 100%;
    text-align: left;
    padding: 9px 10px;
    border-radius: 9px;
    display: grid;
    gap: 2px;
  }

  .rec-item:hover {
    background: var(--hover-wash);
  }

  .rec-item.active {
    background: var(--blue);
    color: #fff;
  }

  .rec-item .t {
    font-size: 13px;
    font-weight: 600;
  }

  .rec-item .m {
    font-size: 11.5px;
    color: var(--muted);
    display: flex;
    justify-content: space-between;
  }

  .rec-item.active .m {
    color: rgba(255, 255, 255, 0.75);
  }

  .side-empty {
    margin: 8px;
    color: var(--muted);
    font-size: 12.5px;
  }

  .rec-detail {
    overflow-y: auto;
    padding: 26px 32px;
    position: relative;
  }

  .rec-head-row {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 12px;
  }

  .rec-detail h2 {
    margin: 0 0 3px;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.015em;
    color: var(--ink);
  }

  .rec-detail .meta {
    font-size: 12.5px;
    color: var(--muted);
    margin: 0 0 20px;
  }

  .menu-anchor {
    position: relative;
  }

  .pill-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 13px;
    border-radius: 999px;
    background: var(--canvas);
    border: 1px solid var(--hairline);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    white-space: nowrap;
  }

  .pill-btn:hover {
    border-color: rgba(0, 102, 204, 0.35);
  }

  .pill-btn .chev {
    font-size: 9px;
    color: var(--muted);
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    border-radius: 12px;
    background: var(--menu-bg);
    backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid var(--hairline);
    box-shadow: var(--shadow-menu);
    padding: 5px;
    min-width: 220px;
    z-index: 50;
  }

  .menu .mi {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 11px;
    border-radius: 8px;
    font-size: 13px;
    text-align: left;
    color: var(--ink);
  }

  .menu .mi:hover:not(:disabled) {
    background: var(--blue);
    color: #fff;
  }

  .menu .mi:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .menu .mi.danger {
    color: var(--red);
  }

  .menu .mi.danger:hover:not(:disabled) {
    background: var(--red);
    color: #fff;
  }

  .menu .sep {
    height: 1px;
    background: var(--divider);
    margin: 5px 8px;
  }

  .notice {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 12px;
    word-break: break-all;
  }

  .notice.error {
    color: var(--red);
  }

  .file-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }

  .file-chips button {
    border: 1px solid var(--hairline);
    border-radius: 999px;
    background: var(--canvas);
    color: var(--muted);
    padding: 5px 11px;
    font-size: 12px;
    font-weight: 600;
  }

  .file-chips button.active {
    border-color: rgba(0, 102, 204, 0.4);
    background: var(--blue-soft);
    color: var(--blue);
  }

  .player {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 24px;
  }

  .play-btn {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: var(--parchment);
    border: 1px solid var(--divider);
    display: grid;
    place-items: center;
    font-size: 15px;
    flex: 0 0 auto;
    transition: transform 0.15s ease;
    color: var(--ink);
  }

  .play-btn:active:not(:disabled) {
    transform: scale(0.94);
  }

  .play-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .wave {
    flex: 1;
    height: 44px;
    position: relative;
  }

  .waveform {
    width: 100%;
    height: 100%;
    cursor: pointer;
    display: block;
  }

  .playhead {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 1.5px;
    background: var(--red);
    pointer-events: none;
  }

  .trim-shade {
    position: absolute;
    top: 0;
    bottom: 0;
    background: rgba(28, 28, 30, 0.32);
    border-radius: 3px;
    pointer-events: none;
    z-index: 3;
  }

  .trim-handle {
    position: absolute;
    top: -5px;
    bottom: -5px;
    width: 14px;
    border-radius: 5px;
    background: var(--blue);
    cursor: ew-resize;
    z-index: 5;
    transform: translateX(-50%);
    display: grid;
    place-items: center;
    touch-action: none;
  }

  .trim-handle::after {
    content: '';
    width: 2px;
    height: 14px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.85);
  }

  .waveform-loading {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--canvas) 82%, transparent);
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    pointer-events: none;
  }

  .wave-time {
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .trim-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: -10px 0 20px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .trim-title {
    font-weight: 600;
    color: var(--ink);
  }

  .trim-range {
    font-variant-numeric: tabular-nums;
    color: var(--blue);
    font-weight: 600;
  }

  .spacer {
    flex: 1;
  }

  .quiet-btn {
    padding: 5px 11px;
    border-radius: 7px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
  }

  .quiet-btn:hover {
    background: var(--hover-wash);
    color: var(--ink);
  }

  .btn-apply {
    padding: 6px 15px;
    border-radius: 999px;
    background: var(--blue);
    color: #fff;
    font-size: 12.5px;
    font-weight: 600;
    transition: transform 0.15s ease;
  }

  .btn-apply:active:not(:disabled) {
    transform: scale(0.95);
  }

  .btn-apply:disabled {
    opacity: 0.6;
    cursor: default;
  }

  audio {
    display: none;
  }

  .tr-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .tr-head h3 {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
    color: var(--ink);
  }

  .tr-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tr-row {
    display: grid;
    grid-template-columns: 52px 1fr;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 9px;
    font-size: 13.5px;
    line-height: 1.5;
    text-align: left;
    width: 100%;
  }

  .tr-row:hover {
    background: var(--parchment);
  }

  .tr-row.now {
    background: var(--blue-soft);
  }

  .tr-row .ts {
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    padding-top: 2px;
  }

  .tr-row .body {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .spk {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 600;
    font-size: 12px;
    color: var(--muted);
  }

  .spk i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }

  .spk i.mic {
    background: var(--blue);
  }

  .tr-row .text {
    color: var(--ink);
    word-break: break-word;
  }

  .tr-row .sub {
    color: var(--muted);
    font-size: 12.5px;
    word-break: break-word;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }

  .empty-state {
    display: grid;
    place-content: center;
    height: 100%;
    text-align: center;
    gap: 6px;
  }

  .empty-state h2 {
    margin: 0;
    color: var(--ink);
    font-size: 17px;
  }

  .empty-state p {
    margin: 0;
    max-width: 380px;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.6;
  }

  .empty-state .retry {
    justify-self: center;
    margin-top: 8px;
    border: 1px solid rgba(0, 102, 204, 0.16);
    border-radius: 8px;
    background: var(--blue-soft);
    color: var(--blue);
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
  }

  @media (max-width: 900px) {
    .recordings {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: 180px minmax(0, 1fr);
    }

    .rec-side {
      border-right: 0;
      border-bottom: 1px solid var(--divider);
    }
  }
</style>
