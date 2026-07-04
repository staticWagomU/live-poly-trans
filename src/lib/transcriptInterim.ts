import type { ChatMessage } from './transcripts';

export type TranscriptMessageState = {
  messages: ChatMessage[];
  interimMessages: ChatMessage[];
};

export function applyTranscriptMessage(
  state: TranscriptMessageState,
  incoming: ChatMessage
): TranscriptMessageState {
  if (!incoming.isFinal) {
    return {
      messages: state.messages,
      interimMessages: upsertInterimMessage(state.interimMessages, incoming)
    };
  }

  return {
    messages: upsertFinalMessage(state.messages, incoming),
    interimMessages: removeInterimMessagesForFinal(state.interimMessages, incoming)
  };
}

export function upsertFinalMessage(messages: ChatMessage[], incoming: ChatMessage) {
  const existingIndex = messages.findIndex((message) => message.id === incoming.id);

  if (existingIndex === -1) {
    return [...messages, incoming];
  }

  return messages.map((message, index) =>
    index === existingIndex
      ? { ...message, ...incoming, translation: incoming.translation ?? message.translation }
      : message
  );
}

export function upsertInterimMessage(
  interimMessages: ChatMessage[],
  incoming: ChatMessage
) {
  const matchingIndex = interimMessages.findIndex((message) =>
    sameInterimLane(message, incoming)
  );

  if (matchingIndex === -1) {
    return [...interimMessages, incoming];
  }

  return interimMessages.map((message, index) =>
    index === matchingIndex ? incoming : message
  );
}

export function removeInterimMessagesForFinal(
  interimMessages: ChatMessage[],
  finalMessage: ChatMessage
) {
  return interimMessages.filter((message) => !sameInterimLane(message, finalMessage));
}

export function sameInterimLane(left: ChatMessage, right: ChatMessage) {
  return left.role === right.role && left.speakerId === right.speakerId;
}
