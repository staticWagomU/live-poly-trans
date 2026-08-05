<script lang="ts">
  import '$lib/theme.css';
</script>

<svelte:head>
  <title>LivePolyTrans Overlay</title>
</svelte:head>

<main class="overlay" aria-label="字幕オーバーレイ">
  <div class="line">字幕オーバーレイ</div>
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

  .line {
    color: #fff;
    font-size: 34px;
    font-weight: 800;
    line-height: 1.2;
    text-align: center;
    text-shadow:
      0 2px 3px #000,
      0 -1px 2px #000,
      1px 0 2px #000,
      -1px 0 2px #000;
  }
</style>
