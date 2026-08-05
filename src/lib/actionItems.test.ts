import { describe, expect, it } from 'vitest';
import {
  actionItemsForMarkdown,
  mergeActionItems,
  parseActionItemsJson,
  rebaseActionSourceIndexes,
  type ActionItem
} from './actionItems';

function item(overrides: Partial<ActionItem> = {}): ActionItem {
  return {
    id: 'a-1',
    text: '告知文を書く',
    assignee: '自分',
    timestampMs: 63_000,
    sourceIndex: 3,
    done: false,
    ...overrides
  };
}

describe('parseActionItemsJson', () => {
  it('parses an actions array returned by the model', () => {
    const result = parseActionItemsJson(
      JSON.stringify({
        actions: [
          {
            text: '日程を調整する',
            assignee: '自分',
            timestampMs: 12_000,
            sourceIndex: 2
          }
        ]
      })
    );

    expect(result).toEqual({
      ok: true,
      items: [
        {
          id: 'action-2-12000-日程を調整する',
          text: '日程を調整する',
          assignee: '自分',
          timestampMs: 12_000,
          sourceIndex: 2,
          done: false
        }
      ]
    });
  });

  it('returns an error for broken JSON instead of throwing', () => {
    expect(parseActionItemsJson('{broken')).toEqual({
      ok: false,
      error: 'アクションアイテムJSONを読み込めませんでした。'
    });
  });
});

describe('mergeActionItems', () => {
  it('deduplicates by text and assignee while preserving checked state', () => {
    const current = [item({ id: 'old', done: true })];
    const incoming = [item({ id: 'new', done: false })];

    expect(mergeActionItems(current, incoming)).toEqual([item({ id: 'old', done: true })]);
  });

  it('appends new actions in arrival order', () => {
    expect(
      mergeActionItems([item()], [
        item({ id: 'a-2', text: 'QA を依頼する', assignee: '佐藤', sourceIndex: 4 })
      ]).map((entry) => entry.text)
    ).toEqual(['告知文を書く', 'QA を依頼する']);
  });
});

describe('rebaseActionSourceIndexes', () => {
  it('adds the bounded transcript offset to source indexes', () => {
    expect(rebaseActionSourceIndexes([item({ sourceIndex: 2 })], 8)).toEqual([
      item({ sourceIndex: 10 })
    ]);
  });

  it('leaves missing source indexes untouched', () => {
    expect(rebaseActionSourceIndexes([item({ sourceIndex: null })], 8)).toEqual([
      item({ sourceIndex: null })
    ]);
  });
});

describe('actionItemsForMarkdown', () => {
  it('formats action items as markdown checklist lines with metadata', () => {
    expect(actionItemsForMarkdown([item({ done: true })])).toEqual([
      '[x] 告知文を書く (自分, 1:03)'
    ]);
  });

  it('omits missing assignee and timestamp metadata', () => {
    expect(actionItemsForMarkdown([item({ assignee: null, timestampMs: null })])).toEqual([
      '[ ] 告知文を書く'
    ]);
  });
});
