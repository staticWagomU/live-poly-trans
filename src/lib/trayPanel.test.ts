import { describe, expect, it } from 'vitest';
import { emptyAudioLevelHistory } from './audioLevels';
import { buildTrayPanelView } from './trayPanel';

describe('buildTrayPanelView', () => {
  it('shows recording timer, language pair, and capture mode label', () => {
    const view = buildTrayPanelView({
      isRecording: true,
      isTranscribing: true,
      recordingElapsedSeconds: 754,
      captureMode: 'both',
      mainLanguage: 'ja-JP',
      subLanguage: 'en-US',
      overlayVisible: true,
      audioLevelHistory: emptyAudioLevelHistory(),
      nowMs: 10_000
    });

    expect(view.statusLabel).toBe('Recording');
    expect(view.timerLabel).toBe('12:34');
    expect(view.contextLabel).toBe('日本語 ⇄ English · Both');
    expect(view.overlayChecked).toBe(true);
  });

  it('marks an active stream silent after the silence window', () => {
    const view = buildTrayPanelView({
      isRecording: false,
      isTranscribing: true,
      recordingElapsedSeconds: 0,
      captureMode: 'speaker',
      mainLanguage: 'en-US',
      subLanguage: '',
      overlayVisible: false,
      audioLevelHistory: {
        mic: [],
        speaker: [
          {
            stream: 'speaker',
            sampleCount: 960,
            rms: 0.001,
            peak: 0.003,
            timestamp: '2026-08-05T00:00:00Z',
            receivedAtMs: 0
          }
        ]
      },
      nowMs: 3_000
    });

    expect(view.lanes).toEqual([
      expect.objectContaining({
        stream: 'speaker',
        label: 'スピーカー',
        state: 'silent',
        statusLabel: '無音 3秒'
      })
    ]);
  });
});
