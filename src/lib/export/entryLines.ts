import type { ExportOptions, TranscriptEntry } from './types';

/// Collapses each whitespace run containing a newline into a single space, so
/// multiline source text can never inject blank lines (cue separators in
/// SRT/VTT, block breaks in Markdown) into line-based export output.
export function collapseNewlines(text: string): string {
  return text.replace(/[^\S\n]*\n\s*/g, ' ');
}

/// Renders one entry into its text line(s) per the shared export options.
/// See ExportOptions for the translation-mode semantics; the result is never
/// empty because 'translationOnly' falls back to the original text.
export function entryLines(entry: TranscriptEntry, opts: ExportOptions): string[] {
  const lines = (
    opts.translation === 'translationOnly'
      ? [entry.translation ?? entry.text]
      : opts.translation === 'both' && entry.translation !== null
        ? [entry.text, entry.translation]
        : [entry.text]
  ).map(collapseNewlines);

  if (!opts.speakerPrefix) {
    return lines;
  }

  const [first, ...rest] = lines;
  return [`${entry.speakerLabel}: ${first}`, ...rest];
}
