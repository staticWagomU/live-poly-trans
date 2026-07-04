import type { ChatMessage } from './transcripts';

export type SummaryRequestPlan = {
  messages: ChatMessage[];
  previousSummary: string | null;
  coveredCount: number;
  truncated: boolean;
};

export type ChatTurn = {
  question: string;
  answer: string;
};

// The on-device Apple Intelligence model has a small context window
// (~4k tokens), so requests carry the previous summary plus only a bounded
// slice of new transcript instead of the whole meeting.
const defaultSummaryBudgetChars = 6000;
const defaultHistoryTurns = 4;

export function boundMessagesByChars(
  messages: ChatMessage[],
  budgetChars: number
): { kept: ChatMessage[]; truncated: boolean } {
  const kept: ChatMessage[] = [];
  let total = 0;

  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    const cost = message.text.length + (message.translation?.length ?? 0) + 40;

    if (kept.length > 0 && total + cost > budgetChars) {
      break;
    }

    kept.unshift(message);
    total += cost;
  }

  return { kept, truncated: kept.length < messages.length };
}

export function planSummaryRequest(
  allMessages: ChatMessage[],
  coveredCount: number,
  previousSummary: string,
  budgetChars = defaultSummaryBudgetChars
): SummaryRequestPlan | null {
  const hasPreviousSummary = previousSummary.trim().length > 0;
  const source = hasPreviousSummary ? allMessages.slice(coveredCount) : allMessages;

  if (source.length === 0) {
    return null;
  }

  const { kept, truncated } = boundMessagesByChars(source, budgetChars);

  return {
    messages: kept,
    previousSummary: hasPreviousSummary ? previousSummary : null,
    coveredCount: allMessages.length,
    truncated
  };
}

export function recentChatHistory(turns: ChatTurn[], maxTurns = defaultHistoryTurns): ChatTurn[] {
  return turns.filter((turn) => turn.answer.trim().length > 0).slice(-maxTurns);
}
