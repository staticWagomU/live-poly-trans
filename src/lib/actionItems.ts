import { formatTimestampMs } from './recordings';

export type ActionItem = {
  id: string;
  text: string;
  assignee: string | null;
  timestampMs: number | null;
  sourceIndex: number | null;
  done: boolean;
};

export type ActionItemsParseResult =
  | { ok: true; items: ActionItem[] }
  | { ok: false; error: string };

export type ActionItemSourceWindow = {
  id: string;
  startMessageIndex: number;
  endMessageIndex?: number;
};

export function actionItemKey(item: Pick<ActionItem, 'text' | 'assignee'>): string {
  return `${item.text.trim().toLowerCase()}|${item.assignee?.trim().toLowerCase() ?? ''}`;
}

export function actionItemId(item: Pick<ActionItem, 'text' | 'timestampMs' | 'sourceIndex'>): string {
  return `action-${item.sourceIndex ?? 'x'}-${item.timestampMs ?? 'x'}-${item.text.trim()}`;
}

export function parseActionItemsJson(raw: string): ActionItemsParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: 'アクションアイテムJSONを読み込めませんでした。' };
  }

  const source = Array.isArray(parsed)
    ? parsed
    : typeof parsed === 'object' && parsed !== null && Array.isArray((parsed as { actions?: unknown }).actions)
      ? (parsed as { actions: unknown[] }).actions
      : null;
  if (source === null) {
    return { ok: false, error: 'アクションアイテムJSONは actions 配列を含む必要があります。' };
  }

  const items: ActionItem[] = [];
  for (const rawItem of source) {
    if (typeof rawItem !== 'object' || rawItem === null) {
      continue;
    }
    const candidate = rawItem as Record<string, unknown>;
    if (typeof candidate.text !== 'string' || candidate.text.trim() === '') {
      continue;
    }

    const item = {
      text: candidate.text.trim(),
      assignee: typeof candidate.assignee === 'string' && candidate.assignee.trim() !== ''
        ? candidate.assignee.trim()
        : null,
      timestampMs: typeof candidate.timestampMs === 'number' && Number.isFinite(candidate.timestampMs)
        ? candidate.timestampMs
        : null,
      sourceIndex: typeof candidate.sourceIndex === 'number' && Number.isInteger(candidate.sourceIndex)
        ? candidate.sourceIndex
        : null,
      done: false
    };
    items.push({ ...item, id: actionItemId(item) });
  }

  return { ok: true, items };
}

export function mergeActionItems(current: ActionItem[], incoming: ActionItem[]): ActionItem[] {
  const merged = [...current];
  const existingKeys = new Set(current.map(actionItemKey));

  for (const item of incoming) {
    const key = actionItemKey(item);
    if (existingKeys.has(key)) {
      continue;
    }
    existingKeys.add(key);
    merged.push(item);
  }

  return merged;
}

export function rebaseActionSourceIndexes(items: ActionItem[], offset: number): ActionItem[] {
  if (offset === 0) {
    return items;
  }

  return items.map((item) => ({
    ...item,
    sourceIndex: item.sourceIndex === null ? null : item.sourceIndex + offset
  }));
}

export function actionItemsForMarkdown(items: ActionItem[]): string[] {
  return items.map((item) => {
    const metadata = [
      item.assignee,
      item.timestampMs === null ? null : formatTimestampMs(item.timestampMs)
    ].filter((value): value is string => value !== null && value !== '');
    const suffix = metadata.length === 0 ? '' : ` (${metadata.join(', ')})`;
    return `[${item.done ? 'x' : ' '}] ${item.text}${suffix}`;
  });
}

export function actionItemsForRecordingWindow(
  items: ActionItem[],
  window: ActionItemSourceWindow
): ActionItem[] {
  return items
    .filter(
      (item) =>
        item.sourceIndex !== null &&
        item.sourceIndex >= window.startMessageIndex &&
        (window.endMessageIndex === undefined || item.sourceIndex < window.endMessageIndex)
    )
    .map((item) => ({
      ...item,
      sourceIndex: item.sourceIndex === null ? null : item.sourceIndex - window.startMessageIndex
    }));
}
