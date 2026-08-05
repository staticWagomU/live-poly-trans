import { displayTranscriptMessage } from './transcriptDisplay';
import type { ChatMessage } from './transcripts';

export type OverlayCaptionLine = {
  id: string;
  primary: string;
  secondary: string | null;
};

export type OverlayCaptionOptions = {
  mainLanguage: string;
  subLanguage: string;
  maxLines: number;
  showTranslation: boolean;
};

export function buildOverlayCaptionLines(
  messages: ChatMessage[],
  options: OverlayCaptionOptions
): OverlayCaptionLine[] {
  const maxLines = Math.max(1, Math.min(3, Math.floor(options.maxLines)));
  return messages.slice(-maxLines).map((message) => {
    const display = displayTranscriptMessage(message, options.mainLanguage, options.subLanguage);
    return {
      id: message.id,
      primary: display.primaryText,
      secondary: options.showTranslation ? display.secondaryText : null
    };
  });
}
