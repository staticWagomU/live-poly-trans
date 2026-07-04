import { describe, expect, it } from 'vitest';
import { boundMessagesByChars, planSummaryRequest, recentChatHistory } from './aiContext';
import type { ChatMessage } from './transcripts';

function message(text: string, overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: `speaker-ja-JP-${text}`,
    role: 'speaker',
    speakerId: 'system-audio',
    speakerLabel: 'Speaker B',
    language: 'ja-JP',
    text,
    translation: null,
    isFinal: true,
    timestamp: '2026-07-04T10:00:00Z',
    segmentId: '1000-500',
    ...overrides
  };
}

describe('boundMessagesByChars', () => {
  it('keeps the most recent messages within budget', () => {
    const messages = [message('a'.repeat(100)), message('b'.repeat(100)), message('c'.repeat(100))];

    const { kept, truncated } = boundMessagesByChars(messages, 300);

    expect(kept.map((entry) => entry.text[0])).toEqual(['b', 'c']);
    expect(truncated).toBe(true);
  });

  it('always keeps at least the latest message even over budget', () => {
    const messages = [message('x'.repeat(10_000))];

    const { kept, truncated } = boundMessagesByChars(messages, 100);

    expect(kept).toHaveLength(1);
    expect(truncated).toBe(false);
  });
});

describe('planSummaryRequest', () => {
  it('returns null when nothing new happened since the covered summary', () => {
    const messages = [message('決定しました')];

    expect(planSummaryRequest(messages, 1, '既存の要約')).toBeNull();
  });

  it('sends only new messages together with the previous summary', () => {
    const messages = [message('古い発言'), message('新しい発言')];

    const plan = planSummaryRequest(messages, 1, '既存の要約');

    expect(plan?.messages.map((entry) => entry.text)).toEqual(['新しい発言']);
    expect(plan?.previousSummary).toBe('既存の要約');
    expect(plan?.coveredCount).toBe(2);
  });

  it('summarizes everything when there is no previous summary', () => {
    const messages = [message('最初の発言'), message('次の発言')];

    const plan = planSummaryRequest(messages, 2, '');

    expect(plan?.messages).toHaveLength(2);
    expect(plan?.previousSummary).toBeNull();
  });
});

describe('recentChatHistory', () => {
  it('keeps only completed turns, bounded to the most recent', () => {
    const turns = [
      { question: 'q1', answer: 'a1' },
      { question: 'q2', answer: '' },
      { question: 'q3', answer: 'a3' },
      { question: 'q4', answer: 'a4' },
      { question: 'q5', answer: 'a5' },
      { question: 'q6', answer: 'a6' }
    ];

    expect(recentChatHistory(turns, 4).map((turn) => turn.question)).toEqual([
      'q3',
      'q4',
      'q5',
      'q6'
    ]);
  });
});
