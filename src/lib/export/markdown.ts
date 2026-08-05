import { formatTimestampMs } from '../recordings';
import { entryLines } from './entryLines';
import { withResolvedTimings } from './timing';
import type { ExportOptions, TranscriptEntry } from './types';

/// Document metadata for the Markdown export. summary/actionItems are the
/// Phase 6 extension points (AI-generated minutes); a section is rendered
/// only when its field is present.
export type MarkdownMeta = {
  title?: string;
  dateLabel?: string;
  participants?: string[];
  summary?: string;
  actionItems?: string[];
};

/// Markdown minutes document: `# title` heading, optional 日時/参加者 meta
/// lines, then one list item per entry with a `[H:MM:SS]` timestamp (start
/// times resolved like the subtitle exports; translation lines continue the
/// list item indented). Optional 要約 / アクションアイテム sections follow.
/// Ends with a single trailing newline.
export function toMarkdown(
  entries: TranscriptEntry[],
  meta: MarkdownMeta,
  opts: ExportOptions
): string {
  const blocks: string[] = [`# ${meta.title ?? '文字起こし'}`];

  const metaLines: string[] = [];
  if (meta.dateLabel !== undefined) {
    metaLines.push(`- 日時: ${meta.dateLabel}`);
  }
  if (meta.participants !== undefined && meta.participants.length > 0) {
    metaLines.push(`- 参加者: ${meta.participants.join(', ')}`);
  }
  if (metaLines.length > 0) {
    blocks.push(metaLines.join('\n'));
  }

  const resolved = withResolvedTimings(entries);
  if (resolved.length > 0) {
    blocks.push(
      resolved
        .map((entry) => {
          const [first, ...rest] = entryLines(entry, opts);
          return [
            `- [${formatTimestampMs(entry.startMs)}] ${first}`,
            ...rest.map((line) => `  ${line}`)
          ].join('\n');
        })
        .join('\n')
    );
  }

  if (meta.summary !== undefined) {
    blocks.push(`## 要約\n\n${meta.summary}`);
  }
  if (meta.actionItems !== undefined && meta.actionItems.length > 0) {
    blocks.push(`## アクションアイテム\n\n${meta.actionItems.map((item) => `- ${item}`).join('\n')}`);
  }

  return `${blocks.join('\n\n')}\n`;
}
