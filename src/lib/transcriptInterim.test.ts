import { describe, expect, it } from 'vitest';
import {
  removeInterimMessagesForFinal,
  upsertInterimMessage
} from './transcriptInterim';
import type { ChatMessage } from './transcripts';

function message(overrides: Partial<ChatMessage>): ChatMessage {
  return {
    id: 'speaker-ja-JP-1000',
    role: 'speaker',
    speakerId: 'system-audio',
    speakerLabel: 'Speaker B',
    language: 'ja-JP',
    text: 'hello',
    translation: null,
    isFinal: false,
    timestamp: '2026-06-23T10:00:00Z',
    segmentId: '1000-1200',
    ...overrides
  };
}

describe('upsertInterimMessage', () => {
  it('keeps only one interim message per speaker even when segment starts drift', () => {
    const current = message({
      id: 'speaker-ja-JP-1000',
      text: '中部の人がですね。',
      segmentId: '1000-1800'
    });
    const incoming = message({
      id: 'speaker-ja-JP-1130',
      text: '中部の人がですね出て行ったということです。',
      segmentId: '1130-2600'
    });

    expect(upsertInterimMessage([current], incoming)).toEqual([incoming]);
  });

  it('keeps microphone and speaker interim messages separate', () => {
    const speaker = message({ speakerId: 'system-audio', role: 'speaker' });
    const mic = message({
      id: 'mic-ja-JP-1000',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Speaker A',
      text: '自分の発言'
    });

    expect(upsertInterimMessage([speaker], mic)).toEqual([speaker, mic]);
  });
});

describe('removeInterimMessagesForFinal', () => {
  it('removes stale interim messages for the same speaker when a final message arrives', () => {
    const interim = message({
      id: 'speaker-ja-JP-1000',
      text: 'これもファイリスト時制で',
      isFinal: false
    });
    const finalMessage = message({
      id: 'speaker-ja-JP-1450',
      text: 'これもファイリスト時制で、と言われてますから。',
      isFinal: true,
      segmentId: '1450-3200'
    });

    expect(removeInterimMessagesForFinal([interim], finalMessage)).toEqual([]);
  });

  it('does not remove another speaker interim message', () => {
    const mic = message({
      id: 'mic-ja-JP-1000',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Speaker A',
      text: '自分の途中発言'
    });
    const speakerFinal = message({ isFinal: true });

    expect(removeInterimMessagesForFinal([mic], speakerFinal)).toEqual([mic]);
  });
});
