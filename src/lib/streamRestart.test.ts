import { describe, expect, it } from 'vitest';
import {
  maxRestartAttempts,
  remainingRestartAttempts,
  restartAttemptResetMs,
  restartBackoffMs
} from './streamRestart';

describe('remainingRestartAttempts', () => {
  it('gives a fresh stream the full attempt budget', () => {
    expect(remainingRestartAttempts(0, null)).toEqual([1, 2, 3]);
  });

  it('continues counting across quick consecutive crashes', () => {
    expect(remainingRestartAttempts(2, 5_000)).toEqual([3]);
  });

  it('gives up after the budget is exhausted by a crash loop', () => {
    expect(remainingRestartAttempts(maxRestartAttempts, 5_000)).toEqual([]);
  });

  it('resets the budget when the restarted stream survived long enough', () => {
    // A stream that ran for an hour before crashing is not in a crash loop;
    // a cumulative counter would make the 4th crash of an 8-hour meeting
    // give up permanently.
    expect(remainingRestartAttempts(maxRestartAttempts, restartAttemptResetMs)).toEqual([1, 2, 3]);
  });

  it('keeps counting when the restarted stream died before the reset window', () => {
    expect(remainingRestartAttempts(1, restartAttemptResetMs - 1)).toEqual([2, 3]);
  });
});

describe('restartBackoffMs', () => {
  it('backs off linearly per attempt', () => {
    expect(restartBackoffMs(1)).toBe(1000);
    expect(restartBackoffMs(3)).toBe(3000);
  });
});
