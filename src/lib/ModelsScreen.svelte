<script lang="ts">
  import { byteLabel, modelCopy, type ModelFileStatus } from '$lib/models';

  let {
    models,
    errors,
    onDownload,
    onDownloadMissing,
    onClose
  }: {
    models: ModelFileStatus[];
    errors: Record<string, string>;
    onDownload: (id: string) => void;
    onDownloadMissing: () => void;
    onClose: () => void;
  } = $props();

  const missing = $derived(models.filter((model) => model.state === 'missing' && model.canDownload));
  const allReady = $derived(models.length > 0 && models.every((model) => model.state === 'ready'));

  function percent(model: ModelFileStatus): number | null {
    if (model.bytes == null || model.totalBytes == null || model.totalBytes <= 0) return null;
    return Math.max(0, Math.min(100, (model.bytes / model.totalBytes) * 100));
  }

  function progressText(model: ModelFileStatus): string {
    if (model.bytes == null) return 'ダウンロード中';
    if (model.totalBytes == null) return `${byteLabel(model.bytes)} を取得しました`;
    return `${byteLabel(model.bytes)} / ${byteLabel(model.totalBytes)}`;
  }

  function stateText(model: ModelFileStatus, error: string | undefined): string {
    if (error && model.state !== 'downloading') return '失敗';
    switch (model.state) {
      case 'ready':
        return '準備完了';
      case 'downloading':
        return 'ダウンロード中';
      default:
        return '未取得';
    }
  }

  function tone(model: ModelFileStatus, error: string | undefined): string {
    if (error && model.state !== 'downloading') return 'failed';
    return model.state;
  }
</script>

<section class="models-view" aria-labelledby="models-title">
  <div class="models-column">
    <header class="models-heading">
      <h1 id="models-title">モデル</h1>
      <p>
        {#if allReady}
          必要なモデルは揃っています。録音を始められます。
        {:else}
          文字起こしと翻訳には、次の3つのモデルが必要です。ダウンロードはボタンを押したときだけ始まります。過去の録音は、モデルがなくても一覧から開けます。
        {/if}
      </p>
    </header>

    {#if models.length === 0}
      <p class="models-empty">モデルを確認しています…</p>
    {:else}
      <ul class="model-list">
        {#each models as model (model.id)}
          {@const copy = modelCopy(model.id)}
          {@const error = errors[model.id]}
          {@const pct = model.state === 'downloading' ? percent(model) : null}
          <li class="model-row" aria-busy={model.state === 'downloading'} title={model.path ?? undefined}>
            <span class="model-icon" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
            <div class="model-main">
              <div class="model-title-line">
                <span class="model-title">{copy.title}</span>
                <span class="model-badge {tone(model, error)}">{stateText(model, error)}</span>
              </div>
              <p class="model-detail">
                {copy.detail}{copy.detail ? ' · ' : ''}{model.fileName}
                {#if model.state === 'ready' && model.bytes != null}
                  · {byteLabel(model.bytes)}
                {/if}
              </p>
              {#if model.state === 'downloading'}
                <div
                  class="model-progress"
                  role="progressbar"
                  aria-label="{copy.title}のダウンロード"
                  aria-valuemin="0"
                  aria-valuemax="100"
                  aria-valuenow={pct === null ? undefined : Math.round(pct)}
                  aria-valuetext={progressText(model)}
                >
                  {#if pct !== null}
                    <span style:width="{pct}%"></span>
                  {/if}
                </div>
                <p class="model-detail">{progressText(model)}</p>
              {/if}
              {#if error && model.state !== 'downloading'}
                <p class="model-error" role="alert">{error}</p>
              {:else if model.message}
                <p class="model-error" role="status">{model.message}</p>
              {/if}
            </div>
            {#if model.state === 'downloading'}
              <span class="model-wait" aria-hidden="true">取得中</span>
            {:else if model.canDownload && model.state === 'missing'}
              <button
                class="row-button"
                type="button"
                aria-label={error ? `${copy.title}を再試行` : `${copy.title}をダウンロード`}
                onclick={() => onDownload(model.id)}
              >
                {error ? '再試行' : 'ダウンロード'}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    <div class="models-actions">
      {#if missing.length > 0}
        <button class="secondary-button" type="button" onclick={onClose}>録音一覧へ</button>
        <button class="primary-button" type="button" onclick={onDownloadMissing}>
          不足分をダウンロード
        </button>
      {:else if allReady}
        <button class="primary-button" type="button" onclick={onClose}>録音一覧へ</button>
      {:else}
        <button class="secondary-button" type="button" onclick={onClose}>録音一覧へ</button>
      {/if}
    </div>
  </div>
</section>

<style>
  .models-view {
    min-height: 0;
    overflow: auto;
    padding: 30px 34px 40px;
  }
  .models-column {
    max-width: 680px;
  }
  .models-heading h1 {
    margin: 0;
    font-size: 25px;
    font-weight: 650;
    line-height: 1.2;
    letter-spacing: 0;
  }
  .models-heading p {
    max-width: 62ch;
    margin: 8px 0 0;
    color: var(--secondary);
    font-size: 13px;
    line-height: 1.55;
  }
  .models-empty {
    margin: 28px 0;
    color: var(--muted);
    font-size: 13px;
  }
  .model-list {
    margin: 22px 0 0;
    padding: 0;
    list-style: none;
    overflow: hidden;
    border: 1px solid var(--separator);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.72);
  }
  .model-row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
  }
  .model-row + .model-row {
    border-top: 1px solid var(--separator);
  }
  .model-icon {
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
  .model-icon i {
    width: 2px;
    border-radius: 2px;
    background: currentColor;
  }
  .model-icon i:nth-child(1) {
    height: 10px;
  }
  .model-icon i:nth-child(2) {
    height: 20px;
  }
  .model-icon i:nth-child(3) {
    height: 14px;
  }
  .model-icon i:nth-child(4) {
    height: 24px;
  }
  .model-icon i:nth-child(5) {
    height: 12px;
  }
  .model-main {
    min-width: 0;
  }
  .model-title-line {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .model-title {
    font-size: 14px;
    font-weight: 650;
  }
  .model-detail {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .model-badge {
    display: inline-flex;
    align-items: center;
    min-height: 20px;
    padding: 2px 7px;
    border-radius: 6px;
    color: var(--secondary);
    background: rgba(120, 120, 128, 0.12);
    font-size: 11px;
    font-weight: 650;
  }
  .model-badge.ready {
    color: var(--success);
    background: rgba(22, 130, 59, 0.1);
  }
  .model-badge.downloading {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .model-badge.failed {
    color: var(--record);
    background: var(--record-soft);
  }
  .model-progress {
    height: 4px;
    margin-top: 8px;
    overflow: hidden;
    border-radius: 2px;
    background: rgba(120, 120, 128, 0.16);
  }
  .model-progress > span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 200ms cubic-bezier(0, 0, 0.58, 1);
  }
  .model-error {
    margin: 6px 0 0;
    color: var(--record);
    font-size: 12px;
    line-height: 1.45;
  }
  .model-wait {
    color: var(--muted);
    font-size: 12px;
    font-weight: 650;
    white-space: nowrap;
  }
  .row-button,
  .secondary-button,
  .primary-button {
    min-height: 32px;
    padding: 0 14px;
    border: 1px solid var(--separator);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.88);
    font-size: 12.5px;
    font-weight: 650;
    white-space: nowrap;
  }
  .row-button:hover:not(:disabled),
  .secondary-button:hover:not(:disabled) {
    background: rgba(120, 120, 128, 0.1);
  }
  .row-button:active:not(:disabled),
  .secondary-button:active:not(:disabled),
  .primary-button:active:not(:disabled) {
    filter: brightness(0.96);
  }
  .primary-button {
    min-height: 34px;
    color: #fff;
    border-color: var(--accent);
    background: var(--accent);
  }
  .primary-button:hover:not(:disabled) {
    background: #005bbd;
  }
  .models-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
  }

  @media (max-width: 820px) {
    .models-view {
      padding: 22px 20px 32px;
    }
    .model-row {
      grid-template-columns: 40px minmax(0, 1fr);
    }
    .row-button,
    .model-wait {
      grid-column: 2;
      justify-self: start;
    }
  }
</style>
