export type TranscriptEvent = {
  type: 'transcript';
  stream: 'mic' | 'speaker';
  lang: string;
  text: string;
  trans: string | null;
  isFinal: boolean;
  timestamp: string;
  segmentId?: string;
};

export type ChatMessage = {
  id: string;
  role: 'self' | 'speaker';
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
    language: event.lang,
    text: event.text,
    translation: event.trans,
    isFinal: event.isFinal,
    timestamp: event.timestamp,
    segmentId
  };
}
