import type { ExportOptions, TranscriptEntry } from './types';

/// Renders one entry into its text line(s) per the shared export options.
/// See ExportOptions for the translation-mode semantics; the result is never
/// empty because 'translationOnly' falls back to the original text.
export function entryLines(entry: TranscriptEntry, opts: ExportOptions): string[] {
  const lines =
    opts.translation === 'translationOnly'
      ? [entry.translation ?? entry.text]
      : opts.translation === 'both' && entry.translation !== null
        ? [entry.text, entry.translation]
        : [entry.text];

  if (!opts.speakerPrefix) {
    return lines;
  }

  const [first, ...rest] = lines;
  return [`${entry.speakerLabel}: ${first}`, ...rest];
}
