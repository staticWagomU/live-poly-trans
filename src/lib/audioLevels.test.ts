import { describe, expect, it } from 'vitest';
import {
  appendAudioLevel,
  audioLevelMeter,
  emptyAudioLevelHistory,
  silenceFor,
  streamSilenceState
} from './audioLevels';
import type { AudioLevelHistory } from './audioLevels';

describe('appendAudioLevel', () => {
  it('keeps a bounded per-stream level history', () => {
    const history = emptyAudioLevelHistory();
    const first = appendAudioLevel(
      history,
      {
        type: 'audio-level',
        stream: 'mic',
        sampleCount: 960,
        rms: 0.1,
        peak: 0.2,
        timestamp: '2026-08-05T00:00:00Z',
        sessionId: 'mic-session-1'
      },
      { receivedAtMs: 1, limit: 1 }
    );
    const second = appendAudioLevel(
      first,
      {
        type: 'audio-level',
        stream: 'mic',
        sampleCount: 960,
        rms: 0.2,
        peak: 0.4,
        timestamp: '2026-08-05T00:00:00.100Z',
        sessionId: 'mic-session-1'
      },
      { receivedAtMs: 2, limit: 1 }
    );

    expect(second.mic).toHaveLength(1);
    expect(second.mic[0]?.peak).toBe(0.4);
    expect(second.speaker).toEqual([]);
  });
});

describe('streamSilenceState', () => {
  it('detects sustained low input after enough recent samples', () => {
    const history: AudioLevelHistory = {
      mic: [
        { stream: 'mic', sampleCount: 960, rms: 0.001, peak: 0.003, timestamp: 'a', receivedAtMs: 0 },
        { stream: 'mic', sampleCount: 960, rms: 0.002, peak: 0.004, timestamp: 'b', receivedAtMs: 1_000 },
        { stream: 'mic', sampleCount: 960, rms: 0.001, peak: 0.003, timestamp: 'c', receivedAtMs: 2_000 }
      ],
      speaker: []
    };

    expect(
      streamSilenceState(history, 'mic', {
        nowMs: 2_000,
        windowMs: 2_000,
        thresholdDb: -40
      })
    ).toBe('silent');
  });

  it('returns active when any recent peak crosses the threshold', () => {
    const history: AudioLevelHistory = {
      mic: [
        { stream: 'mic', sampleCount: 960, rms: 0.001, peak: 0.003, timestamp: 'a', receivedAtMs: 0 },
        { stream: 'mic', sampleCount: 960, rms: 0.03, peak: 0.08, timestamp: 'b', receivedAtMs: 1_000 }
      ],
      speaker: []
    };

    expect(
      streamSilenceState(history, 'mic', {
        nowMs: 1_000,
        windowMs: 2_000,
        thresholdDb: -40
      })
    ).toBe('active');
  });
});

describe('silenceFor', () => {
  it('returns how long recent RMS has stayed below the dB threshold', () => {
    const levels = [
      { rms: 0.2, receivedAtMs: 0 },
      { rms: 0.001, receivedAtMs: 1_000 },
      { rms: 0.001, receivedAtMs: 2_000 },
      { rms: 0.001, receivedAtMs: 3_000 }
    ];

    expect(silenceFor(levels, -40, 5_000, 3_000)).toBe(2_000);
  });
});

describe('audioLevelMeter', () => {
  it('normalizes peak into a stable visual range', () => {
    expect(audioLevelMeter({ stream: 'speaker', peak: 0.25 })).toEqual({
      stream: 'speaker',
      value: 0.25
    });
    expect(audioLevelMeter({ stream: 'speaker', peak: 2 })).toEqual({
      stream: 'speaker',
      value: 1
    });
  });
});
