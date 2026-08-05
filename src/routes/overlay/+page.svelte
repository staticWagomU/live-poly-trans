<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import '$lib/theme.css';
  import {
    captionFontFamilyValue,
    captionLineHeightValue,
    type CaptionFontFamily,
    type CaptionLineHeight
  } from '$lib/captionAppearance';
  import type { OverlayCaptionLine } from '$lib/overlayCaptions';
  import type { OverlaySettings } from '$lib/overlaySettings';

  let lines: OverlayCaptionLine[] = [];
  let settings: Pick<OverlaySettings, 'fadeSeconds' | 'fontScale'> & {
    captionFontFamily: CaptionFontFamily;
    captionLineHeight: CaptionLineHeight;
  } = {
    fadeSeconds: 0,
    fontScale: 1,
    captionFontFamily: 'system',
    captionLineHeight: 'normal'
  };
  let faded = false;
  let adjusting = false;
  let adjustmentError: string | null = null;
  let fadeTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    let disposeCaptions: (() => void) | null = null;
    let disposeAdjustment: (() => void) | null = null;
    void invoke<boolean>('overlay_adjustment_enabled')
      .then((enabled) => {
        adjusting = enabled;
      })
      .catch(() => {
        adjusting = false;
      });
    void listen<{
      lines: OverlayCaptionLine[];
      settings: Pick<OverlaySettings, 'fadeSeconds' | 'fontScale'> & {
        captionFontFamily: CaptionFontFamily;
        captionLineHeight: CaptionLineHeight;
      };
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
      disposeCaptions = unlisten;
    });
    void listen<boolean>('overlay-adjustment', (event) => {
      adjusting = event.payload;
      adjustmentError = null;
    }).then((unlisten) => {
      disposeAdjustment = unlisten;
    });

    return () => {
      if (fadeTimer) {
        clearTimeout(fadeTimer);
      }
      disposeCaptions?.();
      disposeAdjustment?.();
    };
  });

  function startDrag() {
    void invoke('start_overlay_drag').catch((error) => {
      adjustmentError = String(error);
    });
  }

  function finishAdjustment() {
    void invoke('finish_overlay_adjustment').catch((error) => {
      adjustmentError = String(error);
    });
  }
</script>

<svelte:head>
  <title>LivePolyTrans Overlay</title>
</svelte:head>

<main
  class="overlay"
  aria-label="字幕オーバーレイ"
  style="--overlay-scale: {settings.fontScale}; --caption-font: {captionFontFamilyValue(
    settings.captionFontFamily
  )}; --caption-line: {captionLineHeightValue(settings.captionLineHeight)}"
>
  {#if adjusting}
    <div class="adjust">
      <button type="button" class="drag" onmousedown={startDrag}>位置をドラッグ</button>
      <button type="button" class="done" onclick={finishAdjustment}>完了</button>
      {#if adjustmentError}
        <span class="err">{adjustmentError}</span>
      {/if}
    </div>
  {/if}
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
    grid-template-rows: auto minmax(0, 1fr);
    align-items: end;
    justify-items: center;
    padding: 18px 24px;
    background: transparent;
  }

  .adjust {
    align-self: start;
    justify-self: center;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    border-radius: 10px;
    background: rgba(17, 24, 39, 0.78);
    color: #fff;
    font:
      12px/1.2 -apple-system,
      BlinkMacSystemFont,
      'SF Pro Text',
      sans-serif;
    pointer-events: auto;
  }

  .adjust button {
    border: 0;
    border-radius: 7px;
    padding: 6px 10px;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .drag {
    background: rgba(255, 255, 255, 0.18);
    cursor: grab;
  }

  .done {
    background: #0a84ff;
  }

  .err {
    max-width: 320px;
    color: #ffb4ab;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    font-family: var(--caption-font);
    font-size: calc(34px * var(--overlay-scale));
    font-weight: 800;
    line-height: var(--caption-line);
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
