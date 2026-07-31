export type Cleanup = () => void;

export type AsyncCleanupRegistry = {
  add(cleanup: Cleanup | Promise<Cleanup>): Promise<void>;
  dispose(): void;
  isDisposed(): boolean;
};

export function createAsyncCleanupRegistry(
  onCleanupError?: (error: unknown) => void
): AsyncCleanupRegistry {
  const cleanups = new Set<Cleanup>();
  let disposed = false;

  function runCleanup(cleanup: Cleanup) {
    try {
      cleanup();
    } catch (error) {
      onCleanupError?.(error);
    }
  }

  return {
    async add(cleanupOrPromise) {
      const cleanup = await cleanupOrPromise;
      if (disposed) {
        runCleanup(cleanup);
        return;
      }
      cleanups.add(cleanup);
    },
    dispose() {
      if (disposed) {
        return;
      }

      disposed = true;
      for (const cleanup of cleanups) {
        runCleanup(cleanup);
      }
      cleanups.clear();
    },
    isDisposed() {
      return disposed;
    }
  };
}
