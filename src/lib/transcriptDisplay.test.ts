import { describe, expect, it } from 'vitest';
import { displayTranscriptMessage } from './transcriptDisplay';
import type { ChatMessage } from './transcripts';

const baseMessage: ChatMessage = {
  id: 'speaker-ja-1000',
  role: 'speaker',
  speakerId: 'system-audio',
  speakerLabel: 'Speaker B',
  language: 'ja-JP',
  text: '今日は予定を確認します。',
  translation: 'We will confirm the schedule today.',
  isFinal: true,
  timestamp: '2026-06-24T00:00:00.000Z',
  segmentId: '1000-2000'
};

describe('displayTranscriptMessage', () => {
  it('keeps text above translation when the transcript language is the main language', () => {
    expect(displayTranscriptMessage(baseMessage, 'ja-JP', 'en-US')).toEqual({
      primaryLanguage: 'ja-JP',
      secondaryLanguage: 'en-US',
      primaryText: '今日は予定を確認します。',
      secondaryText: 'We will confirm the schedule today.'
    });
  });

  it('moves translation above original text when the transcript language is the sub language', () => {
    expect(
      displayTranscriptMessage(
        {
          ...baseMessage,
          language: 'en-US',
          text: 'We will confirm the schedule today.',
          translation: '今日は予定を確認します。'
        },
        'ja-JP',
        'en-US'
      )
    ).toEqual({
      primaryLanguage: 'ja-JP',
      secondaryLanguage: 'en-US',
      primaryText: '今日は予定を確認します。',
      secondaryText: 'We will confirm the schedule today.'
    });
  });

  it('falls back to the original transcript text when main language text is unavailable', () => {
    expect(
      displayTranscriptMessage(
        {
          ...baseMessage,
          language: 'en-US',
          text: 'We will confirm the schedule today.',
          translation: null
        },
        'ja-JP',
        'en-US'
      )
    ).toEqual({
      primaryLanguage: 'en-US',
      secondaryLanguage: null,
      primaryText: 'We will confirm the schedule today.',
      secondaryText: null
    });
  });

  it('matches language priority by primary language code', () => {
    expect(displayTranscriptMessage(baseMessage, 'ja', 'en-GB').primaryLanguage).toBe('ja-JP');
  });
});
