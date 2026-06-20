import { describe, expect, it } from 'vitest';
import { transcriptEventToMessage } from './transcripts';

describe('transcriptEventToMessage', () => {
  it('maps microphone events to self chat messages', () => {
    expect(
      transcriptEventToMessage({
        type: 'transcript',
        stream: 'mic',
        speakerId: 'self',
        speakerLabel: 'Mic',
        lang: 'en-US',
        text: 'hello',
        trans: 'こんにちは',
        isFinal: true,
        timestamp: '2026-06-15T00:00:00Z',
        sessionId: 'mic-session-1',
        segmentId: '1200-800'
      })
    ).toEqual({
      id: 'mic-en-US-1200',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Mic',
      language: 'en-US',
      text: 'hello',
      translation: 'こんにちは',
      isFinal: true,
      timestamp: '2026-06-15T00:00:00Z',
      segmentId: '1200-800'
    });
  });

  it('uses native stream labels as speaker fallback for older events', () => {
    expect(
      transcriptEventToMessage({
        type: 'transcript',
        stream: 'speaker',
        lang: 'en-US',
        text: 'remote audio',
        trans: null,
        isFinal: true,
        timestamp: '2026-06-15T00:00:00Z',
        sessionId: 'speaker-session-1',
        segmentId: '3200-1000'
      })
    ).toMatchObject({
      role: 'speaker',
      speakerId: 'system-audio',
      speakerLabel: 'Speaker'
    });
  });

  it('keeps volatile updates for the same start time in one chat message', () => {
    const first = transcriptEventToMessage({
      type: 'transcript',
      stream: 'mic',
      lang: 'ja-JP',
      text: 'ありがとう',
      trans: 'Thank you.',
      isFinal: false,
      timestamp: '2026-06-15T00:00:00Z',
      sessionId: 'mic-session-1',
      segmentId: '21840-3245'
    });
    const update = transcriptEventToMessage({
      type: 'transcript',
      stream: 'mic',
      lang: 'ja-JP',
      text: 'ありがとうございます。',
      trans: 'Thank you.',
      isFinal: false,
      timestamp: '2026-06-15T00:00:01Z',
      sessionId: 'mic-session-1',
      segmentId: '21840-4145'
    });

    expect(update.id).toBe(first.id);
  });
});
