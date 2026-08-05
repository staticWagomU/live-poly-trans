import { sanitizeFileName } from './export/saveTextExport';

export const DEFAULT_FILE_NAME_TEMPLATE = '{date} {time} {title}';

export type FileNameContext = {
  date: Date;
  title: string;
  lang: string;
};

function pad(value: number): string {
  return String(value).padStart(2, '0');
}

export function fileNameDate(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function fileNameTime(date: Date): string {
  return `${pad(date.getHours())}${pad(date.getMinutes())}`;
}

export function renderFileName(template: string, ctx: FileNameContext): string {
  const rendered = template
    .replaceAll('{date}', fileNameDate(ctx.date))
    .replaceAll('{time}', fileNameTime(ctx.date))
    .replaceAll('{title}', ctx.title)
    .replaceAll('{lang}', ctx.lang);

  return sanitizeFileName(rendered);
}

export function markdownExportPath(
  directory: string,
  template: string,
  ctx: FileNameContext
): string {
  const base = directory.endsWith('/') ? directory.slice(0, -1) : directory;
  return `${base}/${renderFileName(template, ctx)}.md`;
}
