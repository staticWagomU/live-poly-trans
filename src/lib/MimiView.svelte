<script lang="ts">
  import { onMount } from 'svelte';
  import LanguagePopover from '$lib/LanguagePopover.svelte';
  import { pillText, type Languages } from '$lib/languages';

  type Line = { id: number; text: string; translation: string | null; translating: boolean };
  let {
    lines,
    languages,
    pending,
    held,
    loading,
    error,
    onHeldChange,
    onLanguagesChange,
    onExit
  }: {
    lines: Line[];
    languages: Languages;
    pending: string;
    held: boolean;
    loading: boolean;
    error: string | null;
    onHeldChange: (held: boolean) => void;
    onLanguagesChange: (next: Languages) => void;
    onExit: () => void;
  } = $props();

  let scale = $state(1);
  let inverted = $state(false);
  let langOpen = $state(false);
  const visible = $derived(lines.slice(-3));

  onMount(() => {
    const saved = Number(localStorage.getItem('lpt-mimi-scale'));
    if (Number.isFinite(saved) && saved >= 0.7 && saved <= 2.2) scale = saved;
    inverted = localStorage.getItem('lpt-mimi-invert') === '1';
    return () => onHeldChange(false);
  });

  function changeScale(step: number) {
    scale = Math.min(2.2, Math.max(0.7, Math.round((scale + step) * 100) / 100));
    localStorage.setItem('lpt-mimi-scale', String(scale));
  }

  function toggleInvert() {
    inverted = !inverted;
    localStorage.setItem('lpt-mimi-invert', inverted ? '1' : '0');
  }

  function keydown(event: KeyboardEvent) {
    if (langOpen) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      onExit();
    } else if (event.code === 'Space') {
      event.preventDefault();
      if (!event.repeat) onHeldChange(true);
    }
  }

  function keyup(event: KeyboardEvent) {
    if (event.code === 'Space') {
      event.preventDefault();
      onHeldChange(false);
    }
  }
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} onblur={() => onHeldChange(false)} />

<section class="mimi" class:inverted aria-label="対面モード" style={`--mimi-scale: ${scale}`}>
  <header>
    <strong>対面モード</strong>
    <span>マイクのみ</span>
    <div class="language">
      <button type="button" aria-haspopup="dialog" aria-expanded={langOpen} title="言語と翻訳" onclick={() => (langOpen = !langOpen)}>{pillText(languages)} ▾</button>
      {#if langOpen}
        <LanguagePopover
          {languages}
          onapply={(next) => {
            onLanguagesChange(next);
            langOpen = false;
          }}
          onclose={() => (langOpen = false)}
        />
      {/if}
    </div>
    <div class="spacer"></div>
    <button type="button" aria-label="文字を小さく" title="文字を小さく" disabled={scale <= 0.7} onclick={() => changeScale(-0.15)}>A−</button>
    <button type="button" aria-label="文字を大きく" title="文字を大きく" disabled={scale >= 2.2} onclick={() => changeScale(0.15)}>A＋</button>
    <button type="button" title="白黒反転" onclick={toggleInvert}>◐ 反転</button>
    <button type="button" class="exit" onclick={onExit}>終了</button>
  </header>

  <div class="lines" aria-live="polite" aria-atomic="false">
    {#if error}
      <p class="error" role="alert">{error}</p>
    {:else if visible.length === 0 && !pending}
      <p class="hint">ボタンを押しながら話してください</p>
    {/if}
    {#each visible as line (line.id)}
      <div class="line">
        <p>{line.text}</p>
        {#if line.translation}
          <p class="translation">{line.translation}</p>
        {:else if line.translating}
          <p class="translation waiting">訳しています…</p>
        {/if}
      </div>
    {/each}
    {#if pending}
      <div class="line current"><p>{pending}</p></div>
    {/if}
  </div>

  <footer>
    <button
      type="button"
      class="ptt"
      class:held
      onpointerdown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        onHeldChange(true);
      }}
      onpointerup={() => onHeldChange(false)}
      onpointercancel={() => onHeldChange(false)}
    >
      <span class="dot" aria-hidden="true"></span>
      {held ? (loading ? 'モデル準備中…' : '聞き取り中…') : '押しながら話す'}
      <span class="shortcut">スペースキー</span>
    </button>
  </footer>
</section>

<style>
  .mimi {
    position: absolute;
    inset: 0;
    z-index: 50;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: #fff;
    color: #1d1d1f;
  }
  .mimi.inverted { background: #080808; color: #fff; }
  header, footer { display: flex; align-items: center; gap: 8px; padding: 14px 20px; border-bottom: 1px solid currentColor; }
  header { padding-left: 82px; border-color: #ddd; }
  footer { border-top: 1px solid #ddd; border-bottom: 0; }
  .inverted header, .inverted footer { border-color: #555; }
  header strong { font-size: 14px; white-space: nowrap; }
  header span { font-size: 12px; opacity: .65; white-space: nowrap; }
  .language { position: relative; min-width: 0; }
  .language > button { max-width: 230px; overflow: hidden; text-overflow: ellipsis; }
  .language :global(.lang-pop) { color: #1d1d1f; }
  .spacer { flex: 1; }
  header button { min-width: 42px; min-height: 36px; padding: 0 8px; border: 1px solid #aaa; border-radius: 7px; font-size: 13px; white-space: nowrap; }
  .inverted header button { border-color: #777; }
  header .exit { color: #c31d2b; }
  .inverted header .exit { color: #ff747c; }
  .lines { min-height: 0; overflow: auto; display: flex; flex-direction: column; justify-content: safe flex-end; gap: 22px; padding: 28px 5%; }
  .hint { margin: auto; color: #666; font-size: 20px; text-align: center; }
  .inverted .hint { color: #aaa; }
  .line { flex-shrink: 0; overflow-wrap: anywhere; opacity: .48; }
  .line:last-child { opacity: 1; }
  .line p { margin: 0; font-size: calc(30px * var(--mimi-scale)); line-height: 1.35; font-weight: 600; }
  .line:last-child p:first-child { font-size: calc(48px * var(--mimi-scale)); }
  .line .translation { margin-top: 5px; color: #0068d8; font-size: calc(27px * var(--mimi-scale)); }
  .inverted .line .translation { color: #82bbff; }
  .line .waiting { opacity: .5; }
  .error { color: #c31d2b; }
  .ptt { display: flex; align-items: center; justify-content: center; gap: 12px; width: 100%; min-height: 64px; border: 1px solid #aaa; border-radius: 8px; background: #f4f4f5; font-size: 17px; font-weight: 600; touch-action: none; user-select: none; }
  .inverted .ptt { color: #fff; background: #222; border-color: #777; }
  .ptt.held, .inverted .ptt.held { color: #fff; background: #16823b; border-color: #16823b; }
  .dot { width: 13px; height: 13px; border-radius: 50%; background: #999; }
  .held .dot { background: #fff; }
  .shortcut { font-size: 12px; font-weight: 400; opacity: .65; }
  @media (max-width: 700px) {
    header { flex-wrap: wrap; padding-left: 78px; }
    header span { display: none; }
    .shortcut { display: none; }
    .lines { padding: 20px; }
  }
</style>
