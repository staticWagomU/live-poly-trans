/// Stages emitted by the Rust `ensure_uv` command over `uv-install-progress`.
export const UV_INSTALL_STAGES = ['download', 'verify', 'extract', 'done'] as const;

export type UvInstallStage = (typeof UV_INSTALL_STAGES)[number];

export function isUvInstallStage(value: unknown): value is UvInstallStage {
  return UV_INSTALL_STAGES.includes(value as UvInstallStage);
}

const labels: Record<UvInstallStage, string> = {
  download: 'ダウンロード中…',
  verify: 'ファイルを検証中…',
  extract: '展開中…',
  done: '完了'
};

export function uvInstallStageLabel(stage: UvInstallStage): string {
  return labels[stage];
}

/// The download dominates the wall clock, so the bar is weighted rather than
/// evenly split: an even split would sit at 25% for the whole wait and then
/// sprint, which reads as a stall.
export function uvInstallStageProgress(stage: UvInstallStage): number {
  switch (stage) {
    case 'download':
      return 0.1;
    case 'verify':
      return 0.75;
    case 'extract':
      return 0.9;
    case 'done':
      return 1;
  }
}
