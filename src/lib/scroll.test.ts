import { describe, expect, it } from 'vitest';
import { isScrolledToBottom } from './scroll';

describe('isScrolledToBottom', () => {
  const testCases: Array<{
    name: string;
    scrollTop: number;
    clientHeight: number;
    scrollHeight: number;
    expected: boolean;
  }> = [
    {
      name: 'treats positions near the bottom as already pinned to the latest transcript',
      scrollTop: 640,
      clientHeight: 360,
      scrollHeight: 1030,
      expected: true
    },
    {
      name: 'treats positions far from the bottom as manually browsing older transcript lines',
      scrollTop: 540,
      clientHeight: 360,
      scrollHeight: 1030,
      expected: false
    }
  ];

  it.each(testCases)('$name', ({ scrollTop, clientHeight, scrollHeight, expected }) => {
    expect(
      isScrolledToBottom({
        scrollTop,
        clientHeight,
        scrollHeight
      })
    ).toBe(expected);
  });
});
