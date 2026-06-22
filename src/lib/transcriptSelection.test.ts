import { describe, expect, it } from 'vitest';
import { mergeTranscriptMessages } from './transcriptSelection';
import type { ChatMessage } from './transcripts';

function message(overrides: Partial<ChatMessage>): ChatMessage {
  return {
    id: 'speaker-en-US-1000',
    role: 'speaker',
    speakerId: 'system-audio',
    speakerLabel: 'Speaker',
    language: 'en-US',
    text: 'hello',
    translation: 'こんにちは',
    isFinal: false,
    timestamp: '2026-06-15T00:00:00Z',
    segmentId: '1000-1200',
    ...overrides
  };
}

describe('mergeTranscriptMessages', () => {
  it('keeps same-language updates on one message', () => {
    const current = message({
      id: 'mic-ja-JP-1000',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Mic',
      language: 'ja-JP',
      text: 'ありが',
      translation: null,
      isFinal: false,
      segmentId: '1000-800'
    });
    const incoming = message({
      id: 'mic-ja-JP-1000',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Mic',
      language: 'ja-JP',
      text: 'ありがとうございます',
      translation: 'Thank you.',
      isFinal: true,
      segmentId: '1000-1400'
    });

    expect(mergeTranscriptMessages(current, incoming)).toMatchObject({
      id: 'mic-ja-JP-1000',
      text: 'ありがとうございます',
      translation: 'Thank you.',
      isFinal: true
    });
  });

  it('prefers the higher-confidence candidate for the same utterance across languages', () => {
    const current = message({
      id: 'speaker-en-US-1000',
      language: 'en-US',
      text: 'This is a noisy guess',
      translation: 'これはノイズ混じりの推測です',
      isFinal: true,
      confidence: 0.31,
      segmentId: '1000-1500'
    });
    const incoming = message({
      id: 'speaker-ja-JP-1000',
      language: 'ja-JP',
      text: 'こんにちは',
      translation: 'Hello.',
      isFinal: true,
      confidence: 0.84,
      segmentId: '1000-1500'
    });

    expect(mergeTranscriptMessages(current, incoming)).toMatchObject({
      id: 'speaker-ja-JP-1000',
      language: 'ja-JP',
      text: 'こんにちは',
      translation: 'Hello.'
    });
  });

  it('prefers the shorter candidate when both claim the same short segment with similar confidence', () => {
    const current = message({
      id: 'speaker-en-US-1000',
      language: 'en-US',
      text: 'yes',
      translation: 'はい',
      isFinal: true,
      confidence: 0.74,
      segmentId: '1000-500'
    });
    const incoming = message({
      id: 'speaker-ja-JP-1000',
      language: 'ja-JP',
      text: 'これは短い発話に対して長すぎる誤認識の文章です',
      translation: 'This is an overly long hallucinated sentence for a short utterance.',
      isFinal: true,
      confidence: 0.76,
      segmentId: '1000-500'
    });

    expect(mergeTranscriptMessages(current, incoming)).toMatchObject({
      id: 'speaker-en-US-1000',
      language: 'en-US',
      text: 'yes',
      translation: 'はい'
    });
  });
});
