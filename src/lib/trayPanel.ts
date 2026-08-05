import type { AudioStream, CaptureMode } from './audioMode';
import { streamsForCaptureMode } from './audioMode';
import {
  audioLevelMeter,
  latestAudioLevel,
  silenceFor,
  streamSilenceState,
  AUDIO_SILENCE_THRESHOLD_DB,
  AUDIO_SILENCE_WINDOW_MS,
  type AudioLevelHistory
} from './audioLevels';
import { formatRecordingTimer } from './captureState';

export type TrayLaneView = {
  stream: AudioStream;
  label: string;
  subtitle: string;
  meterValue: number;
  state: 'unknown' | 'active' | 'silent' | 'inactive';
  statusLabel: string;
};

export type TrayPanelView = {
  statusLabel: string;
  timerLabel: string;
  contextLabel: string;
  overlayChecked: boolean;
  lanes: TrayLaneView[];
};

export type TrayPanelState = {
  isRecording: boolean;
  isTranscribing: boolean;
  recordingElapsedSeconds: number;
  captureMode: CaptureMode;
  activeStreams: AudioStream[];
  mainLanguage: string;
  subLanguage: string;
  overlayVisible: boolean;
  audioLevelHistory: AudioLevelHistory;
  nowMs?: number;
};

export function buildTrayPanelView(state: TrayPanelState): TrayPanelView {
  const nowMs = state.nowMs ?? Date.now();
  return {
    statusLabel: state.isRecording ? 'Recording' : state.isTranscribing ? 'Transcribing' : 'Paused',
    timerLabel: formatRecordingTimer(state.recordingElapsedSeconds),
    contextLabel: `${languageLabel(state.mainLanguage)}${languageSeparator(state.subLanguage)} · ${captureModeLabel(
      state.captureMode
    )}`,
    overlayChecked: state.overlayVisible,
    lanes: streamsForCaptureMode(state.captureMode).map((stream) =>
      trayLaneView(
        stream,
        state.audioLevelHistory,
        state.isTranscribing && state.activeStreams.includes(stream),
        nowMs
      )
    )
  };
}

function trayLaneView(
  stream: AudioStream,
  history: AudioLevelHistory,
  isTranscribing: boolean,
  nowMs: number
): TrayLaneView {
  if (!isTranscribing) {
    return {
      stream,
      label: streamLabel(stream),
      subtitle: streamSubtitle(stream),
      meterValue: 0,
      state: 'inactive',
      statusLabel: '停止中'
    };
  }

  const latest = latestAudioLevel(history, stream);
  const state = streamSilenceState(history, stream, { nowMs });
  const silentForSeconds = Math.floor(
    silenceFor(history[stream], AUDIO_SILENCE_THRESHOLD_DB, AUDIO_SILENCE_WINDOW_MS, nowMs) / 1000
  );

  return {
    stream,
    label: streamLabel(stream),
    subtitle: streamSubtitle(stream),
    meterValue: latest ? audioLevelMeter(latest).value : 0,
    state,
    statusLabel:
      state === 'silent' ? `無音 ${silentForSeconds}秒` : state === 'active' ? '良好' : '待機'
  };
}

function languageSeparator(subLanguage: string): string {
  return subLanguage ? ` ⇄ ${languageLabel(subLanguage)}` : '';
}

function languageLabel(language: string): string {
  if (language.startsWith('ja')) {
    return '日本語';
  }
  if (language.startsWith('en')) {
    return 'English';
  }
  return language || '未設定';
}

function captureModeLabel(mode: CaptureMode): string {
  if (mode === 'mic') {
    return 'Mic';
  }
  if (mode === 'speaker') {
    return 'Speaker';
  }
  return 'Both';
}

function streamLabel(stream: AudioStream): string {
  return stream === 'mic' ? 'マイク' : 'スピーカー';
}

function streamSubtitle(stream: AudioStream): string {
  return stream === 'mic' ? '入力音声' : 'システム音声';
}
