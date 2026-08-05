export type CaptionFontFamily = 'system' | 'rounded' | 'serif';
export type CaptionLineHeight = 'compact' | 'normal' | 'relaxed';

export const DEFAULT_CAPTION_FONT_FAMILY: CaptionFontFamily = 'system';
export const DEFAULT_CAPTION_LINE_HEIGHT: CaptionLineHeight = 'normal';

const CAPTION_FONT_FAMILIES = ['system', 'rounded', 'serif'] as const;
const CAPTION_LINE_HEIGHTS = ['compact', 'normal', 'relaxed'] as const;

export function parseCaptionFontFamily(raw: string | null): CaptionFontFamily {
  return CAPTION_FONT_FAMILIES.find((candidate) => candidate === raw) ?? DEFAULT_CAPTION_FONT_FAMILY;
}

export function parseCaptionLineHeight(raw: string | null): CaptionLineHeight {
  return CAPTION_LINE_HEIGHTS.find((candidate) => candidate === raw) ?? DEFAULT_CAPTION_LINE_HEIGHT;
}

export function captionFontFamilyValue(preference: CaptionFontFamily): string {
  if (preference === 'rounded') {
    return "'SF Pro Rounded', 'Hiragino Maru Gothic ProN', 'Hiragino Sans', -apple-system, BlinkMacSystemFont, sans-serif";
  }

  if (preference === 'serif') {
    return "'Hiragino Mincho ProN', 'Yu Mincho', 'Times New Roman', serif";
  }

  return "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif";
}

export function captionLineHeightValue(preference: CaptionLineHeight): number {
  if (preference === 'compact') {
    return 1.25;
  }

  if (preference === 'relaxed') {
    return 1.7;
  }

  return 1.45;
}
