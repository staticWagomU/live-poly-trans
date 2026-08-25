<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import LanguagePopover from '$lib/LanguagePopover.svelte';
  import { pillText, type Languages } from '$lib/languages';
  import { clock, dayGroup, durationLabel, sessionStart, timeOfDay } from '$lib/format';

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
  /// One session on disk, as the library lists it.
  type Recording = {
    name: string;
    dir: string;
    title: string | null;
    startedAtMs: number | null;
    durationMs: number;
    lanes: Lane[];
    snippet: string;
    utterances: number;
  };
  /// A past session, read back from its transcript.
  type Session = {
    name: string;
    dir: string;
    title: string | null;
    startedAtMs: number | null;
    durationMs: number;
    lanes: Lane[];
    lines: {
      id: number;
      lane: Lane;
      startMs: number;
      endMs: number;
      lang: string | null;
      text: string;
      translation: string | null;
    }[];
  };

  // Cap the transcript so an hours-long session doesn't grow the DOM
  // without bound; the oldest lines scroll away first anyway.
  const MAX_LINES = 500;

  const LANE_LABELS: Record<Lane, string> = { mic: 'マイク', speaker: 'スピーカー' };
  const LANES: Lane[] = ['mic', 'speaker'];

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

  /// `home` is the library; `session` is one recording — the live one when
  /// `opened` is null, otherwise the one read back off disk.
  let view = $state<'home' | 'session'>('home');
  let opened = $state<Session | null>(null);
  let recordings = $state<Recording[]>([]);
  let search = $state('');
  let langOpen = $state(false);
  let actionsOpen = $state(false);
  let editingTitle = $state(false);
  let titleDraft = $state('');
  let savingTitle = $state(false);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  // The backend is the source of truth for running: a capture error flips
  // it back to idle/error even though the Record invoke itself succeeded.
  const running = $derived(status.state === 'loading' || status.state === 'listening');
  // The folder name alone: it is the session's timestamp, and the full path
  // is a tooltip away.
  const recordingName = $derived(status.recordingDir?.split(/[\\/]/).pop() ?? null);

  function showToast(message: string) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2400);
  }

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
      const wasRunning = running;
      status = event.payload;
      // A session that just ended is a new row in the library.
      if (wasRunning && !running) void refresh();
    });
    // Status events only fire on change; ask for the current snapshot so a
    // (re)loaded webview doesn't show "idle" while the backend is listening.
    invoke<StatusPayload>('get_status').then((s) => {
      status = s;
      // Reloading mid-session lands on the session, not on the library: the
      // recording is what the window is for while it runs.
      if (s.state === 'loading' || s.state === 'listening') view = 'session';
    });
    invoke<Languages>('get_languages').then((l) => {
      languages = l;
    });
    void refresh();
    return () => {
      unlistenTranscript.then((fn) => fn());
      unlistenTranslation.then((fn) => fn());
      unlistenStatus.then((fn) => fn());
    };
  });

  async function refresh() {
    try {
      recordings = await invoke<Recording[]>('list_recordings');
    } catch (error) {
      showToast(`録音一覧を読めませんでした: ${error}`);
    }
  }

  // ── The running session's clock ──────────────────────────────────────
  // Counted from the moment the recording directory is named after, not from
  // when this page mounted, so a reloaded webview shows the true elapsed time.
  let nowMs = $state(Date.now());
  $effect(() => {
    if (!running) return;
    const timer = setInterval(() => (nowMs = Date.now()), 1000);
    return () => clearInterval(timer);
  });
  const startedAt = $derived(sessionStart(recordingName));
  const elapsed = $derived(startedAt === null ? 0 : Math.max(0, nowMs - startedAt));

  // ── The transcript as one stream ─────────────────────────────────────
  // Mic and speaker share the recording's clock to within ~15ms (plan.md
  // Step 4), so ordering by `startMs` reconstructs the conversation. The
  // speaker pill carries the attribution the two columns used to.
  type StreamLine = {
    key: string;
    lane: Lane;
    startMs: number;
    text: string;
    translation: string | null;
    translating: boolean;
  };

  const stream = $derived.by<StreamLine[]>(() => {
    if (opened) {
      return opened.lines.map((l) => ({
        key: `${l.lane}-${l.id}`,
        lane: l.lane,
        startMs: l.startMs,
        text: l.text,
        translation: l.translation,
        translating: false
      }));
    }
    return LANES.flatMap((lane) =>
      lanes[lane].lines.map((l) => ({
        key: `${lane}-${l.id}`,
        lane,
        startMs: l.startMs,
        text: l.text,
        translation: l.translation,
        translating: l.translating
      }))
    ).sort((a, b) => a.startMs - b.startMs || a.key.localeCompare(b.key));
  });

  /// The utterance each lane is still in the middle of. It has no settled
  /// position yet, so it sits after everything that does.
  const inFlight = $derived(
    opened ? [] : LANES.filter((lane) => lanes[lane].pending || lanes[lane].volatile)
  );

  const hasTranscript = $derived(stream.length > 0 || inFlight.length > 0);

  // ── Following the tail ───────────────────────────────────────────────
  // Follow the live text like a teleprompter — but only while the reader is
  // at the tail. Scrolling up to reread history pauses the follow; returning
  // near the bottom resumes it.
  let follow = true;
  const FOLLOW_SLACK_PX = 48;

  function onTranscriptScroll(event: Event) {
    const el = event.currentTarget as HTMLElement;
    follow = el.scrollHeight - el.scrollTop - el.clientHeight < FOLLOW_SLACK_PX;
  }

  // Re-runs whenever the transcript changes because the attachment reads it.
  function followTail(el: HTMLElement) {
    void stream.length;
    void inFlight.length;
    void lanes.mic.pending;
    void lanes.mic.volatile;
    void lanes.speaker.pending;
    void lanes.speaker.volatile;
    if (follow) el.scrollTo({ top: el.scrollHeight });
  }

  // ── Session identity ─────────────────────────────────────────────────
  const sessionTitle = $derived.by(() => {
    if (!opened) return '新しい録音';
    if (opened.title) return opened.title;
    if (opened.startedAtMs === null) return opened.name;
    const d = new Date(opened.startedAtMs);
    return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()} ${timeOfDay(opened.startedAtMs)}`;
  });

  /// What the badge says, straight off the pipeline's state — including the
  /// drain after Stop, when the last sentences are still being translated.
  const badge = $derived.by(() => {
    if (opened) return { tone: 'done', glyph: '✓', text: '文字起こし' };
    switch (status.state) {
      case 'loading':
        return { tone: 'busy', glyph: '◌', text: 'モデルを読み込み中' };
      case 'listening':
        return { tone: 'live', glyph: '●', text: '録音中' };
      case 'stopping':
        return { tone: 'busy', glyph: '◌', text: '残りを翻訳中' };
      case 'error':
        return { tone: 'error', glyph: '⚠', text: status.message ?? 'エラー' };
      default:
        return { tone: 'done', glyph: '✓', text: '文字起こし完了' };
    }
  });

  // ── Actions ──────────────────────────────────────────────────────────
  async function toggle() {
    busy = true;
    const wasRunning = running;
    try {
      await invoke(wasRunning ? 'stop_capture' : 'start_capture');
      if (!wasRunning) {
        // The backend's status event may lag the accepted Start; reflect it
        // now so a quick second click means Stop, not another Start.
        if (!running) status = { ...status, state: 'loading', message: null };
        startLive();
      }
    } catch (error) {
      status = { ...status, state: 'error', message: String(error) };
    } finally {
      busy = false;
    }
  }

  /// Recording always starts from a clean transcript: the previous session's
  /// lines belong to the recording on disk, not to this one.
  function startLive() {
    for (const lane of LANES) lanes[lane] = emptyLane();
    opened = null;
    follow = true;
    view = 'session';
  }

  async function openRecording(recording: Recording) {
    try {
      opened = await invoke<Session>('read_recording', { dir: recording.dir });
      editingTitle = false;
      follow = false;
      view = 'session';
    } catch (error) {
      showToast(`この録音を開けませんでした: ${error}`);
    }
  }

  function goHome() {
    view = 'home';
    editingTitle = false;
    langOpen = false;
    actionsOpen = false;
    void refresh();
  }

  function beginTitleEdit() {
    if (!opened) return;
    titleDraft = sessionTitle;
    editingTitle = true;
    actionsOpen = false;
  }

  function cancelTitleEdit() {
    editingTitle = false;
    titleDraft = '';
  }

  function focusTitleInput(input: HTMLInputElement) {
    input.focus();
    input.select();
  }

  async function saveTitle() {
    if (!opened || savingTitle) return;
    const title = titleDraft.trim();
    if (!title) {
      showToast('タイトルを入力してください');
      return;
    }

    savingTitle = true;
    try {
      const saved = await invoke<string>('set_recording_title', { dir: opened.dir, title });
      opened.title = saved;
      const listed = recordings.find((recording) => recording.dir === opened?.dir);
      if (listed) listed.title = saved;
      cancelTitleEdit();
      showToast('タイトルを変更しました');
    } catch (error) {
      showToast(`タイトルを変更できませんでした: ${error}`);
    } finally {
      savingTitle = false;
    }
  }

  async function applyLanguages(next: Languages) {
    const previous = languages;
    languages = next; // optimistic: the control must not lag the click
    langOpen = false;
    try {
      await invoke('set_languages', { languages: next });
      showToast(`次の発話から「${pillText(next)}」で認識します`);
    } catch (error) {
      languages = previous;
      showToast(`言語を変更できませんでした: ${error}`);
    }
  }

  /// `[mm:ss] 話者: 原文` with the translation hung under it, which is how the
  /// transcript reads on screen and how it should paste.
  function transcriptText(scope: 'full' | 'original' | 'translation'): string {
    return stream
      .map((line) => {
        const body = scope === 'translation' ? (line.translation ?? line.text) : line.text;
        const head = `[${clock(line.startMs)}] ${LANE_LABELS[line.lane]}: ${body}`;
        if (scope !== 'full' || !line.translation) return head;
        return `${head}\n    ${line.translation}`;
      })
      .join('\n');
  }

  const COPY_LABELS = { full: '文字起こし全文', original: '原文', translation: '翻訳' } as const;

  async function copy(scope: 'full' | 'original' | 'translation') {
    actionsOpen = false;
    try {
      await navigator.clipboard.writeText(transcriptText(scope));
      showToast(`${COPY_LABELS[scope]}をコピーしました`);
    } catch {
      showToast('コピーできませんでした');
    }
  }

  // ── The library list ─────────────────────────────────────────────────
  /// Recordings under the heading they belong to. The list is already newest
  /// first, so a group ends exactly where its heading changes.
  const sections = $derived.by(() => {
    const term = search.trim().toLocaleLowerCase();
    const groups: { label: string; items: Recording[] }[] = [];
    for (const recording of recordings) {
      const haystack =
        `${recording.title ?? ''} ${recording.snippet} ${recording.name}`.toLocaleLowerCase();
      if (term && !haystack.includes(term)) continue;
      const label = dayGroup(recording.startedAtMs);
      const last = groups.at(-1);
      if (last && last.label === label) last.items.push(recording);
      else groups.push({ label, items: [recording] });
    }
    return groups;
  });

  const matchCount = $derived(sections.reduce((n, s) => n + s.items.length, 0));

  /// A recording with nothing recognised still has to be tellable apart from
  /// the next one, so it falls back to when it was made.
  const rowTitle = (recording: Recording) =>
    recording.title ||
    recording.snippet ||
    (recording.startedAtMs === null ? recording.name : `${timeOfDay(recording.startedAtMs)} の録音`);

  const laneSummary = (list: Lane[]) => list.map((lane) => LANE_LABELS[lane]).join(' + ');

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      actionsOpen = false;
      if (!langOpen) return;
      // The popover handles its own Escape (it backs out of the catalogue
      // first); this only catches the case where it is not mounted.
    }
  }

  /// Clicking anywhere else closes whatever is open — the menus are transient
  /// and should not need a second visit to the same button.
  function onpointerdown(event: PointerEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest('.lang-anchor')) langOpen = false;
    if (!target.closest('.session-actions')) actionsOpen = false;
  }
</script>

<svelte:window {onkeydown} {onpointerdown} />

<main class="app-window">
  <header class="window-toolbar">
    {#if view === 'session'}
      <button class="back-button" type="button" aria-label="録音一覧に戻る" onclick={goHome}>‹</button
      >
    {/if}
    <span class="window-title">{view === 'home' ? '録音' : sessionTitle}</span>
    <span class="toolbar-spacer"></span>

    {#if running}
      <div class="toolbar-group">
        <button
          class="toolbar-button record-active"
          type="button"
          aria-label="録音中のセッションを表示"
          onclick={() => {
            opened = null;
            view = 'session';
          }}
        >
          <span aria-hidden="true">●</span><span>{clock(elapsed)}</span>
        </button>
        <button class="toolbar-button stop-button" type="button" disabled={busy} onclick={toggle}>
          <span aria-hidden="true">■</span><span class="long-label">停止</span>
        </button>
      </div>
    {:else}
      <button class="toolbar-button" type="button" disabled={busy} onclick={toggle}>
        <span class="record-symbol" aria-hidden="true">●</span>
        <span class="long-label">録音を開始</span>
      </button>
    {/if}
  </header>

  {#if view === 'home'}
    <section class="home-view" aria-label="すべての録音">
      <div class="home-heading">
        <div>
          <h1>すべての録音</h1>
          <span class="count">
            {recordings.length}件{running ? ' · 1件を録音中' : ''}
          </span>
        </div>
        <label class="search-field">
          <span aria-hidden="true">⌕</span>
          <input type="search" bind:value={search} placeholder="録音を検索" aria-label="録音を検索" />
        </label>
      </div>

      {#if running}
        <div class="recording-section">
          <h2 class="section-label">録音中</h2>
          <div class="recording-list">
            <button
              class="recording-row active-session-row"
              type="button"
              onclick={() => {
                opened = null;
                view = 'session';
              }}
            >
              <span class="row-icon" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
              <span class="row-main">
                <span class="row-title">新しい録音</span>
                <span class="row-meta">録音中 · {clock(elapsed)}</span>
              </span>
              <span class="row-source">マイク + スピーカー</span>
              <span class="row-chevron" aria-hidden="true">›</span>
            </button>
          </div>
        </div>
      {/if}

      {#each sections as section (section.label)}
        <div class="recording-section">
          <h2 class="section-label">{section.label}</h2>
          <div class="recording-list">
            {#each section.items as recording (recording.dir)}
              <button
                class="recording-row"
                type="button"
                onclick={() => openRecording(recording)}
                title={recording.dir}
              >
                <span class="row-icon" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
                <span class="row-main">
                  <span class="row-title">{rowTitle(recording)}</span>
                  <span class="row-meta">
                    {recording.startedAtMs === null
                      ? recording.name
                      : timeOfDay(recording.startedAtMs)} · {durationLabel(recording.durationMs)}
                    {recording.utterances ? ` · ${recording.utterances}発話` : ' · 文字起こしなし'}
                  </span>
                </span>
                <span class="row-source">{laneSummary(recording.lanes)}</span>
                <span class="row-chevron" aria-hidden="true">›</span>
              </button>
            {/each}
          </div>
        </div>
      {/each}

      {#if !matchCount}
        <div class="empty-state">
          {recordings.length ? '該当する録音はありません' : 'まだ録音がありません'}
        </div>
      {/if}
    </section>
  {:else}
    <section class="session-view" aria-label={sessionTitle}>
      <div class="session-head">
        <div class="session-identity">
          {#if opened && editingTitle}
            <form
              class="title-editor"
              onsubmit={(event) => {
                event.preventDefault();
                void saveTitle();
              }}
            >
              <input
                type="text"
                bind:value={titleDraft}
                maxlength="100"
                aria-label="録音タイトル"
                disabled={savingTitle}
                onkeydown={(event) => {
                  if (event.key !== 'Escape') return;
                  event.preventDefault();
                  cancelTitleEdit();
                }}
                {@attach focusTitleInput}
              />
              <button
                class="title-editor-button"
                type="button"
                disabled={savingTitle}
                onclick={cancelTitleEdit}>取消</button
              >
              <button
                class="title-editor-button primary"
                type="submit"
                disabled={savingTitle || !titleDraft.trim()}>保存</button
              >
            </form>
          {:else}
            <h1>{sessionTitle}</h1>
          {/if}
          <div class="session-meta">
            {#if opened}
              <span>{durationLabel(opened.durationMs)} · {laneSummary(opened.lanes)}</span>
            {/if}
            <span class="state-badge {badge.tone}">
              <span aria-hidden="true">{badge.glyph}</span>{badge.text}
            </span>

            {#if !opened}
              <!-- Languages are a live setting: they take effect from the next
                   utterance, so they belong to a session that is still running.
                   A recording already on disk is history. -->
              <span class="lang-anchor">
                <button
                  class="lang-pill"
                  type="button"
                  aria-haspopup="dialog"
                  aria-expanded={langOpen}
                  aria-label="言語と翻訳の設定を変更"
                  onclick={() => (langOpen = !langOpen)}
                >
                  <span>{pillText(languages)}</span>
                  <span class="chev" aria-hidden="true">▾</span>
                </button>
                {#if langOpen}
                  <LanguagePopover
                    {languages}
                    onapply={applyLanguages}
                    onclose={() => (langOpen = false)}
                  />
                {/if}
              </span>

              {#if running}
                <span class="capture-badges">
                  {#each LANES as lane (lane)}
                    {@const active = !!(lanes[lane].pending || lanes[lane].volatile)}
                    <span
                      class="capture-badge"
                      class:active
                      aria-label={`${LANE_LABELS[lane]}${active ? 'を認識中' : 'は待機中'}`}
                    >
                      <span class="level-meter" aria-hidden="true"><i></i><i></i><i></i></span>
                      {LANE_LABELS[lane]}
                    </span>
                  {/each}
                </span>
              {/if}
            {/if}
          </div>
        </div>

        <div class="session-actions">
          <button
            class="toolbar-button icon-button"
            type="button"
            aria-label="この録音の操作"
            aria-haspopup="menu"
            aria-expanded={actionsOpen}
            disabled={!opened && !hasTranscript}
            onclick={() => (actionsOpen = !actionsOpen)}>•••</button
          >
          {#if actionsOpen}
            <div class="actions-menu" role="menu">
              {#if opened}
                <div class="menu-heading">録音</div>
                <button class="menu-item" type="button" role="menuitem" onclick={beginTitleEdit}>
                  タイトルを変更
                </button>
                <div class="menu-separator"></div>
              {/if}
              <div class="menu-heading">クリップボード</div>
              <button
                class="menu-item"
                type="button"
                role="menuitem"
                disabled={!hasTranscript}
                onclick={() => copy('full')}
              >
                全文をコピー
              </button>
              <button
                class="menu-item"
                type="button"
                role="menuitem"
                disabled={!hasTranscript}
                onclick={() => copy('original')}
              >
                原文のみをコピー
              </button>
              <button
                class="menu-item"
                type="button"
                role="menuitem"
                disabled={!hasTranscript}
                onclick={() => copy('translation')}
              >
                翻訳のみをコピー
              </button>
            </div>
          {/if}
        </div>
      </div>

      <div
        class="transcript"
        class:live={!opened && running}
        onscroll={onTranscriptScroll}
        aria-live={opened ? 'off' : 'polite'}
        aria-atomic="false"
        {@attach followTail}
      >
        <div class="transcript-header">
          <span class="transcript-label">
            {opened ? '文字起こし' : running ? 'リアルタイム文字起こし' : '文字起こし'}
          </span>
          <button
            class="copy-all-button"
            type="button"
            disabled={!hasTranscript}
            onclick={() => copy('full')}
          >
            <span aria-hidden="true">⧉</span>{opened ? '全文をコピー' : 'ここまでをコピー'}
          </button>
        </div>

        {#each stream as line (line.key)}
          <div class="transcript-line">
            <span class="timestamp">{clock(line.startMs)}</span>
            <span class="speaker lane-{line.lane}">{LANE_LABELS[line.lane]}</span>
            <span class="utterance">
              {line.text}
              {#if line.translation}
                <span class="translation">{line.translation}</span>
              {:else if line.translating}
                <span class="translation waiting">訳しています…</span>
              {/if}
            </span>
          </div>
        {/each}

        {#each inFlight as lane (lane)}
          <div class="transcript-line">
            <span class="timestamp pending">··:··</span>
            <span class="speaker lane-{lane}">{LANE_LABELS[lane]}</span>
            <span class="utterance partial"
              >{lanes[lane].pending}<span class="volatile">{lanes[lane].volatile}</span></span
            >
          </div>
        {/each}

        {#if !hasTranscript}
          <p class="empty-state">
            {opened
              ? 'この録音には文字起こしがありません。'
              : running
                ? '話しかけてください。認識した文がここに出ます。'
                : '「録音を開始」を押すと、ここに文字起こしが出ます。'}
          </p>
        {/if}
      </div>
    </section>
  {/if}

  {#if toast}
    <div class="toast" role="status" aria-live="polite">{toast}</div>
  {/if}
</main>

<style>
  /* The window's palette, from mockups/desktop-prototype.html. Global so the
     popover — a separate component with its own scope — resolves them too. */
  :global(:root) {
    --accent: #0068d8;
    --accent-soft: rgba(0, 104, 216, 0.12);
    --record: #d70015;
    --record-soft: rgba(215, 0, 21, 0.1);
    --speaker-a: #0068d8;
    --speaker-b: #9a4f00;
    --success: #16823b;
    --text: #1d1d1f;
    --secondary: #5b5c63;
    --muted: #686970;
    --separator: rgba(30, 30, 36, 0.12);
    --surface: #fbfbfd;
    --surface-raised: rgba(255, 255, 255, 0.92);
    --shadow-menu: 0 18px 52px rgba(0, 0, 0, 0.24);
  }

  :global(*) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    color: var(--text);
    background: var(--surface);
    font-family:
      -apple-system,
      BlinkMacSystemFont,
      'SF Pro Text',
      'Hiragino Sans',
      sans-serif;
  }
  :global(button),
  :global(input) {
    font: inherit;
    color: inherit;
  }
  :global(button) {
    border: 0;
    background: none;
    cursor: pointer;
  }
  :global(button:disabled) {
    cursor: default;
    opacity: 0.45;
  }
  :global(button:focus-visible),
  :global(input:focus-visible) {
    outline: 3px solid rgba(0, 104, 216, 0.62);
    outline-offset: 2px;
  }

  .app-window {
    position: relative;
    height: 100vh;
    overflow: hidden;
    display: grid;
    grid-template-rows: 54px minmax(0, 1fr);
  }

  /* ── Toolbar ─────────────────────────────────────────────────────── */
  .window-toolbar {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    /* Room for the traffic lights: this is the real window's title bar. */
    padding: 0 14px 0 82px;
    border-bottom: 1px solid var(--separator);
    background: rgba(252, 252, 253, 0.84);
    backdrop-filter: blur(24px) saturate(180%);
  }
  .back-button {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: var(--secondary);
    font-size: 25px;
    line-height: 1;
  }
  .back-button:hover {
    background: rgba(120, 120, 128, 0.12);
  }
  .window-title {
    min-width: 0;
    overflow: hidden;
    font-size: 13px;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toolbar-spacer {
    flex: 1;
  }
  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .toolbar-button {
    min-height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 11px;
    border: 1px solid var(--separator);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.82);
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
  }
  .toolbar-button:hover:not(:disabled) {
    background: rgba(120, 120, 128, 0.1);
  }
  .record-symbol {
    color: var(--record);
    font-size: 14px;
  }
  .record-active {
    color: var(--record);
    border-color: rgba(215, 0, 21, 0.24);
    background: var(--record-soft);
    font-variant-numeric: tabular-nums;
  }
  .stop-button {
    color: #fff;
    border-color: var(--record);
    background: var(--record);
  }
  .stop-button:hover:not(:disabled) {
    background: #b90013;
  }
  .icon-button {
    width: 30px;
    padding: 0;
    font-size: 15px;
  }

  /* ── Library ─────────────────────────────────────────────────────── */
  .home-view {
    min-height: 0;
    overflow: auto;
    padding: 30px 34px 34px;
  }
  .home-heading {
    display: flex;
    align-items: end;
    gap: 18px;
    margin-bottom: 22px;
  }
  .home-heading h1 {
    margin: 0;
    font-size: 25px;
    line-height: 1.2;
  }
  .count {
    display: block;
    margin-top: 2px;
    color: var(--muted);
    font-size: 12px;
  }
  .search-field {
    position: relative;
    width: 260px;
    margin-left: auto;
  }
  .search-field span {
    position: absolute;
    left: 10px;
    top: 7px;
    color: var(--muted);
    font-size: 14px;
    pointer-events: none;
  }
  .search-field input {
    width: 100%;
    height: 32px;
    padding: 0 10px 0 30px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: rgba(120, 120, 128, 0.12);
    font-size: 13px;
  }
  .search-field input:focus {
    border-color: rgba(0, 104, 216, 0.35);
    background: #fff;
  }
  .recording-section + .recording-section {
    margin-top: 22px;
  }
  .section-label {
    margin: 0 0 7px 8px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 650;
  }
  .recording-list {
    overflow: hidden;
    border: 1px solid var(--separator);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.72);
  }
  .recording-row {
    width: 100%;
    min-height: 62px;
    display: grid;
    grid-template-columns: 44px minmax(170px, 1fr) 140px 16px;
    align-items: center;
    gap: 14px;
    padding: 11px 16px;
    text-align: left;
  }
  .recording-row + .recording-row {
    border-top: 1px solid var(--separator);
  }
  .recording-row:hover {
    background: rgba(0, 104, 216, 0.06);
  }
  .row-icon {
    width: 38px;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border-radius: 9px;
    color: var(--accent);
    background: var(--accent-soft);
  }
  .row-icon i {
    width: 2px;
    border-radius: 2px;
    background: currentColor;
  }
  .row-icon i:nth-child(1) {
    height: 10px;
  }
  .row-icon i:nth-child(2) {
    height: 20px;
  }
  .row-icon i:nth-child(3) {
    height: 14px;
  }
  .row-icon i:nth-child(4) {
    height: 24px;
  }
  .row-icon i:nth-child(5) {
    height: 12px;
  }
  .row-main {
    min-width: 0;
  }
  .row-title {
    display: block;
    overflow: hidden;
    font-size: 14px;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-meta,
  .row-source {
    display: block;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
  }
  .row-source {
    text-align: right;
    white-space: nowrap;
  }
  .row-chevron {
    color: var(--muted);
    font-size: 18px;
  }
  .active-session-row {
    color: var(--record);
    background: var(--record-soft);
  }
  .active-session-row .row-icon {
    color: var(--record);
    background: rgba(215, 0, 21, 0.1);
  }
  .active-session-row .row-meta {
    color: #a00012;
    font-variant-numeric: tabular-nums;
  }
  .empty-state {
    padding: 48px 0;
    color: var(--muted);
    text-align: center;
    font-size: 13px;
  }

  /* ── Session ─────────────────────────────────────────────────────── */
  .session-view {
    min-height: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
  }
  .session-head {
    display: flex;
    align-items: start;
    gap: 12px;
    padding: 22px 24px 14px;
  }
  .session-identity {
    min-width: 0;
  }
  .session-head h1 {
    margin: 0 0 4px;
    font-size: 20px;
    line-height: 1.25;
  }
  .title-editor {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }
  .title-editor input {
    width: min(420px, 55vw);
    height: 30px;
    padding: 0 9px;
    border: 1px solid rgba(0, 104, 216, 0.55);
    border-radius: 6px;
    background: #fff;
    box-shadow: 0 0 0 3px rgba(0, 104, 216, 0.12);
    font-size: 16px;
    font-weight: 650;
  }
  .title-editor-button {
    min-height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .title-editor-button:hover:not(:disabled) {
    background: var(--accent-soft);
  }
  .title-editor-button.primary {
    color: #fff;
    background: var(--accent);
  }
  .title-editor-button.primary:hover:not(:disabled) {
    background: #005bbd;
  }
  .session-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    color: var(--muted);
    font-size: 12px;
  }
  .state-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 7px;
    border-radius: 6px;
    color: var(--success);
    background: rgba(22, 130, 59, 0.1);
    font-size: 11px;
    font-weight: 650;
  }
  .state-badge.live {
    color: var(--record);
    background: var(--record-soft);
  }
  .state-badge.busy {
    color: var(--secondary);
    background: rgba(120, 120, 128, 0.12);
  }
  .state-badge.error {
    color: var(--record);
    background: var(--record-soft);
  }
  .capture-badges {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  /* Dim until that lane actually has words in flight: the badge reports what
     is being heard, and a permanently lit meter would report nothing. */
  .capture-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 3px 8px;
    border: 1px solid var(--separator);
    border-radius: 6px;
    color: var(--muted);
    background: rgba(120, 120, 128, 0.08);
    font-size: 11px;
    font-weight: 650;
  }
  .capture-badge.active {
    color: #116d31;
    border-color: rgba(22, 130, 59, 0.2);
    background: rgba(22, 130, 59, 0.08);
  }
  .level-meter {
    height: 12px;
    display: inline-flex;
    align-items: end;
    gap: 2px;
    color: currentColor;
  }
  .level-meter i {
    width: 2px;
    border-radius: 2px;
    background: currentColor;
    transform-origin: bottom;
  }
  .level-meter i:nth-child(1) {
    height: 5px;
  }
  .level-meter i:nth-child(2) {
    height: 11px;
  }
  .level-meter i:nth-child(3) {
    height: 8px;
  }
  .capture-badge.active .level-meter i {
    animation: input-level 850ms ease-in-out infinite alternate;
  }
  .capture-badge.active .level-meter i:nth-child(2) {
    animation-delay: 140ms;
  }
  .capture-badge.active .level-meter i:nth-child(3) {
    animation-delay: 260ms;
  }

  .lang-anchor {
    position: relative;
    display: inline-flex;
  }
  .lang-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 3px 8px;
    border: 1px solid var(--separator);
    border-radius: 6px;
    color: var(--text);
    background: rgba(255, 255, 255, 0.82);
    font-size: 11px;
    font-weight: 650;
  }
  .lang-pill:hover {
    border-color: rgba(0, 104, 216, 0.4);
    background: var(--accent-soft);
  }
  .lang-pill[aria-expanded='true'] {
    color: #fff;
    border-color: var(--accent);
    background: var(--accent);
  }
  .lang-pill .chev {
    color: var(--muted);
    font-size: 9px;
  }
  .lang-pill[aria-expanded='true'] .chev {
    color: rgba(255, 255, 255, 0.72);
  }

  .session-actions {
    position: relative;
    margin-left: auto;
    display: flex;
    gap: 7px;
  }
  .actions-menu {
    position: absolute;
    right: 0;
    top: 36px;
    z-index: 35;
    width: 220px;
    padding: 6px;
    border: 1px solid var(--separator);
    border-radius: 10px;
    background: var(--surface-raised);
    box-shadow: var(--shadow-menu);
    backdrop-filter: blur(28px) saturate(180%);
    animation: appear 160ms ease-out;
  }
  .menu-heading {
    padding: 6px 9px 4px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
  }
  .menu-item {
    width: 100%;
    min-height: 29px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 9px;
    border-radius: 6px;
    text-align: left;
    font-size: 13px;
  }
  .menu-item:hover {
    color: #fff;
    background: var(--accent);
  }
  .menu-item:disabled {
    color: var(--muted);
    background: transparent;
  }
  .menu-separator {
    height: 1px;
    margin: 5px 8px;
    background: var(--separator);
  }

  /* ── Transcript ──────────────────────────────────────────────────── */
  .transcript {
    min-height: 0;
    overflow: auto;
    padding: 4px 24px 30px;
  }
  .transcript-header {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0 12px;
    background: linear-gradient(var(--surface) 70%, transparent);
  }
  .transcript-label {
    color: var(--muted);
    font-size: 11px;
    font-weight: 650;
  }
  .copy-all-button {
    min-height: 26px;
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 9px;
    border: 1px solid var(--separator);
    border-radius: 7px;
    color: var(--accent);
    background: rgba(255, 255, 255, 0.82);
    font-size: 12px;
    font-weight: 600;
  }
  .copy-all-button:hover:not(:disabled) {
    background: var(--accent-soft);
  }
  .transcript-line {
    display: grid;
    grid-template-columns: 48px 74px minmax(0, 1fr);
    gap: 10px;
    align-items: baseline;
    padding: 7px 0;
    border-radius: 6px;
  }
  .transcript-line:hover {
    background: rgba(120, 120, 128, 0.07);
  }
  .timestamp {
    color: var(--muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    user-select: none;
  }
  /* The line still being spoken has no settled position yet. */
  .timestamp.pending {
    color: rgba(104, 105, 112, 0.4);
  }
  .speaker {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    width: max-content;
    padding: 2px 7px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 650;
  }
  .speaker::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .speaker.lane-mic {
    color: var(--speaker-a);
    background: var(--accent-soft);
  }
  .speaker.lane-speaker {
    color: var(--speaker-b);
    background: rgba(154, 79, 0, 0.1);
  }
  .utterance {
    font-size: 14px;
    line-height: 1.55;
    white-space: pre-wrap;
  }
  /* While recording, the transcript is read across the room rather than up
     close, so the live text takes a size up. */
  .transcript.live .utterance {
    font-size: 17px;
    font-weight: 600;
    line-height: 1.5;
  }
  /* The translation reads as a second voice under the sentence, not as part
     of it: its own line, dimmer. */
  .translation {
    display: block;
    margin-top: 2px;
    color: var(--muted);
    font-size: 12.5px;
    line-height: 1.6;
    font-weight: 400;
  }
  .translation.waiting {
    color: rgba(104, 105, 112, 0.55);
    font-style: italic;
  }
  .volatile {
    color: var(--muted);
  }
  .partial::after {
    content: '';
    display: inline-block;
    width: 2px;
    height: 1em;
    margin-left: 3px;
    vertical-align: -0.15em;
    background: var(--accent);
    animation: blink 1s steps(1) infinite;
  }

  .toast {
    position: absolute;
    right: 24px;
    bottom: 24px;
    z-index: 80;
    max-width: 320px;
    padding: 12px 14px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-radius: 10px;
    color: #fff;
    background: rgba(24, 24, 30, 0.84);
    backdrop-filter: blur(24px) saturate(170%);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.38);
    font-size: 12.5px;
    line-height: 1.5;
    animation: appear 160ms ease-out;
  }

  @keyframes appear {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  @keyframes input-level {
    from {
      transform: scaleY(0.48);
    }
    to {
      transform: scaleY(1);
    }
  }

  @media (max-width: 820px) {
    .home-view {
      padding: 22px 20px;
    }
    .home-heading {
      align-items: stretch;
      flex-wrap: wrap;
    }
    .search-field {
      width: 100%;
      margin-left: 0;
    }
    .recording-row {
      grid-template-columns: 40px minmax(150px, 1fr) 16px;
    }
    .row-source {
      display: none;
    }
    .session-head {
      padding: 18px 18px 12px;
    }
    .title-editor {
      flex-wrap: wrap;
    }
    .title-editor input {
      width: 100%;
    }
    .transcript {
      padding-inline: 18px;
    }
    .long-label {
      display: none;
    }
  }

  @media (prefers-contrast: more) {
    :global(:root) {
      --separator: rgba(0, 0, 0, 0.32);
      --muted: #494a50;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(*),
    :global(*::before),
    :global(*::after) {
      animation: none !important;
      transition: none !important;
    }
  }
</style>
