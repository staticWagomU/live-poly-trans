import { describe, expect, it } from 'vitest';
import {
  applyTranslationEvent,
  interleaveThreadItems,
  isRecordingMarker,
  recordingStartMarker,
  recordingStopMarker,
  transcriptEventToMessage,
  windowThreadItems
} from './transcripts';
import type { ChatMessage } from './transcripts';

describe('transcriptEventToMessage', () => {
  it('maps microphone events to self chat messages', () => {
    expect(
      transcriptEventToMessage({
        type: 'transcript',
        stream: 'mic',
        speakerId: 'self',
        speakerLabel: 'Speaker A',
        lang: 'en-US',
        text: 'hello',
        trans: 'こんにちは',
        isFinal: true,
        timestamp: '2026-06-15T00:00:00Z',
        sessionId: 'mic-session-1',
        segmentId: '1200-800'
      })
    ).toEqual({
      id: 'mic-session-1-mic-en-US-1200',
      role: 'self',
      speakerId: 'self',
      speakerLabel: 'Speaker A',
      language: 'en-US',
      text: 'hello',
      translation: 'こんにちは',
      isFinal: true,
      timestamp: '2026-06-15T00:00:00Z',
      sessionId: 'mic-session-1',
      segmentId: '1200-800'
    });
  });

  it('keeps messages from different helper sessions apart', () => {
    // After an auto-restart the helper's clock starts back at 0, so segment
    // ids repeat; a bubble from the top of the meeting must not be replaced
    // in place by a post-restart segment.
    const beforeRestart = transcriptEventToMessage({
      type: 'transcript',
      stream: 'mic',
      lang: 'ja-JP',
      text: '冒頭の発話',
      trans: null,
      isFinal: true,
      timestamp: '2026-06-15T00:00:00Z',
      sessionId: 'mic-session-1',
      segmentId: '0-1000'
    });
    const afterRestart = transcriptEventToMessage({
      type: 'transcript',
      stream: 'mic',
      lang: 'ja-JP',
      text: '再起動後の発話',
      trans: null,
      isFinal: true,
      timestamp: '2026-06-15T02:00:00Z',
      sessionId: 'mic-session-2',
      segmentId: '0-1000'
    });

    expect(afterRestart.id).not.toBe(beforeRestart.id);
  });

  it('uses speaker letters as fallback for older events', () => {
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
      speakerLabel: 'Speaker B'
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

  it('preserves confidence metadata for transcript JSON export', () => {
    expect(
      transcriptEventToMessage({
        type: 'transcript',
        stream: 'mic',
        lang: 'en-US',
        text: 'hello',
        trans: null,
        isFinal: true,
        time: '2026-06-15T00:00:00Z',
        timestamp: '2026-06-15T00:00:01Z',
        sessionId: 'mic-session-1',
        segmentId: '100-500',
        confidence: 0.75,
        detectedLang: 'en',
        detectedLangConfidence: 0.92,
        spans: [{ text: 'hello', confidence: 0.75, startMs: 100, endMs: 600 }]
      })
    ).toMatchObject({
      timestamp: '2026-06-15T00:00:00Z',
      confidence: 0.75,
      detectedLanguage: 'en',
      detectedLanguageConfidence: 0.92,
      spans: [{ text: 'hello', confidence: 0.75, startMs: 100, endMs: 600 }]
    });
  });
});

describe('applyTranslationEvent', () => {
  const finalMessage: ChatMessage = {
    id: 'speaker-session-1-speaker-en-US-5000',
    role: 'speaker',
    speakerId: 'system-audio',
    speakerLabel: 'Speaker B',
    language: 'en-US',
    text: 'the release is Friday',
    translation: null,
    isFinal: true,
    timestamp: '2026-07-04T10:00:00Z',
    sessionId: 'speaker-session-1',
    segmentId: '5000-2000'
  };

  it('patches the matching final message with the delivered translation', () => {
    const updated = applyTranslationEvent([finalMessage], {
      stream: 'speaker',
      sessionId: 'speaker-session-1',
      segmentId: '5000-2000',
      trans: 'リリースは金曜日です'
    });

    expect(updated[0].translation).toBe('リリースは金曜日です');
  });

  it('leaves messages untouched when nothing matches', () => {
    const updated = applyTranslationEvent([finalMessage], {
      stream: 'mic',
      sessionId: 'speaker-session-1',
      segmentId: '5000-2000',
      trans: '別ストリームの翻訳'
    });

    expect(updated).toEqual([finalMessage]);
  });

  it('does not patch a message from a different helper session', () => {
    const updated = applyTranslationEvent([finalMessage], {
      stream: 'speaker',
      sessionId: 'speaker-session-2',
      segmentId: '5000-2000',
      trans: '再起動後セッションの翻訳'
    });

    expect(updated).toEqual([finalMessage]);
  });
});

describe('recording markers', () => {
  const message = (id: string): ChatMessage => ({
    id,
    role: 'self',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'en-US',
    text: id,
    translation: null,
    isFinal: true,
    timestamp: '2026-07-31T00:00:00Z',
    segmentId: id
  });

  it('builds start and stop markers anchored to the current message count', () => {
    const start = recordingStartMarker('rec-1', '2026-07-31T10:00:00Z', 2);

    expect(start).toEqual({
      kind: 'recording-marker',
      id: 'rec-1-start',
      phase: 'start',
      timestamp: '2026-07-31T10:00:00Z',
      afterMessageCount: 2
    });

    expect(recordingStopMarker('rec-1', '2026-07-31T10:05:30Z', 5, 330)).toEqual({
      kind: 'recording-marker',
      id: 'rec-1-stop',
      phase: 'stop',
      timestamp: '2026-07-31T10:05:30Z',
      afterMessageCount: 5,
      durationSeconds: 330
    });
  });

  it('identifies markers among thread items', () => {
    expect(isRecordingMarker(recordingStartMarker('rec-1', 't', 0))).toBe(true);
    expect(isRecordingMarker(message('m1'))).toBe(false);
  });

  it('interleaves markers at their anchored positions', () => {
    const messages = [message('m1'), message('m2'), message('m3')];
    const start = recordingStartMarker('rec-1', 't1', 1);
    const stop = recordingStopMarker('rec-1', 't2', 3, 60);

    expect(interleaveThreadItems(messages, [start, stop]).map((item) => item.id)).toEqual([
      'm1',
      'rec-1-start',
      'm2',
      'm3',
      'rec-1-stop'
    ]);
  });

  it('keeps marker order stable when several share a position', () => {
    const messages = [message('m1')];
    const stop = recordingStopMarker('rec-1', 't1', 1, 10);
    const start = recordingStartMarker('rec-2', 't2', 1);

    expect(interleaveThreadItems(messages, [stop, start]).map((item) => item.id)).toEqual([
      'm1',
      'rec-1-stop',
      'rec-2-start'
    ]);
  });

  it('clamps markers anchored beyond the current list to the end', () => {
    const messages = [message('m1')];
    const start = recordingStartMarker('rec-1', 't1', 9);

    expect(interleaveThreadItems(messages, [start]).map((item) => item.id)).toEqual([
      'm1',
      'rec-1-start'
    ]);
  });
});

describe('windowThreadItems', () => {
  const message = (id: string): ChatMessage => ({
    id,
    role: 'self',
    speakerId: 'self',
    speakerLabel: 'Speaker A',
    language: 'en-US',
    text: id,
    translation: null,
    isFinal: true,
    timestamp: '2026-07-31T00:00:00Z',
    segmentId: id
  });

  it('returns everything when under the limit', () => {
    const items = [message('m1'), message('m2')];

    expect(windowThreadItems(items, 5)).toEqual({ items, hiddenCount: 0 });
  });

  it('keeps only the newest items and reports how many are hidden', () => {
    const items = [message('m1'), message('m2'), message('m3'), message('m4')];

    const windowed = windowThreadItems(items, 2);

    expect(windowed.items.map((item) => item.id)).toEqual(['m3', 'm4']);
    expect(windowed.hiddenCount).toBe(2);
  });
});
