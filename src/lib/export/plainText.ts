import type { TranscriptEntry } from './types';

/// Formats entries into the plain-text transcript used by the clipboard
/// copy action. Mirrors the Rust TXT export format in
/// src-tauri/src/lib.rs (save_transcript_blocking); keep them in sync.
export function toPlainText(entries: TranscriptEntry[]): string {
  return entries
    .map((entry) => {
      const translation = entry.translation ? `\n  => ${entry.translation}` : '';
      return `[${entry.timestamp}] ${entry.speakerLabel} / ${entry.language}: ${entry.text}${translation}`;
    })
    .join('\n');
}
