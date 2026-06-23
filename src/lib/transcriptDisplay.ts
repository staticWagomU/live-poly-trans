import type { ChatMessage } from './transcripts';

export type DisplayTranscriptMessage = {
  primaryLanguage: string;
  secondaryLanguage: string | null;
  primaryText: string;
  secondaryText: string | null;
};

export function displayTranscriptMessage(
  message: ChatMessage,
  mainLanguage: string,
  subLanguage: string
): DisplayTranscriptMessage {
  if (samePrimaryLanguage(message.language, mainLanguage)) {
    return {
      primaryLanguage: message.language,
      secondaryLanguage: message.translation ? subLanguage : null,
      primaryText: message.text,
      secondaryText: message.translation
    };
  }

  if (samePrimaryLanguage(message.language, subLanguage) && message.translation) {
    return {
      primaryLanguage: mainLanguage,
      secondaryLanguage: message.language,
      primaryText: message.translation,
      secondaryText: message.text
    };
  }

  return {
    primaryLanguage: message.language,
    secondaryLanguage: message.translation ? oppositeDisplayLanguage(message, mainLanguage, subLanguage) : null,
    primaryText: message.text,
    secondaryText: message.translation
  };
}

export function samePrimaryLanguage(left: string, right: string) {
  return primaryLanguage(left) === primaryLanguage(right);
}

function oppositeDisplayLanguage(message: ChatMessage, mainLanguage: string, subLanguage: string) {
  return samePrimaryLanguage(message.language, mainLanguage) ? subLanguage : mainLanguage;
}

function primaryLanguage(language: string) {
  return language.split('-')[0]?.toLowerCase() ?? '';
}
