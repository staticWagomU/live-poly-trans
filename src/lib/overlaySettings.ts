export type OverlaySettings = {
  lineCount: number;
  showTranslation: boolean;
  fadeSeconds: number;
  fontScale: number;
};

export const DEFAULT_OVERLAY_SETTINGS: OverlaySettings = {
  lineCount: 2,
  showTranslation: true,
  fadeSeconds: 0,
  fontScale: 1
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function parseOverlayLineCount(raw: string | null): number {
  const parsed = Number.parseInt(raw ?? '', 10);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_OVERLAY_SETTINGS.lineCount;
  }
  return clamp(parsed, 1, 3);
}

export function parseOverlayFadeSeconds(raw: string | null): number {
  const parsed = Number.parseFloat(raw ?? '');
  if (!Number.isFinite(parsed)) {
    return DEFAULT_OVERLAY_SETTINGS.fadeSeconds;
  }
  return clamp(Math.round(parsed), 0, 10);
}

export function parseOverlayFontScale(raw: string | null): number {
  const parsed = Number.parseFloat(raw ?? '');
  if (!Number.isFinite(parsed)) {
    return DEFAULT_OVERLAY_SETTINGS.fontScale;
  }
  return Math.round(clamp(parsed, 0.7, 1.8) * 100) / 100;
}
