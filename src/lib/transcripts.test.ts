import { describe, expect, it } from 'vitest';
import { transcriptEventToMessage } from './transcripts';

describe('transcriptEventToMessage', () => {
  it('maps microphone events to self chat messages', () => {
    expect(
      transcriptEventToMessage({
        type: 'transcript',
        stream: 'mic',
        lang: 'en-US',
        text: 'hello',
        trans: 'こんにちは',
        isFinal: true,
        timestamp: '2026-06-15T00:00:00Z'
      })
    ).toEqual({
      id: '2026-06-15T00:00:00Z-mic-en-US-hello',
      role: 'self',
      language: 'en-US',
      text: 'hello',
      translation: 'こんにちは',
      isFinal: true,
      timestamp: '2026-06-15T00:00:00Z'
    });
  });
});
