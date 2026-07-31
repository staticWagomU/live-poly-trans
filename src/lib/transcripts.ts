export type TranscriptEvent = {
  type: 'transcript';
  stream: 'mic' | 'speaker';
  speakerId?: string;
  speakerLabel?: string;
  lang: string;
  text: string;
  trans: string | null;
  isFinal: boolean;
  time?: string;
  timestamp: string;
  sessionId: string;
  segmentId?: string;
  confidence?: number;
  detectedLang?: string;
  detectedLangConfidence?: number;
  spans?: TranscriptSpan[];
};

export type TranscriptSpan = {
  text: string;
  confidence?: number;
  startMs?: number;
  endMs?: number;
};

export type TranslationEvent = {
  type: 'translation';
  stream: 'mic' | 'speaker';
  segmentId: string;
  language: string;
  targetLanguage: string;
  trans: string;
  timestamp: string;
  sessionId: string;
};

export type StatusEvent = {
  type: 'status';
  stream: 'mic' | 'speaker';
  state: string;
  lang?: string;
  sessionId: string;
};

export type HelperEvent = TranscriptEvent | TranslationEvent | StatusEvent;

export type ChatMessage = {
  id: string;
  role: 'self' | 'speaker';
  speakerId: string;
  speakerLabel: string;
  language: string;
  text: string;
  translation: string | null;
  isFinal: boolean;
  timestamp: string;
  sessionId?: string;
  segmentId: string;
  /// True when this utterance was captured while a recording session was
  /// open; the caption stream draws the red recording rail on it.
  inRecording?: boolean;
  confidence?: number;
  detectedLanguage?: string;
  detectedLanguageConfidence?: number;
  spans?: TranscriptSpan[];
};

/// A recording start/stop line rendered inside the caption stream. Markers
/// are anchored by how many final messages existed when they were created
/// (arrival order), not by timestamp — mic and speaker clocks are not
/// comparable, but arrival order is what the user saw.
export type RecordingMarker = {
  kind: 'recording-marker';
  id: string;
  phase: 'start' | 'stop';
  timestamp: string;
  afterMessageCount: number;
  durationSeconds?: number;
};

export type ThreadItem = ChatMessage | RecordingMarker;

export function isRecordingMarker(item: ThreadItem): item is RecordingMarker {
  return 'kind' in item && item.kind === 'recording-marker';
}

export function recordingStartMarker(
  recordingId: string,
  timestamp: string,
  afterMessageCount: number
): RecordingMarker {
  return {
    kind: 'recording-marker',
    id: `${recordingId}-start`,
    phase: 'start',
    timestamp,
    afterMessageCount
  };
}

export function recordingStopMarker(
  recordingId: string,
  timestamp: string,
  afterMessageCount: number,
  durationSeconds: number
): RecordingMarker {
  return {
    kind: 'recording-marker',
    id: `${recordingId}-stop`,
    phase: 'stop',
    timestamp,
    afterMessageCount,
    durationSeconds
  };
}

/// Weaves markers into the message list at their anchored positions.
/// Markers anchored past the end (messages were cleared afterwards) clamp
/// to the end instead of disappearing.
export function interleaveThreadItems(
  messages: ChatMessage[],
  markers: RecordingMarker[]
): ThreadItem[] {
  const items: ThreadItem[] = [];

  for (let position = 0; position <= messages.length; position += 1) {
    for (const marker of markers) {
      const anchor = Math.min(marker.afterMessageCount, messages.length);
      if (anchor === position) {
        items.push(marker);
      }
    }

    if (position < messages.length) {
      items.push(messages[position]);
    }
  }

  return items;
}

export function transcriptEventToMessage(event: TranscriptEvent): ChatMessage {
  const segmentId = event.segmentId ?? `${event.timestamp}-${event.text}`;
  const stableSegmentId = segmentId.split('-')[0] || segmentId;

  const message: ChatMessage = {
    // Namespaced by helper session: after an auto-restart the helper clock
    // resets to 0, so segment ids repeat and would otherwise replace bubbles
    // from earlier in the meeting in place.
    id: `${event.sessionId}-${event.stream}-${event.lang}-${stableSegmentId}`,
    role: event.stream === 'mic' ? 'self' : 'speaker',
    speakerId: event.speakerId ?? fallbackSpeakerId(event.stream),
    speakerLabel: event.speakerLabel ?? fallbackSpeakerLabel(event.stream),
    language: event.lang,
    text: event.text,
    translation: event.trans,
    isFinal: event.isFinal,
    timestamp: event.time ?? event.timestamp,
    sessionId: event.sessionId,
    segmentId
  };

  if (event.confidence !== undefined) {
    message.confidence = event.confidence;
  }

  if (event.detectedLang !== undefined) {
    message.detectedLanguage = event.detectedLang;
  }

  if (event.detectedLangConfidence !== undefined) {
    message.detectedLanguageConfidence = event.detectedLangConfidence;
  }

  if (event.spans !== undefined) {
    message.spans = event.spans;
  }

  return message;
}

export function fallbackSpeakerId(stream: TranscriptEvent['stream']) {
  return stream === 'mic' ? 'self' : 'system-audio';
}

export function fallbackSpeakerLabel(stream: TranscriptEvent['stream']) {
  return stream === 'mic' ? 'Speaker A' : 'Speaker B';
}

export function applyTranslationEvent(
  messages: ChatMessage[],
  event: Pick<TranslationEvent, 'stream' | 'segmentId' | 'trans'> &
    Partial<Pick<TranslationEvent, 'sessionId'>>
): ChatMessage[] {
  const role = event.stream === 'mic' ? 'self' : 'speaker';
  const index = messages.findIndex(
    (message) =>
      message.role === role &&
      message.segmentId === event.segmentId &&
      // Segment ids repeat across helper restarts; a translation may only
      // patch the message from its own session. Messages or events without a
      // session (older exports) keep the previous permissive matching.
      (message.sessionId === undefined ||
        event.sessionId === undefined ||
        message.sessionId === event.sessionId)
  );

  if (index === -1) {
    return messages;
  }

  return messages.map((message, messageIndex) =>
    messageIndex === index ? { ...message, translation: event.trans } : message
  );
}
