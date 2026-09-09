import { expect, it } from 'vitest';
import { advanceRequestPage, firstRequestPage, previousRequestPage } from './requestPaging';

it('returns through visited cursor pages without skipping equal-time requests', () => {
  const first = firstRequestPage();
  const a = { afterTime: '2026-08-01T00:00:00.000Z', afterId: 'a' };
  const b = { ...a, afterId: 'b' };
  const second = advanceRequestPage(first, a);
  const third = advanceRequestPage(second, b);
  expect(previousRequestPage(third)).toEqual(second);
  expect(previousRequestPage(second)).toEqual(first);
  expect(previousRequestPage(first)).toBe(first);
  expect(advanceRequestPage(third, null)).toBe(third);
  expect(firstRequestPage()).toEqual({ cursor: null, previous: [] });
});
