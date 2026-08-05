<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import '$lib/theme.css';
  import type { OverlayCaptionLine } from '$lib/overlayCaptions';
  import type { OverlaySettings } from '$lib/overlaySettings';

  let lines: OverlayCaptionLine[] = [];
  let settings: Pick<OverlaySettings, 'fadeSeconds' | 'fontScale'> = {
    fadeSeconds: 0,
    fontScale: 1
  };
  let faded = false;
  let fadeTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    let dispose: (() => void) | null = null;
    void listen<{
      lines: OverlayCaptionLine[];
      settings: Pick<OverlaySettings, 'fadeSeconds' | 'fontScale'>;
    }>('overlay-captions', (event) => {
      lines = event.payload.lines;
      settings = event.payload.settings;
      faded = false;
      if (fadeTimer) {
        clearTimeout(fadeTimer);
      }
      if (settings.fadeSeconds > 0) {
        fadeTimer = setTimeout(() => {
          faded = true;
        }, settings.fadeSeconds * 1000);
      }
    }).then((unlisten) => {
      dispose = unlisten;
    });

    return () => {
      if (fadeTimer) {
        clearTimeout(fadeTimer);
      }
      dispose?.();
    };
  });
</script>

<svelte:head>
  <title>LivePolyTrans Overlay</title>
</svelte:head>

<main class="overlay" aria-label="字幕オーバーレイ" style="--overlay-scale: {settings.fontScale}">
  <div class="lines" class:faded>
    {#if lines.length === 0}
      <div class="line muted">音声を待っています</div>
    {:else}
      {#each lines as line (line.id)}
        <div class="line">
          <div>{line.primary}</div>
          {#if line.secondary}
            <div class="sub">{line.secondary}</div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: transparent;
  }

  .overlay {
    min-height: 100vh;
    display: grid;
    place-items: end center;
    padding: 18px 24px;
    background: transparent;
  }

  .lines {
    display: grid;
    gap: 8px;
    width: min(100%, 920px);
    opacity: 1;
    transition: opacity 180ms ease;
  }

  .lines.faded {
    opacity: 0;
  }

  .line {
    color: #fff;
    font-size: calc(34px * var(--overlay-scale));
    font-weight: 800;
    line-height: 1.2;
    text-align: center;
    text-shadow:
      0 2px 3px #000,
      0 -1px 2px #000,
      1px 0 2px #000,
      -1px 0 2px #000;
  }

  .line.muted {
    opacity: 0.72;
  }

  .sub {
    margin-top: 3px;
    font-size: calc(22px * var(--overlay-scale));
    font-weight: 700;
  }

  @media (prefers-reduced-motion: reduce) {
    .lines {
      transition: none;
    }
  }
</style>
