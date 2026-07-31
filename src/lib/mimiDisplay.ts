/// Display scale for the face-to-face (mimi) mode: 48px base text, scaled
/// 0.7-2.2x. Tuned for reading at about one meter.
export const DEFAULT_MIMI_SCALE = 1;
export const MIN_MIMI_SCALE = 0.7;
export const MAX_MIMI_SCALE = 2.2;
const MIMI_SCALE_STEP = 0.15;

function clampMimiScale(scale: number): number {
  return Math.min(MAX_MIMI_SCALE, Math.max(MIN_MIMI_SCALE, scale));
}

export function increaseMimiScale(scale: number): number {
  return clampMimiScale(Math.round((scale + MIMI_SCALE_STEP) * 100) / 100);
}

export function decreaseMimiScale(scale: number): number {
  return clampMimiScale(Math.round((scale - MIMI_SCALE_STEP) * 100) / 100);
}

export function parseMimiScale(raw: string | null): number {
  const parsed = Number.parseFloat(raw ?? '');
  if (!Number.isFinite(parsed)) {
    return DEFAULT_MIMI_SCALE;
  }

  return clampMimiScale(parsed);
}
