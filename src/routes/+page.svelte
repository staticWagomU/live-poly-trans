<main class="shell">
  <section class="window" aria-label="LivePolyTrans">
    <div class="traffic-lights" aria-hidden="true">
      <span class="close"></span>
      <span class="minimize"></span>
      <span class="zoom"></span>
    </div>
    <div class="empty-state">
      <div class="bubble-icon">􀌤</div>
      <p>Listening for speaker audio.</p>
    </div>
    <div class="status"><span></span>Listening</div>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    min-height: 100vh;
    background: radial-gradient(circle at 50% 40%, #686866 0, #4f504f 52%, #3d3e3d 100%);
    color: #1f1f1f;
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  }

  .shell {
    display: grid;
    min-height: 100vh;
    place-items: center;
    padding: 32px;
  }

  .window {
    position: relative;
    width: min(1120px, 92vw);
    height: min(720px, 82vh);
    border: 1px solid rgba(255, 255, 255, 0.38);
    border-radius: 34px;
    background:
      linear-gradient(135deg, rgba(255, 255, 255, 0.84), rgba(228, 228, 226, 0.88)),
      #ececeb;
    box-shadow:
      0 34px 80px rgba(0, 0, 0, 0.38),
      inset 0 1px 0 rgba(255, 255, 255, 0.82);
  }

  .traffic-lights {
    position: absolute;
    top: 29px;
    left: 29px;
    display: flex;
    gap: 12px;
  }

  .traffic-lights span {
    width: 18px;
    height: 18px;
    border-radius: 999px;
  }

  .close {
    background: #ff5f57;
    border: 1px solid #e0443e;
  }

  .minimize {
    background: #febc2e;
    border: 1px solid #d89a15;
  }

  .zoom {
    background: #28c840;
    border: 1px solid #1faa34;
  }

  .empty-state {
    display: grid;
    height: 100%;
    place-content: center;
    color: #777;
    text-align: center;
  }

  .bubble-icon {
    font-size: 32px;
    line-height: 1;
  }

  .empty-state p,
  .status {
    font-size: 18px;
    font-weight: 500;
  }

  .status {
    position: absolute;
    bottom: 24px;
    left: 30px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: #707070;
  }

  .status span {
    width: 10px;
    height: 10px;
    border-radius: 999px;
    background: #35c759;
  }
</style>
