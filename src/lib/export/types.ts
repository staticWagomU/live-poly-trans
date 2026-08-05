import type { ChatMessage } from '../transcripts';

/// Normalized export-facing transcript line: what every export format
/// (clipboard text, JSON, TXT) needs, decoupled from UI-only ChatMessage
/// state such as interim flags and rendering ids.
export type TranscriptEntry = {
  timestamp: string;
  speakerId: string;
  speakerLabel: string;
  language: string;
  text: string;
  translation: string | null;
  startMs?: number;
  endMs?: number;
};

export function chatMessageToTranscriptEntry(message: ChatMessage): TranscriptEntry {
  return {
    timestamp: message.timestamp,
    speakerId: message.speakerId,
    speakerLabel: message.speakerLabel,
    language: message.language,
    text: message.text,
    translation: message.translation
  };
}

export function chatMessagesToTranscriptEntries(messages: ChatMessage[]): TranscriptEntry[] {
  return messages.map(chatMessageToTranscriptEntry);
}
