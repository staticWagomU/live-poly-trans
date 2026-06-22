import type { ChatMessage } from './transcripts';

const CROSS_LANGUAGE_SEGMENT_TOLERANCE_MS = 250;
const CONFIDENCE_DECISIVE_GAP = 0.12;
const DENSITY_PENALTY_THRESHOLD = 24;
const LENGTH_RATIO_THRESHOLD = 1.8;

export function mergeTranscriptMessages(current: ChatMessage, incoming: ChatMessage): ChatMessage {
  if (current.id === incoming.id) {
    return {
      ...current,
      ...incoming,
      isFinal: current.isFinal || incoming.isFinal
    };
  }

  if (!isCompetingTranscriptCandidate(current, incoming)) {
    return incoming;
  }

  return compareTranscriptCandidates(current, incoming) >= 0 ? current : incoming;
}

export function isCompetingTranscriptCandidate(current: ChatMessage, incoming: ChatMessage) {
  if (current.role !== incoming.role || current.language === incoming.language) {
    return false;
  }

  const currentStart = segmentStart(current.segmentId);
  const incomingStart = segmentStart(incoming.segmentId);

  if (currentStart === null || incomingStart === null) {
    return false;
  }

  return Math.abs(currentStart - incomingStart) <= CROSS_LANGUAGE_SEGMENT_TOLERANCE_MS;
}

export function compareTranscriptCandidates(left: ChatMessage, right: ChatMessage) {
  const confidenceGap = transcriptConfidence(left) - transcriptConfidence(right);

  if (Math.abs(confidenceGap) >= CONFIDENCE_DECISIVE_GAP) {
    return confidenceGap;
  }

  const leftFitness = languageFitness(left.text, left.language);
  const rightFitness = languageFitness(right.text, right.language);
  const fitnessGap = leftFitness - rightFitness;

  if (Math.abs(fitnessGap) >= 0.2) {
    return fitnessGap;
  }

  const densityGap = transcriptDensityPenalty(right) - transcriptDensityPenalty(left);

  if (Math.abs(densityGap) >= 0.2) {
    return densityGap;
  }

  const leftLength = normalizedTextLength(left.text);
  const rightLength = normalizedTextLength(right.text);
  const longerLength = Math.max(leftLength, rightLength);
  const shorterLength = Math.max(1, Math.min(leftLength, rightLength));

  if (longerLength / shorterLength >= LENGTH_RATIO_THRESHOLD) {
    return leftLength < rightLength ? 1 : -1;
  }

  if (right.isFinal !== left.isFinal) {
    return left.isFinal ? 1 : -1;
  }

  return 1;
}

export function segmentStart(segmentId: string) {
  const start = Number(segmentId.split('-')[0]);
  return Number.isFinite(start) ? start : null;
}

export function transcriptConfidence(message: ChatMessage) {
  return message.confidence ?? 0;
}

export function transcriptDensityPenalty(message: ChatMessage) {
  const duration = segmentDuration(message.segmentId);

  if (duration === null || duration <= 0) {
    return 0;
  }

  const charactersPerSecond = normalizedTextLength(message.text) / (duration / 1000);

  if (charactersPerSecond <= DENSITY_PENALTY_THRESHOLD) {
    return 0;
  }

  return Math.min(1, (charactersPerSecond - DENSITY_PENALTY_THRESHOLD) / DENSITY_PENALTY_THRESHOLD);
}

export function languageFitness(text: string, language: string) {
  const primaryLanguage = language.split('-')[0]?.toLowerCase() ?? '';
  const significantCharacters = [...text].filter((character) => /\p{L}|\p{N}/u.test(character));

  if (significantCharacters.length === 0) {
    return 0;
  }

  if (primaryLanguage === 'ja') {
    const matching = significantCharacters.filter(isJapaneseCharacter).length;
    return matching / significantCharacters.length;
  }

  if (usesLatinScript(primaryLanguage)) {
    const matching = significantCharacters.filter(isLatinCharacter).length;
    return matching / significantCharacters.length;
  }

  return 0.5;
}

function segmentDuration(segmentId: string) {
  const duration = Number(segmentId.split('-')[1]);
  return Number.isFinite(duration) ? duration : null;
}

function normalizedTextLength(text: string) {
  return [...text.trim()].length;
}

function isJapaneseCharacter(character: string) {
  return /[\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Han}]/u.test(character);
}

function isLatinCharacter(character: string) {
  return /[\p{Script=Latin}]/u.test(character);
}

function usesLatinScript(language: string) {
  return ['en', 'fr', 'de', 'es', 'it', 'pt', 'nl'].includes(language);
}
