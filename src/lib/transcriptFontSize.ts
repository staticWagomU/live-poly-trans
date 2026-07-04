export const TRANSCRIPT_FONT_SCALE_STEPS = [0.85, 1, 1.15, 1.3, 1.5] as const;

export const DEFAULT_TRANSCRIPT_FONT_SCALE = 1;

function nearestStepIndex(scale: number) {
  let bestIndex = 0;
  for (let index = 1; index < TRANSCRIPT_FONT_SCALE_STEPS.length; index += 1) {
    const bestDistance = Math.abs(TRANSCRIPT_FONT_SCALE_STEPS[bestIndex] - scale);
    const distance = Math.abs(TRANSCRIPT_FONT_SCALE_STEPS[index] - scale);
    if (distance < bestDistance) {
      bestIndex = index;
    }
  }
  return bestIndex;
}

export function increaseTranscriptFontScale(scale: number) {
  const index = nearestStepIndex(scale);
  return TRANSCRIPT_FONT_SCALE_STEPS[Math.min(index + 1, TRANSCRIPT_FONT_SCALE_STEPS.length - 1)];
}

export function decreaseTranscriptFontScale(scale: number) {
  const index = nearestStepIndex(scale);
  return TRANSCRIPT_FONT_SCALE_STEPS[Math.max(index - 1, 0)];
}

export function canIncreaseTranscriptFontScale(scale: number) {
  return nearestStepIndex(scale) < TRANSCRIPT_FONT_SCALE_STEPS.length - 1;
}

export function canDecreaseTranscriptFontScale(scale: number) {
  return nearestStepIndex(scale) > 0;
}

export function parseTranscriptFontScale(raw: string | null): number {
  if (raw === null) {
    return DEFAULT_TRANSCRIPT_FONT_SCALE;
  }

  const parsed = Number(raw);
  const step = TRANSCRIPT_FONT_SCALE_STEPS.find((candidate) => candidate === parsed);
  return step ?? DEFAULT_TRANSCRIPT_FONT_SCALE;
}
