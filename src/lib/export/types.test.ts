import { describe, expect, it } from 'vitest';
import { chatMessageToTranscriptEntry, chatMessagesToTranscriptEntries } from './types';
import type { ChatMessage } from '../transcripts';

function makeMessage(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: 'session-1-mic-en-US-1200',
    role: 'self',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'en-US',
    text: 'hello',
    translation: 'こんにちは',
    isFinal: true,
    timestamp: '2026-06-15T00:00:00Z',
    sessionId: 'session-1',
    segmentId: '1200-800',
    ...overrides
  };
}

describe('chatMessageToTranscriptEntry', () => {
  it('maps chat message fields into the export entry', () => {
    expect(chatMessageToTranscriptEntry(makeMessage())).toEqual({
      timestamp: '2026-06-15T00:00:00Z',
      speakerId: 'self',
      speakerLabel: 'Speaker A',
      language: 'en-US',
      text: 'hello',
      translation: 'こんにちは'
    });
  });

  it('keeps a null translation as null', () => {
    expect(chatMessageToTranscriptEntry(makeMessage({ translation: null })).translation).toBeNull();
  });
});

describe('chatMessagesToTranscriptEntries', () => {
  it('maps every message in order', () => {
    const entries = chatMessagesToTranscriptEntries([
      makeMessage({ text: 'first' }),
      makeMessage({ text: 'second', translation: null })
    ]);

    expect(entries).toHaveLength(2);
    expect(entries[0].text).toBe('first');
    expect(entries[1].text).toBe('second');
    expect(entries[1].translation).toBeNull();
  });

  it('returns an empty array for no messages', () => {
    expect(chatMessagesToTranscriptEntries([])).toEqual([]);
  });
});
