import { describe, expect, it } from 'vitest';
import { createLatestRequestGuard } from './latestRequest';

describe('createLatestRequestGuard', () => {
  it('marks an earlier request stale when a newer request begins', () => {
    const guard = createLatestRequestGuard();
    const firstIsLatest = guard.begin();
    const secondIsLatest = guard.begin();

    expect(firstIsLatest()).toBe(false);
    expect(secondIsLatest()).toBe(true);
  });

  it('invalidates the current request without starting another one', () => {
    const guard = createLatestRequestGuard();
    const isLatest = guard.begin();

    guard.invalidate();

    expect(isLatest()).toBe(false);
  });
});
