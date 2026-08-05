<script lang="ts">
  import { onMount } from 'svelte';
  import {
    decreaseMimiScale,
    DEFAULT_MIMI_SCALE,
    increaseMimiScale,
    MAX_MIMI_SCALE,
    MIN_MIMI_SCALE
  } from '$lib/mimiDisplay';
  import { getMimiInvert, getMimiScale, setMimiInvert, setMimiScale } from '$lib/settingsStore';

  /// Latest utterance texts, oldest first; the last one is the live line.
  export let lines: string[];
  export let pttHeld: boolean;
  export let onPttChange: (held: boolean) => void;
  export let onExit: () => void;

  let scale = DEFAULT_MIMI_SCALE;
  let invert = false;

  onMount(() => {
    scale = getMimiScale();
    invert = getMimiInvert();

    return () => {
      onPttChange(false);
    };
  });

  function setScale(next: number) {
    scale = next;
    setMimiScale(next);
  }

  function toggleInvert() {
    invert = !invert;
    setMimiInvert(invert);
  }

  // Space works like the on-screen button: recognition only while held.
  // key repeat must not re-trigger, and releasing anywhere must stop.
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onExit();
      return;
    }

    if (event.code === 'Space' && !event.repeat) {
      event.preventDefault();
      onPttChange(true);
    }
  }

  function handleKeyup(event: KeyboardEvent) {
    if (event.code === 'Space') {
      event.preventDefault();
      onPttChange(false);
    }
  }

  $: visibleLines = lines.slice(-3);
</script>

<svelte:window on:keydown={handleKeydown} on:keyup={handleKeyup} />

<section class="mimi" class:invert aria-label="対面モード">
  <header class="mimi-top">
    <span class="mimi-badge">👂 対面モード · マイクのみ</span>
    <div class="spacer"></div>
    <button
      type="button"
      class="mimi-ctl"
      title="文字を小さく"
      disabled={scale <= MIN_MIMI_SCALE}
      on:click={() => setScale(decreaseMimiScale(scale))}
    >
      A−
    </button>
    <button
      type="button"
      class="mimi-ctl"
      title="文字を大きく"
      disabled={scale >= MAX_MIMI_SCALE}
      on:click={() => setScale(increaseMimiScale(scale))}
    >
      A＋
    </button>
    <button type="button" class="mimi-ctl" title="白黒反転" on:click={toggleInvert}>
      ◐ 反転
    </button>
    <button type="button" class="mimi-ctl exit" on:click={onExit}>終了</button>
  </header>

  <div class="mimi-body" style="--mfs: {scale}">
    {#if visibleLines.length === 0}
      <p class="mimi-hint">下のボタンを押しながらマイクに向かって話すと、ここに大きな文字で表示されます</p>
    {:else}
      {#each visibleLines as line, index (index)}
        <div class="mimi-line">
          <p>{line}</p>
        </div>
      {/each}
    {/if}
  </div>

  <footer class="mimi-foot">
    <button
      type="button"
      class="mimi-ptt"
      class:hold={pttHeld}
      on:pointerdown={(event) => {
        event.preventDefault();
        onPttChange(true);
      }}
      on:pointerup={() => onPttChange(false)}
      on:pointercancel={() => onPttChange(false)}
      on:pointerleave={() => pttHeld && onPttChange(false)}
    >
      <span class="ptt-dot"></span>
      <span>{pttHeld ? '聞き取り中…' : '押しながら話す(スペースキーでも可)'}</span>
    </button>
  </footer>
</section>

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

  .mimi {
    position: absolute;
    inset: 0;
    z-index: 45;
    display: grid;
    grid-template-rows: auto 1fr auto;
    background: #ffffff;
    color: #1d1d1f;
    animation: fadeUp 0.25s ease;
  }

  .mimi.invert {
    background: #000;
    color: #fff;
  }

  @keyframes fadeUp {
    from {
      opacity: 0;
      transform: translateY(12px);
    }

    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mimi {
      animation: none;
    }
  }

  .spacer {
    flex: 1;
  }

  .mimi-top {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.09);
  }

  .mimi.invert .mimi-top,
  .mimi.invert .mimi-foot {
    border-color: rgba(255, 255, 255, 0.18);
  }

  .mimi-badge {
    font-size: 13px;
    font-weight: 600;
    color: #515154;
  }

  .mimi.invert .mimi-badge {
    color: rgba(255, 255, 255, 0.75);
  }

  .mimi-ctl {
    min-width: 52px;
    min-height: 44px;
    padding: 8px 14px;
    border-radius: 11px;
    border: 1px solid rgba(0, 0, 0, 0.2);
    background: transparent;
    font-size: 15px;
    font-weight: 600;
  }

  .mimi.invert .mimi-ctl {
    border-color: rgba(255, 255, 255, 0.35);
    color: #fff;
  }

  .mimi-ctl:active:not(:disabled) {
    transform: scale(0.95);
  }

  .mimi-ctl:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .mimi-ctl.exit {
    color: var(--red);
  }

  .mimi.invert .mimi-ctl.exit {
    color: #ff6961;
  }

  .mimi-body {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: calc(18px * var(--mfs));
    padding: 24px 40px 28px;
  }

  .mimi-hint {
    margin: auto;
    font-size: 20px;
    color: #515154;
    text-align: center;
    line-height: 1.6;
  }

  .mimi.invert .mimi-hint {
    color: rgba(255, 255, 255, 0.75);
  }

  .mimi-line {
    animation: fadeUp 0.3s ease;
  }

  @media (prefers-reduced-motion: reduce) {
    .mimi-line {
      animation: none;
    }
  }

  .mimi-line p {
    margin: 0;
    font-size: calc(26px * var(--mfs));
    line-height: 1.4;
    font-weight: 600;
    letter-spacing: 0;
    opacity: 0.5;
    transition: font-size 0.25s ease, opacity 0.25s ease;
    word-break: break-word;
  }

  .mimi-line:last-child p {
    font-size: calc(48px * var(--mfs));
    opacity: 1;
  }

  .mimi-foot {
    padding: 12px 16px 16px;
    border-top: 1px solid rgba(0, 0, 0, 0.09);
  }

  .mimi-ptt {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100%;
    min-height: 64px;
    border-radius: 16px;
    border: 1px solid rgba(0, 0, 0, 0.2);
    background: #f5f5f7;
    color: #1d1d1f;
    font-size: 17px;
    font-weight: 600;
    user-select: none;
    -webkit-user-select: none;
    touch-action: none;
    transition: all 0.15s ease;
  }

  .mimi-ptt .ptt-dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #86868b;
    opacity: 0.5;
    transition: all 0.15s ease;
  }

  .mimi-ptt.hold {
    background: var(--green);
    border-color: var(--green);
    color: #fff;
    transform: scale(0.99);
  }

  .mimi-ptt.hold .ptt-dot {
    background: #fff;
    opacity: 1;
    animation: pulse 1.2s ease-out infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .mimi-ptt.hold .ptt-dot {
      animation: none;
    }
  }

  @keyframes pulse {
    0% {
      box-shadow: 0 0 0 0 rgba(52, 199, 89, 0.35);
    }

    100% {
      box-shadow: 0 0 0 20px rgba(52, 199, 89, 0);
    }
  }

  .mimi.invert .mimi-ptt {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
  }

  .mimi.invert .mimi-ptt.hold {
    background: var(--green);
    border-color: var(--green);
  }
</style>
