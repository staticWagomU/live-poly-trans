export type LatestRequestGuard = {
  begin(): () => boolean;
  invalidate(): void;
};

export function createLatestRequestGuard(): LatestRequestGuard {
  let latestRequestId = 0;

  return {
    begin() {
      const requestId = ++latestRequestId;
      return () => requestId === latestRequestId;
    },
    invalidate() {
      latestRequestId += 1;
    }
  };
}
