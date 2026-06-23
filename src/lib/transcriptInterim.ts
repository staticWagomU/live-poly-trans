import type { ChatMessage } from './transcripts';

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
