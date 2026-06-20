export type TranscriptEvent = {
  type: 'transcript';
  stream: 'mic' | 'speaker';
  speakerId?: string;
  speakerLabel?: string;
  lang: string;
  text: string;
  trans: string | null;
  isFinal: boolean;
  timestamp: string;
  sessionId: string;
  segmentId?: string;
};

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
};

export function transcriptEventToMessage(event: TranscriptEvent): ChatMessage {
  const segmentId = event.segmentId ?? `${event.timestamp}-${event.text}`;
  const stableSegmentId = segmentId.split('-')[0] || segmentId;

  return {
    id: `${event.stream}-${event.lang}-${stableSegmentId}`,
    role: event.stream === 'mic' ? 'self' : 'speaker',
    speakerId: event.speakerId ?? fallbackSpeakerId(event.stream),
    speakerLabel: event.speakerLabel ?? fallbackSpeakerLabel(event.stream),
    language: event.lang,
    text: event.text,
    translation: event.trans,
    isFinal: event.isFinal,
    timestamp: event.timestamp,
    segmentId
  };
}

export function fallbackSpeakerId(stream: TranscriptEvent['stream']) {
  return stream === 'mic' ? 'self' : 'system-audio';
}

export function fallbackSpeakerLabel(stream: TranscriptEvent['stream']) {
  return stream === 'mic' ? 'Mic' : 'Speaker';
}
