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
export const AUDIO_SILENCE_THRESHOLD_PEAK = 0.01;

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
  options: { nowMs?: number; windowMs?: number; thresholdPeak?: number } = {}
): SilenceState {
  const nowMs = options.nowMs ?? Date.now();
  const windowMs = options.windowMs ?? AUDIO_SILENCE_WINDOW_MS;
  const thresholdPeak = options.thresholdPeak ?? AUDIO_SILENCE_THRESHOLD_PEAK;
  const recent = history[stream].filter((level) => nowMs - level.receivedAtMs <= windowMs);

  if (recent.length === 0) {
    return 'unknown';
  }

  return recent.some((level) => level.peak >= thresholdPeak) ? 'active' : 'silent';
}

function clampLevel(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(1, Math.max(0, value));
}
