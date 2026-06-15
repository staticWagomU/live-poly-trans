export type TranscriptEvent = {
  type: 'transcript';
  stream: 'mic' | 'speaker';
  lang: string;
  text: string;
  trans: string | null;
  isFinal: boolean;
  timestamp: string;
};

export type ChatMessage = {
  id: string;
  role: 'self' | 'speaker';
  language: string;
  text: string;
  translation: string | null;
  isFinal: boolean;
  timestamp: string;
};

export function transcriptEventToMessage(event: TranscriptEvent): ChatMessage {
  return {
    id: `${event.timestamp}-${event.stream}-${event.lang}-${event.text}`,
    role: event.stream === 'mic' ? 'self' : 'speaker',
    language: event.lang,
    text: event.text,
    translation: event.trans,
    isFinal: event.isFinal,
    timestamp: event.timestamp
  };
}
