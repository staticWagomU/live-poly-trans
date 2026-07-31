import { describe, expect, it } from 'vitest';
import { isUvInstallStage, uvInstallStageLabel, uvInstallStageProgress } from './uvInstall';

describe('isUvInstallStage', () => {
  it('accepts only the stages the Rust side emits', () => {
    expect(isUvInstallStage('download')).toBe(true);
    expect(isUvInstallStage('verify')).toBe(true);
    expect(isUvInstallStage('extract')).toBe(true);
    expect(isUvInstallStage('done')).toBe(true);
    expect(isUvInstallStage('install')).toBe(false);
    expect(isUvInstallStage(42)).toBe(false);
  });
});

describe('uvInstallStageLabel', () => {
  it('describes each stage in terms of what is happening, not what ran', () => {
    expect(uvInstallStageLabel('download')).toBe('ダウンロード中…');
    expect(uvInstallStageLabel('verify')).toBe('ファイルを検証中…');
    expect(uvInstallStageLabel('extract')).toBe('展開中…');
    expect(uvInstallStageLabel('done')).toBe('完了');
  });
});

describe('uvInstallStageProgress', () => {
  it('advances monotonically so the bar never moves backwards', () => {
    const stages = ['download', 'verify', 'extract', 'done'] as const;
    const values = stages.map(uvInstallStageProgress);

    expect(values[0]).toBeGreaterThan(0);
    expect(values.at(-1)).toBe(1);
    for (let index = 1; index < values.length; index += 1) {
      expect(values[index]).toBeGreaterThan(values[index - 1]);
    }
  });
});
