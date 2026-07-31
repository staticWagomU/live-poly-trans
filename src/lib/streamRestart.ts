export const maxRestartAttempts = 3;

/// A restarted stream that stayed alive this long is considered recovered:
/// the next crash gets a fresh attempt budget instead of consuming the
/// remainder. Keeps crash loops bounded while letting an 8-hour session
/// survive any number of isolated crashes.
export const restartAttemptResetMs = 60_000;

/**
 * Attempt numbers still available for restarting a crashed stream, given how
 * many attempts the previous crash burst already used and how long the
 * stream ran before this crash (null when unknown).
 */
export function remainingRestartAttempts(
  previousAttempts: number,
  uptimeMs: number | null
): number[] {
  const usedAttempts =
    uptimeMs !== null && uptimeMs >= restartAttemptResetMs ? 0 : previousAttempts;

  const attempts: number[] = [];
  for (let attempt = usedAttempts + 1; attempt <= maxRestartAttempts; attempt++) {
    attempts.push(attempt);
  }
  return attempts;
}

export function restartBackoffMs(attempt: number): number {
  return attempt * 1000;
}
