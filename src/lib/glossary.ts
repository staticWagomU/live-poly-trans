export type GlossaryRule = { from: string; to: string };

const KEY = 'lpt-mimi-glossary';

export function loadGlossary(): GlossaryRule[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? '[]');
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (rule): rule is GlossaryRule =>
        typeof rule?.from === 'string' &&
        typeof rule?.to === 'string' &&
        rule.from.trim().length > 0 &&
        rule.to.trim().length > 0 &&
        [...rule.from].length <= 80 &&
        [...rule.to].length <= 80
    ).slice(0, 50);
  } catch {
    return [];
  }
}

export function saveGlossary(rules: GlossaryRule[]): void {
  localStorage.setItem(KEY, JSON.stringify(rules));
}
