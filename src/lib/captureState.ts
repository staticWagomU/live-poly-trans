import { streamsForCaptureMode, type AudioStream, type CaptureMode } from './audioMode';

/// Which helper session, if any, each stream is currently attached to.
/// Events carry the session id they were produced under; anything that does
/// not match the current id here is from a session the user already left.
export type StreamSessions = Record<AudioStream, string | null>;

export function emptyStreamSessions(): StreamSessions {
  return { mic: null, speaker: null };
}

export function withStreamSession(
  sessions: StreamSessions,
  stream: AudioStream,
  sessionId: string | null
): StreamSessions {
  return { ...sessions, [stream]: sessionId };
}

export function withoutStreamSessions(
  sessions: StreamSessions,
  streams: AudioStream[]
): StreamSessions {
  const next = { ...sessions };
  for (const stream of streams) {
    next[stream] = null;
  }
  return next;
}

export function isCurrentSessionEvent(
  sessions: StreamSessions,
  event: { stream: AudioStream; sessionId: string }
): boolean {
  return sessions[event.stream] === event.sessionId;
}

/// A recording session the user opened on top of the always-running
/// transcription. Transcription state (activeStreams) and recording state
/// are deliberately independent: stopping one must not touch the other.
export type RecordingSession = {
  id: string;
  dir: string;
  startedAtMs: number;
  startMessageIndex: number;
};

export function recordingElapsedSeconds(
  session: Pick<RecordingSession, 'startedAtMs'>,
  nowMs: number
): number {
  return Math.max(0, Math.floor((nowMs - session.startedAtMs) / 1000));
}

export function formatRecordingTimer(elapsedSeconds: number): string {
  const hours = Math.floor(elapsedSeconds / 3600);
  const minutes = Math.floor((elapsedSeconds % 3600) / 60);
  const seconds = elapsedSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

/// Decides whether an auto-restart attempt may still start its stream once
/// its backoff delay has elapsed. The world can change during the delay in
/// three ways, each of which must cancel the restart:
/// - the user stopped or restarted capture (generation bump),
/// - the user switched to a capture mode without this stream,
/// - something else already started a new session on this stream.
export function shouldRestartStream(params: {
  generationAtExit: number;
  currentGeneration: number;
  captureMode: CaptureMode;
  stream: AudioStream;
  sessions: StreamSessions;
}): boolean {
  return (
    params.generationAtExit === params.currentGeneration &&
    streamsForCaptureMode(params.captureMode).includes(params.stream) &&
    params.sessions[params.stream] === null
  );
}
