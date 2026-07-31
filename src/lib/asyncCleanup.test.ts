import { describe, expect, it, vi } from 'vitest';
import { createAsyncCleanupRegistry } from './asyncCleanup';

describe('createAsyncCleanupRegistry', () => {
  it('runs registered cleanup functions exactly once when disposed', async () => {
    const cleanup = vi.fn();
    const registry = createAsyncCleanupRegistry();

    await registry.add(Promise.resolve(cleanup));
    registry.dispose();
    registry.dispose();

    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it('cleans up work that finishes after the registry was disposed', async () => {
    const cleanup = vi.fn();
    let resolveCleanup: ((cleanup: () => void) => void) | undefined;
    const pendingCleanup = new Promise<() => void>((resolve) => {
      resolveCleanup = resolve;
    });
    const registry = createAsyncCleanupRegistry();

    const registration = registry.add(pendingCleanup);
    registry.dispose();
    resolveCleanup?.(cleanup);
    await registration;

    expect(cleanup).toHaveBeenCalledTimes(1);
  });
});
