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
        timestamp: '2026-06-15T00:00:00Z',
        segmentId: '1200-800'
      })
    ).toEqual({
      id: 'mic-en-US-1200-800',
      role: 'self',
      language: 'en-US',
      text: 'hello',
      translation: 'こんにちは',
      isFinal: true,
      timestamp: '2026-06-15T00:00:00Z',
      segmentId: '1200-800'
    });
  });
});
