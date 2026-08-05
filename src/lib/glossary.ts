/// Glossary (custom dictionary) core: rules that correct recurring speech
/// recognition mistakes ("誤 → 正"). Application is strictly non-destructive —
/// stored transcripts and events keep the raw recognition output; rules run
/// only at display and text-export time.
import type { TranscriptEntry } from './export/types';

export type GlossaryRule = {
  from: string;
  to: string;
  matchType: 'text' | 'regex';
  enabled: boolean;
};

export type GlossaryImportResult =
  | { ok: true; rules: GlossaryRule[] }
  | { ok: false; error: string };

function isActive(rule: GlossaryRule): boolean {
  return rule.enabled && rule.from !== '';
}

/// Applies enabled rules to `text` in array order, so later rules see the
/// output of earlier ones. 'text' rules replace every occurrence literally;
/// 'regex' rules use JS RegExp semantics (groups and $1 backreferences work).
/// Deterministic and total: an invalid regex pattern skips its rule silently,
/// so user-entered patterns can never break rendering or exports.
export function applyGlossary(text: string, rules: GlossaryRule[]): string {
  let result = text;

  for (const rule of rules) {
    if (!isActive(rule)) {
      continue;
    }

    if (rule.matchType === 'regex') {
      try {
        result = result.replace(new RegExp(rule.from, 'g'), rule.to);
      } catch {
        // Invalid user pattern: ignore the rule, keep the text.
      }
    } else {
      // split/join keeps both sides literal ($ in `to` is not expanded).
      result = result.split(rule.from).join(rule.to);
    }
  }

  return result;
}

/// applyGlossary over a whole transcript's `text` fields. Translations stay
/// as-is: the glossary corrects recognition errors in the source text, while
/// translations are engine output. Returns the input array unchanged when no
/// rule is active, so no-glossary callers pay nothing.
export function applyGlossaryToEntries<T extends TranscriptEntry>(
  entries: T[],
  rules: GlossaryRule[]
): T[] {
  if (!rules.some(isActive)) {
    return entries;
  }

  return entries.map((entry) => ({ ...entry, text: applyGlossary(entry.text, rules) }));
}

/// Tolerant deserialization for settings storage: null, malformed JSON, or a
/// non-array yield []; non-conforming items are dropped; unknown matchType
/// falls back to 'text' and a missing enabled flag means the rule is on.
export function parseGlossaryRules(raw: string | null): GlossaryRule[] {
  if (raw === null) {
    return [];
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }

  if (!Array.isArray(parsed)) {
    return [];
  }

  const rules: GlossaryRule[] = [];
  for (const item of parsed) {
    if (typeof item !== 'object' || item === null) {
      continue;
    }
    const candidate = item as Record<string, unknown>;
    if (typeof candidate.from !== 'string' || typeof candidate.to !== 'string') {
      continue;
    }
    rules.push({
      from: candidate.from,
      to: candidate.to,
      matchType: candidate.matchType === 'regex' ? 'regex' : 'text',
      enabled: candidate.enabled !== false
    });
  }

  return rules;
}

export function serializeGlossaryRules(rules: GlossaryRule[]): string {
  return JSON.stringify(rules);
}

export function importGlossaryJson(raw: string): GlossaryImportResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: '用語集JSONを読み込めませんでした。' };
  }

  if (!Array.isArray(parsed)) {
    return { ok: false, error: '用語集JSONは配列である必要があります。' };
  }

  return { ok: true, rules: parseGlossaryRules(raw) };
}
