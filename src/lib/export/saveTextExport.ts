import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { toMarkdown, type MarkdownMeta } from './markdown';
import { toPlainText } from './plainText';
import { toSrt } from './srt';
import { toVtt } from './vtt';
import type { ExportOptions, TranscriptEntry } from './types';

export type TextExportFormat = 'markdown' | 'srt' | 'vtt' | 'txt';

export type TextExportFile = {
  contents: string;
  extension: string;
  /// Human-readable label for the save dialog's file-type filter.
  filterName: string;
  defaultFileName: string;
};

/// One-click exports ship with sensible defaults; the per-export options
/// dialog is a deferred future extension (plan.md 1-3).
export const DEFAULT_TEXT_EXPORT_OPTIONS: ExportOptions = {
  speakerPrefix: true,
  translation: 'both'
};

/// Strips characters that are path separators or reserved on common
/// filesystems so a recording title can seed a save-dialog file name.
export function sanitizeFileName(base: string): string {
  const cleaned = base.replace(/[/\\:*?"<>|]/g, '-').trim();
  return cleaned === '' ? 'transcript' : cleaned;
}

/// Participants in first-appearance order for the Markdown 参加者 line.
export function uniqueSpeakerLabels(entries: TranscriptEntry[]): string[] {
  const labels: string[] = [];
  for (const entry of entries) {
    if (entry.speakerLabel !== '' && !labels.includes(entry.speakerLabel)) {
      labels.push(entry.speakerLabel);
    }
  }
  return labels;
}

/// Local `YYYYMMDD-HHMMSS`, mirroring readable_timestamp() in
/// src-tauri/src/lib.rs so frontend- and Rust-named files sort together.
export function timestampLabel(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return (
    `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}` +
    `-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`
  );
}

export function buildTextExport(
  format: TextExportFormat,
  entries: TranscriptEntry[],
  opts: { baseName: string; markdownMeta?: MarkdownMeta; exportOptions?: ExportOptions }
): TextExportFile {
  const exportOptions = opts.exportOptions ?? DEFAULT_TEXT_EXPORT_OPTIONS;

  let contents: string;
  let extension: string;
  let filterName: string;
  switch (format) {
    case 'markdown':
      contents = toMarkdown(entries, opts.markdownMeta ?? {}, exportOptions);
      extension = 'md';
      filterName = 'Markdown';
      break;
    case 'srt':
      contents = toSrt(entries, exportOptions);
      extension = 'srt';
      filterName = 'SubRip 字幕';
      break;
    case 'vtt':
      contents = toVtt(entries, exportOptions);
      extension = 'vtt';
      filterName = 'WebVTT 字幕';
      break;
    case 'txt':
      contents = toPlainText(entries);
      extension = 'txt';
      filterName = 'テキスト';
      break;
  }

  return {
    contents,
    extension,
    filterName,
    defaultFileName: `${sanitizeFileName(opts.baseName)}.${extension}`
  };
}

/// Thin shell around the pure builder result: asks the user where to save,
/// then hands the generated text to Rust. Returns the chosen path, or null
/// when the user cancels the dialog.
export async function saveTextExportToFile(file: TextExportFile): Promise<string | null> {
  const path = await save({
    defaultPath: file.defaultFileName,
    filters: [{ name: file.filterName, extensions: [file.extension] }]
  });
  if (path === null) {
    return null;
  }

  await invoke('save_text_file', { path, contents: file.contents });
  return path;
}
