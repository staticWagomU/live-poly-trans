<script lang="ts">
  import {
    LANGUAGES,
    MAX_SPOKEN,
    canBeMutual,
    labelOf,
    matches,
    type Languages
  } from '$lib/languages';

  /// Edited as a draft and committed on 適用. The recogniser picks the change
  /// up at the next utterance either way, but a half-built setting — the
  /// second language chosen, its translation not yet — should not reach it.
  let {
    languages,
    onapply,
    onclose
  }: {
    languages: Languages;
    onapply: (next: Languages) => void;
    onclose: () => void;
  } = $props();

  // Capturing the value at open time is the point: the popover is mounted
  // when it opens, and the draft must not follow the live setting underneath.
  // svelte-ignore state_referenced_locally
  let draft = $state<Languages>($state.snapshot(languages));

  /// The second page: the catalogue, reached from whichever slot asked for it.
  type Extra = { label: string; sub: string; value: string };
  type Picker = {
    title: string;
    current: string | null;
    /// Languages already spoken elsewhere in this setting: shown, but not
    /// choosable, so the reason they are unavailable stays visible.
    blocked: string[];
    extras: Extra[];
    /// The translation slot has no per-language answer beyond its extras.
    onlyExtras: boolean;
    pick: (value: string) => void;
  };
  let picker = $state<Picker | null>(null);
  let term = $state('');

  function open(next: Omit<Picker, 'extras' | 'blocked' | 'onlyExtras'> & Partial<Picker>) {
    term = '';
    picker = { extras: [], blocked: [], onlyExtras: false, ...next };
  }

  function choose(value: string) {
    picker?.pick(value);
    picker = null;
  }

  const full = $derived(draft.spoken.length >= MAX_SPOKEN);
  const shown = $derived(LANGUAGES.filter((l) => matches(l, term)));

  const targetLabel = $derived(
    draft.mutual && canBeMutual(draft)
      ? 'おたがいの言語へ'
      : draft.target
        ? `${labelOf(draft.target)} へ`
        : '訳さない'
  );

  function editSpoken(index: number) {
    open({
      title: '話される言語',
      current: draft.spoken[index],
      blocked: draft.spoken.filter((_, i) => i !== index),
      pick: (code) => {
        const spoken = draft.spoken.map((c, i) => (i === index ? code : c));
        draft = { ...draft, spoken };
      }
    });
  }

  function addSpoken() {
    open({
      title: '追加する言語',
      current: null,
      blocked: draft.spoken,
      pick: (code) => {
        // A second language is added because both will be spoken, so a
        // translation is almost certainly wanted; offering it on by default
        // is one less step, and it is one click to turn off.
        const spoken = [...draft.spoken, code];
        draft = { ...draft, spoken, mutual: draft.target ? true : draft.mutual };
      }
    });
  }

  /// Removing the second language leaves nothing to translate back into, so
  /// mutual goes with it.
  function removeSpoken(index: number) {
    if (draft.spoken.length === 1) return;
    draft = { ...draft, spoken: draft.spoken.filter((_, i) => i !== index), mutual: false };
  }

  function editTarget() {
    open({
      title: '訳して表示',
      current: draft.mutual ? 'mutual' : (draft.target ?? 'none'),
      extras: [
        { label: '訳さない', sub: '文字起こしのみ', value: 'none' },
        ...(draft.spoken.length === MAX_SPOKEN
          ? [
              {
                label: 'おたがいの言語へ',
                sub: '両方向に訳す',
                value: 'mutual'
              }
            ]
          : [])
      ],
      pick: (value) => {
        if (value === 'none') draft = { ...draft, target: null, mutual: false };
        else if (value === 'mutual') draft = { ...draft, target: draft.spoken[0], mutual: true };
        else draft = { ...draft, target: value, mutual: false };
      }
    });
  }

  function apply() {
    // Guard the invariant the backend also enforces: mutual without a second
    // spoken language has nothing to mean.
    onapply({ ...draft, mutual: draft.mutual && canBeMutual(draft) });
  }

  /// Escape backs out one level at a time — out of the catalogue first, then
  /// out of the popover — so a mistyped search is not a reason to lose the
  /// rest of the edit.
  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.stopPropagation();
    if (picker) picker = null;
    else onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="lang-pop" role="dialog" aria-label="言語と翻訳">
  {#if picker}
    <div class="picker-head">
      <button class="picker-back" type="button" aria-label="戻る" onclick={() => (picker = null)}
        >‹</button
      >
      <span class="picker-title">{picker.title}</span>
    </div>
    {#if !picker.onlyExtras}
      <div class="picker-search">
        <!-- svelte-ignore a11y_autofocus -- the picker exists to be typed into -->
        <input
          type="search"
          autofocus
          bind:value={term}
          placeholder="言語を検索（例: Spanish, es, スペイン）"
          aria-label="言語を検索"
        />
      </div>
    {/if}
    <div class="picker-list" role="listbox" aria-label={picker.title}>
      {#if !term}
        {#each picker.extras as extra (extra.value)}
          <button
            class="picker-item"
            type="button"
            role="option"
            aria-selected={picker.current === extra.value}
            onclick={() => choose(extra.value)}
          >
            <span class="tick">{picker.current === extra.value ? '✓' : ''}</span>
            <span class="name">{extra.label}</span>
            <span class="jp">{extra.sub}</span>
            <span class="code"></span>
          </button>
        {/each}
      {/if}
      {#each shown as lang (lang.code)}
        {@const blocked = picker.blocked.includes(lang.code)}
        <button
          class="picker-item"
          type="button"
          role="option"
          disabled={blocked}
          aria-selected={picker.current === lang.code}
          onclick={() => choose(lang.code)}
        >
          <span class="tick">{picker.current === lang.code ? '✓' : ''}</span>
          <span class="name">{lang.name}</span>
          <span class="jp">{blocked ? '選択済み' : lang.jp === lang.name ? '' : lang.jp}</span>
          <span class="code">{lang.code}</span>
        </button>
      {/each}
      {#if !shown.length}
        <div class="picker-empty">該当する言語はありません</div>
      {/if}
    </div>
  {:else}
    <div class="pop-section">
      <div class="pop-title">話される言語</div>
      <p class="pop-lead">
        候補が少ないほど取り違えが減ります。1つに絞ると言語判定そのものを行いません。
      </p>
      {#each draft.spoken as code, index (index)}
        <div class="spoken-row">
          <button class="slot" type="button" onclick={() => editSpoken(index)}>
            <span class="name">{labelOf(code)}</span><span class="code">{code}</span>
          </button>
          <button
            class="slot-remove"
            type="button"
            disabled={draft.spoken.length === 1}
            aria-label={`${labelOf(code)} を候補から外す`}
            onclick={() => removeSpoken(index)}>×</button
          >
        </div>
      {/each}
      {#if full}
        <p class="cap-hint">
          会議全体で{MAX_SPOKEN}言語まで。3つ以上は1秒の窓では判定が当たらないため設けていません。
        </p>
      {:else}
        <button class="add-lang" type="button" onclick={addSpoken}>
          <span aria-hidden="true">＋</span>言語を追加
        </button>
        <p class="fast-hint">いまは1言語なので、言語判定を行いません。その分だけ速く済みます。</p>
      {/if}
    </div>

    <div class="pop-section">
      <div class="pop-title">訳して表示</div>
      <button class="slot" type="button" onclick={editTarget}>
        <span class="name">{targetLabel}</span>
        <span class="code">{draft.mutual ? '⇄' : draft.target ? '→' : ''}</span>
      </button>
    </div>

    <div class="pop-foot">
      <span class="pop-hint">次の発話から反映されます</span>
      <button class="btn" type="button" onclick={onclose}>キャンセル</button>
      <button class="btn primary" type="button" onclick={apply}>適用</button>
    </div>
  {/if}
</div>

<style>
  .lang-pop {
    position: absolute;
    left: 0;
    top: 32px;
    z-index: 40;
    width: 330px;
    overflow: hidden;
    border: 1px solid var(--separator);
    border-radius: 12px;
    background: var(--surface-raised);
    box-shadow: var(--shadow-menu);
    backdrop-filter: blur(28px) saturate(180%);
    animation: appear 150ms ease-out;
  }
  .pop-section {
    padding: 12px 14px;
  }
  .pop-section + .pop-section {
    border-top: 1px solid var(--separator);
  }
  .pop-title {
    font-size: 12.5px;
    font-weight: 700;
  }
  .pop-lead {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.55;
  }
  .pop-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px 12px;
    border-top: 1px solid var(--separator);
  }
  .pop-hint {
    min-width: 0;
    flex: 1;
    color: var(--muted);
    font-size: 10.5px;
    line-height: 1.4;
  }
  .btn {
    min-height: 28px;
    padding: 0 12px;
    border: 1px solid var(--separator);
    border-radius: 7px;
    background: #fff;
    font-size: 12.5px;
    font-weight: 600;
  }
  .btn:hover {
    background: rgba(120, 120, 128, 0.1);
  }
  .btn.primary {
    color: #fff;
    border-color: var(--accent);
    background: var(--accent);
  }
  .btn.primary:hover {
    background: #005dbf;
  }

  .spoken-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .slot {
    width: 100%;
    min-height: 36px;
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-top: 9px;
    padding: 8px 10px;
    border: 1px solid var(--separator);
    border-radius: 8px;
    background: #fff;
    text-align: left;
  }
  .slot:hover {
    border-color: rgba(0, 104, 216, 0.45);
    background: var(--accent-soft);
  }
  .slot .name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-size: 13px;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .slot-remove {
    width: 24px;
    height: 24px;
    flex: 0 0 24px;
    margin-top: 9px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--muted);
    font-size: 14px;
    line-height: 1;
  }
  .slot-remove:hover {
    color: #fff;
    background: var(--record);
  }
  .slot-remove:disabled {
    opacity: 0;
    cursor: default;
  }
  .code {
    flex: 0 0 auto;
    color: var(--muted);
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
  }

  .add-lang {
    min-height: 30px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 8px;
    padding: 0 10px;
    border: 1px dashed rgba(0, 104, 216, 0.4);
    border-radius: 8px;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .add-lang:hover {
    background: var(--accent-soft);
  }
  .cap-hint {
    margin: 8px 0 0;
    color: var(--muted);
    font-size: 10.5px;
    line-height: 1.5;
  }
  /* A single candidate lets the engine skip detection outright; say so as the
     benefit it is, rather than leaving it to be inferred. */
  .fast-hint {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin: 9px 0 0;
    color: #116d31;
    font-size: 10.5px;
    line-height: 1.5;
  }
  .fast-hint::before {
    content: '▸';
    font-size: 8px;
  }

  .picker-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 8px;
  }
  .picker-back {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--secondary);
    font-size: 20px;
  }
  .picker-back:hover {
    background: rgba(120, 120, 128, 0.12);
  }
  .picker-title {
    font-size: 12.5px;
    font-weight: 700;
  }
  .picker-search {
    padding: 0 12px 8px;
  }
  .picker-search input {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--separator);
    border-radius: 8px;
    background: #fff;
    font-size: 12.5px;
  }
  .picker-list {
    max-height: 244px;
    overflow: auto;
    padding: 0 8px 8px;
  }
  .picker-item {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 7px;
    text-align: left;
  }
  .picker-item:hover:not(:disabled) {
    color: #fff;
    background: var(--accent);
  }
  .picker-item .name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-size: 12.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Each language leads with how it writes itself; the Japanese name is a
     step down, there to be scanned. */
  .picker-item .jp {
    flex: 0 0 auto;
    color: var(--muted);
    font-size: 10.5px;
  }
  .picker-item .code {
    width: 26px;
    text-align: right;
  }
  .picker-item .tick {
    width: 12px;
    flex: 0 0 12px;
    color: var(--accent);
    font-size: 11px;
  }
  .picker-item:hover:not(:disabled) .jp,
  .picker-item:hover:not(:disabled) .code {
    color: rgba(255, 255, 255, 0.76);
  }
  .picker-item:hover:not(:disabled) .tick {
    color: #fff;
  }
  .picker-item:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .picker-empty {
    padding: 14px 10px;
    color: var(--muted);
    font-size: 12px;
    text-align: center;
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
  @media (prefers-reduced-motion: reduce) {
    .lang-pop {
      animation: none;
    }
  }
</style>
