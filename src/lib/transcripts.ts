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
  segmentId: string;
  confidence?: number;
  detectedLanguage?: string;
  detectedLanguageConfidence?: number;
  spans?: TranscriptSpan[];
};

export function transcriptEventToMessage(event: TranscriptEvent): ChatMessage {
  const segmentId = event.segmentId ?? `${event.timestamp}-${event.text}`;
  const stableSegmentId = segmentId.split('-')[0] || segmentId;

  const message: ChatMessage = {
    id: `${event.stream}-${event.lang}-${stableSegmentId}`,
    role: event.stream === 'mic' ? 'self' : 'speaker',
    speakerId: event.speakerId ?? fallbackSpeakerId(event.stream),
    speakerLabel: event.speakerLabel ?? fallbackSpeakerLabel(event.stream),
    language: event.lang,
    text: event.text,
    translation: event.trans,
    isFinal: event.isFinal,
    timestamp: event.time ?? event.timestamp,
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
  event: Pick<TranslationEvent, 'stream' | 'segmentId' | 'trans'>
): ChatMessage[] {
  const role = event.stream === 'mic' ? 'self' : 'speaker';
  const index = messages.findIndex(
    (message) => message.role === role && message.segmentId === event.segmentId
  );

  if (index === -1) {
    return messages;
  }

  return messages.map((message, messageIndex) =>
    messageIndex === index ? { ...message, translation: event.trans } : message
  );
}
