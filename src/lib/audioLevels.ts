import type { AudioStream } from './audioMode';
import type { AudioLevelEvent } from './transcripts';

export type StreamAudioLevel = {
  stream: AudioStream;
  sampleCount: number;
  rms: number;
  peak: number;
  timestamp: string;
  receivedAtMs: number;
};

export type AudioLevelHistory = Record<AudioStream, StreamAudioLevel[]>;
export type SilenceState = 'unknown' | 'active' | 'silent';

export const AUDIO_LEVEL_HISTORY_LIMIT = 32;
export const AUDIO_SILENCE_WINDOW_MS = 3_000;
export const AUDIO_SILENCE_THRESHOLD_DB = -40;

export function emptyAudioLevelHistory(): AudioLevelHistory {
  return {
    mic: [],
    speaker: []
  };
}

export function appendAudioLevel(
  history: AudioLevelHistory,
  event: AudioLevelEvent,
  options: { receivedAtMs?: number; limit?: number } = {}
): AudioLevelHistory {
  const receivedAtMs = options.receivedAtMs ?? Date.now();
  const limit = Math.max(1, options.limit ?? AUDIO_LEVEL_HISTORY_LIMIT);
  const nextLevel: StreamAudioLevel = {
    stream: event.stream,
    sampleCount: event.sampleCount,
    rms: event.rms,
    peak: event.peak,
    timestamp: event.timestamp,
    receivedAtMs
  };
  const streamLevels = [...history[event.stream], nextLevel].slice(-limit);

  return {
    ...history,
    [event.stream]: streamLevels
  };
}

export function audioLevelMeter(
  level: Pick<StreamAudioLevel, 'stream' | 'peak'>
): { stream: AudioStream; value: number } {
  return {
    stream: level.stream,
    value: clampLevel(level.peak)
  };
}

export function latestAudioLevel(
  history: AudioLevelHistory,
  stream: AudioStream
): StreamAudioLevel | undefined {
  return history[stream][history[stream].length - 1];
}

export function streamSilenceState(
  history: AudioLevelHistory,
  stream: AudioStream,
  options: { nowMs?: number; windowMs?: number; thresholdDb?: number } = {}
): SilenceState {
  const nowMs = options.nowMs ?? Date.now();
  const windowMs = options.windowMs ?? AUDIO_SILENCE_WINDOW_MS;
  const thresholdDb = options.thresholdDb ?? AUDIO_SILENCE_THRESHOLD_DB;
  const recent = history[stream].filter((level) => nowMs - level.receivedAtMs <= windowMs);

  if (recent.length === 0) {
    return 'unknown';
  }

  return silenceFor(recent, thresholdDb, windowMs, nowMs) >= windowMs ? 'silent' : 'active';
}

export function silenceFor(
  levels: readonly Pick<StreamAudioLevel, 'rms' | 'receivedAtMs'>[],
  thresholdDb: number,
  windowMs: number,
  nowMs = Date.now()
): number {
  const recent = levels.filter((level) => nowMs - level.receivedAtMs <= windowMs);
  if (recent.length === 0) {
    return 0;
  }

  const lastActiveIndex = recent.findLastIndex((level) => rmsDb(level.rms) >= thresholdDb);
  const silenceStartedAt =
    lastActiveIndex >= 0
      ? (recent[lastActiveIndex + 1]?.receivedAtMs ?? nowMs)
      : (recent[0]?.receivedAtMs ?? nowMs);

  return Math.max(0, nowMs - silenceStartedAt);
}

export function rmsDb(rms: number): number {
  if (!Number.isFinite(rms) || rms <= 0) {
    return -Infinity;
  }

  return 20 * Math.log10(rms);
}

function clampLevel(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(1, Math.max(0, value));
}
