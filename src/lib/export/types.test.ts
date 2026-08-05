import { describe, expect, it } from 'vitest';
import {
  chatMessageToTranscriptEntry,
  chatMessagesToTranscriptEntries,
  recordingItemToTranscriptEntry
} from './types';
import type { ChatMessage } from '../transcripts';
import type { RecordingTranscriptItem } from '../recordings';

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
      translation: 'こんにちは',
      startMs: 1_200,
      endMs: 2_000
    });
  });

  it('keeps a null translation as null', () => {
    expect(chatMessageToTranscriptEntry(makeMessage({ translation: null })).translation).toBeNull();
  });

  it('leaves timings undefined for a malformed segment id', () => {
    const entry = chatMessageToTranscriptEntry(makeMessage({ segmentId: 'broken' }));

    expect(entry.startMs).toBeUndefined();
    expect(entry.endMs).toBeUndefined();
  });
});

describe('recordingItemToTranscriptEntry', () => {
  function makeItem(overrides: Partial<RecordingTranscriptItem> = {}): RecordingTranscriptItem {
    return {
      key: '0-mic-1000-1500',
      startMs: 1_000,
      stream: 'mic',
      speakerLabel: 'Speaker A',
      language: 'ja-JP',
      text: 'こんにちは',
      translation: 'hello',
      ...overrides
    };
  }

  it('maps a recording item with timings into the export entry', () => {
    expect(recordingItemToTranscriptEntry(makeItem({ endMs: 2_500 }))).toEqual({
      timestamp: '0:01',
      speakerId: 'mic',
      speakerLabel: 'Speaker A',
      language: 'ja-JP',
      text: 'こんにちは',
      translation: 'hello',
      startMs: 1_000,
      endMs: 2_500
    });
  });

  it('leaves endMs undefined when the item has none', () => {
    expect(recordingItemToTranscriptEntry(makeItem()).endMs).toBeUndefined();
  });

  it('anchors the timestamp to the recording start when provided', () => {
    const entry = recordingItemToTranscriptEntry(makeItem(), {
      baseTimestamp: '2026-06-15T00:00:00Z'
    });

    expect(entry.timestamp).toBe('2026-06-15T00:00:01.000Z');
  });

  it('uses the diarized speaker index as the speaker id when present', () => {
    expect(recordingItemToTranscriptEntry(makeItem({ speakerIndex: 1 })).speakerId).toBe(
      'speaker-1'
    );
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
