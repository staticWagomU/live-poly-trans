import { describe, expect, it } from 'vitest';
import { buildOverlayCaptionLines } from './overlayCaptions';
import type { ChatMessage } from './transcripts';

function message(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: 'm-1',
    role: 'self',
    speakerId: 'self',
    speakerLabel: '自分',
    language: 'en-US',
    text: 'Hello',
    translation: 'こんにちは',
    isFinal: true,
    timestamp: '2026-08-05T10:00:00+09:00',
    segmentId: '0-1000',
    ...overrides
  };
}

describe('buildOverlayCaptionLines', () => {
  it('keeps the newest lines up to the configured count', () => {
    const lines = buildOverlayCaptionLines(
      [
        message({ id: 'm-1', text: 'first' }),
        message({ id: 'm-2', text: 'second' }),
        message({ id: 'm-3', text: 'third' })
      ],
      { mainLanguage: 'en-US', subLanguage: 'ja-JP', maxLines: 2, showTranslation: false }
    );

    expect(lines.map((line) => line.primary)).toEqual(['second', 'third']);
  });

  it('includes translated text when enabled', () => {
    expect(
      buildOverlayCaptionLines([message()], {
        mainLanguage: 'en-US',
        subLanguage: 'ja-JP',
        maxLines: 1,
        showTranslation: true
      })
    ).toEqual([{ primary: 'Hello', secondary: 'こんにちは', id: 'm-1' }]);
  });
});
