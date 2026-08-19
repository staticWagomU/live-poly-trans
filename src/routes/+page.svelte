<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  type Lane = 'mic' | 'speaker';
  /// A finished utterance, positioned in this session's recording.
  type Utterance = {
    id: number;
    text: string;
    startMs: number;
    endMs: number;
    lang: string | null;
    /// A translation is on its way; leave room for it.
    translating: boolean;
  };
  type TranscriptPayload = {
    lane: Lane;
    committedDelta: string;
    volatile: string;
    utteranceFinal: Utterance | null;
  };
  /// `text: null` means this utterance is not getting a translation after all
  /// (the queue overflowed, or the backend failed).
  type TranslationPayload = { id: number; lane: Lane; text: string | null; lang: string | null };
  type StatusPayload = {
    /// `stopping` is the drain: capture has ended and the last sentences are
    /// still being translated.
    state: 'idle' | 'loading' | 'listening' | 'stopping' | 'error';
    message: string | null;
    /// Directory this session's WAV files are being written to; stays put
    /// after a stop so the files can still be found.
    recordingDir: string | null;
  };
  type Languages = { spoken: string[]; target: string | null; mutual: boolean };

  // Cap the transcript so an hours-long session doesn't grow the DOM
  // without bound; the oldest lines scroll away first anyway.
  const MAX_LINES = 500;

  // Mic and speaker get separate columns: the two lanes are transcribed
  // independently and interleaving them would misattribute who said what.
  const LANES: { id: Lane; label: string }[] = [
    { id: 'mic', label: 'マイク' },
    { id: 'speaker', label: 'スピーカー' }
  ];

  /// The languages offered, labelled as they label themselves — no flags: a
  /// language is not a country (English is not only the US, and representing
  /// Arabic with a Saudi flag is worse than lazy).
  const LANGUAGES: { code: string; label: string }[] = [
    { code: 'ja', label: '日本語' },
    { code: 'en', label: 'English' },
    { code: 'zh', label: '中文' },
    { code: 'ko', label: '한국어' },
    { code: 'es', label: 'Español' },
    { code: 'fr', label: 'Français' },
    { code: 'de', label: 'Deutsch' },
    { code: 'pt', label: 'Português' },
    { code: 'ru', label: 'Русский' },
    { code: 'it', label: 'Italiano' },
    { code: 'id', label: 'Bahasa Indonesia' },
    { code: 'vi', label: 'Tiếng Việt' },
    { code: 'th', label: 'ไทย' },
    { code: 'hi', label: 'हिन्दी' },
    { code: 'ar', label: 'العربية' }
  ];
  const labelOf = (code: string) => LANGUAGES.find((l) => l.code === code)?.label ?? code;

  /// A settled utterance. `id` comes from the backend so a translation
  /// arriving seconds later can find its line — including across a stop and
  /// a fresh Record.
  type Line = {
    id: number;
    startMs: number;
    text: string;
    translation: string | null;
    translating: boolean;
  };
  /// `pending` is the text committed for the utterance still being spoken;
  /// it becomes a line once the utterance ends and gets its timestamp.
  type LaneState = { lines: Line[]; pending: string; volatile: string };

  const emptyLane = (): LaneState => ({ lines: [], pending: '', volatile: '' });
  let lanes = $state<Record<Lane, LaneState>>({ mic: emptyLane(), speaker: emptyLane() });
  let status = $state<StatusPayload>({ state: 'idle', message: null, recordingDir: null });
  let languages = $state<Languages>({ spoken: ['ja', 'en'], target: 'ja', mutual: false });
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
        lane.lines.push({
          id: finished.id,
          startMs: finished.startMs,
          text: finished.text,
          translation: null,
          translating: finished.translating
        });
        lane.lines.splice(0, lane.lines.length - MAX_LINES);
        lane.pending = '';
      }
      lane.volatile = event.payload.volatile;
    });
    // Translations arrive seconds after their sentence and out of order
    // between lanes, so they are matched by id rather than by position.
    const unlistenTranslation = listen<TranslationPayload>('translation', (event) => {
      const line = lanes[event.payload.lane].lines.find((l) => l.id === event.payload.id);
      if (!line) return; // scrolled off the top, or from a cleared session
      line.translation = event.payload.text;
      line.translating = false;
    });
    const unlistenStatus = listen<StatusPayload>('status', (event) => {
      status = event.payload;
    });
    // Status events only fire on change; ask for the current snapshot so a
    // (re)loaded webview doesn't show "idle" while the backend is listening.
    invoke<StatusPayload>('get_status').then((s) => {
      status = s;
    });
    invoke<Languages>('get_languages').then((l) => {
      languages = l;
    });
    return () => {
      unlistenTranscript.then((fn) => fn());
      unlistenTranslation.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  });

  /// The pill: the symbol itself says which of the four states this is —
  /// none = single language, untranslated; → = one way; ⇄ = mutual;
  /// · = several languages, untranslated.
  const pill = $derived.by(() => {
    const spoken = languages.spoken.map(labelOf);
    if (!languages.target) return spoken.join(' · ');
    const target = labelOf(languages.target);
    const other = languages.spoken.find((code) => code !== languages.target);
    if (languages.mutual && other) return `${target} ⇄ ${labelOf(other)}`;
    return `${other ? labelOf(other) : spoken[0]} → ${target}`;
  });
  // Mutual only means something with a second language to translate into.
  const canBeMutual = $derived(
    languages.spoken.length === 2 && !!languages.target && languages.spoken.includes(languages.target)
  );

  async function applyLanguages(next: Languages) {
    const previous = languages;
    languages = next; // optimistic: the control must not lag the click
    try {
      await invoke('set_languages', { languages: next });
    } catch (error) {
      languages = previous;
      status = { ...status, state: 'error', message: String(error) };
    }
  }

  function setSpoken(index: number, code: string) {
    const spoken = [...languages.spoken];
    if (code === '') {
      // Dropping the second language: detection stops running at all.
      spoken.splice(index, 1);
    } else {
      spoken[index] = code;
    }
    const deduped = spoken.filter((c, i) => spoken.indexOf(c) === i);
    applyLanguages({ ...languages, spoken: deduped, mutual: deduped.length === 2 && languages.mutual });
  }

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
      <details class="lang">
        <summary title="言語と翻訳の設定">{pill}</summary>
        <div class="lang-pop">
          <div class="pop-title">話される言語</div>
          <p class="pop-lead">候補が少ないほど誤認識が減ります。1つなら言語判定を行いません。</p>
          <div class="pop-row">
            <select
              aria-label="話される言語 1"
              value={languages.spoken[0] ?? 'ja'}
              onchange={(e) => setSpoken(0, e.currentTarget.value)}
            >
              {#each LANGUAGES as l (l.code)}
                <option value={l.code}>{l.label}</option>
              {/each}
            </select>
            <select
              aria-label="話される言語 2"
              value={languages.spoken[1] ?? ''}
              onchange={(e) => setSpoken(1, e.currentTarget.value)}
            >
              <option value="">（なし）</option>
              {#each LANGUAGES as l (l.code)}
                <option value={l.code}>{l.label}</option>
              {/each}
            </select>
          </div>

          <div class="pop-title">翻訳</div>
          <div class="pop-row">
            <select
              aria-label="翻訳先の言語"
              value={languages.target ?? ''}
              onchange={(e) =>
                applyLanguages({ ...languages, target: e.currentTarget.value || null })}
            >
              <option value="">訳さない</option>
              {#each LANGUAGES as l (l.code)}
                <option value={l.code}>{l.label} へ</option>
              {/each}
            </select>
          </div>
          {#if canBeMutual}
            <label class="pop-check">
              <input
                type="checkbox"
                checked={languages.mutual}
                onchange={(e) =>
                  applyLanguages({ ...languages, mutual: e.currentTarget.checked })}
              />
              もう一方の言語にも訳す
            </label>
          {/if}
          <p class="pop-lead">変更は次の発話から反映されます。</p>
        </div>
      </details>
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
            <p>
              <span class="at">{clock(line.startMs)}</span>{line.text}
              {#if line.translation}
                <span class="translation">{line.translation}</span>
              {:else if line.translating}
                <span class="translation waiting">訳しています…</span>
              {/if}
            </p>
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
  /* The translation reads as a second voice under the sentence, not as part
     of it: its own line, dimmer, marked by the rule down the side. */
  .translation {
    display: block;
    margin-top: 0.1rem;
    padding-left: 0.6rem;
    border-left: 2px solid #2a3138;
    color: #9fb4c7;
    font-size: 0.95rem;
    line-height: 1.6;
  }
  .translation.waiting {
    color: #4c5866;
    font-style: italic;
  }
  .lang {
    position: relative;
  }
  .lang summary {
    list-style: none;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.3rem 0.7rem;
    border: 1px solid #3a434c;
    border-radius: 999px;
    color: #c7d3de;
    user-select: none;
  }
  .lang summary::-webkit-details-marker {
    display: none;
  }
  .lang[open] summary {
    border-color: #5d6a76;
  }
  .lang-pop {
    position: absolute;
    top: calc(100% + 0.4rem);
    right: 0;
    z-index: 10;
    width: 17rem;
    padding: 0.75rem;
    background: #161c22;
    border: 1px solid #2a3138;
    border-radius: 10px;
    box-shadow: 0 12px 32px rgb(0 0 0 / 45%);
  }
  .pop-title {
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #8b98a5;
    margin-bottom: 0.35rem;
  }
  .pop-lead {
    margin: 0 0 0.6rem;
    font-size: 0.7rem;
    line-height: 1.5;
    color: #5d6a76;
  }
  .pop-row {
    display: flex;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }
  .pop-row select {
    flex: 1;
    min-width: 0;
    background: #1a2026;
    color: #e8edf2;
    border: 1px solid #3a434c;
    border-radius: 6px;
    padding: 0.3rem 0.4rem;
    font-size: 0.8rem;
  }
  .pop-check {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: #c7d3de;
    margin-bottom: 0.75rem;
  }
</style>
