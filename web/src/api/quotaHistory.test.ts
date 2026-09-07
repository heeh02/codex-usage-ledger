import { afterEach, expect, it, vi } from 'vitest';
import { fetchQuotaHistory, validateQuotaHistory } from './quotaHistory';
import { mockQuotaHistory } from './quotaHistoryMock';

afterEach(() => vi.unstubAllGlobals());
it('validates all-account and single-account pages without mixing views', () => {
  const first = mockQuotaHistory({ account: 'all' });
  const second = mockQuotaHistory({ account: 'all', cursor: first.next });
  const third = mockQuotaHistory({ account: 'all', cursor: second.next });
  expect([first.intervals.length, second.intervals.length, third.intervals.length]).toEqual([20, 20, 5]);
  expect(third.next).toBeNull();
  expect(new Set([...first.intervals, ...second.intervals, ...third.intervals].map(row => row.id)).size).toBe(45);
  expect(() => validateQuotaHistory(first, { account: 'acct-personal' })).toThrow();
  expect(mockQuotaHistory({ account: 'acct-personal' }).intervals.every(row => row.accountId === 'acct-personal')).toBe(true);
  expect(() => validateQuotaHistory({ ...second, view: { ...second.view, revision: 2 } }, { account: 'all', cursor: first.next })).toThrow();
});
it('rejects duplicate, unordered, invalid-count and incomplete-range evidence', () => {
  const page = mockQuotaHistory({ account: 'all' });
  for (const intervals of [
    [page.intervals[0], page.intervals[0]], [...page.intervals].reverse(),
    [{ ...page.intervals[0], sampleCount: 0 }],
    [{ ...page.intervals[0], tokenSampleEnd: null }],
    [{ ...page.intervals[0], lastUsedPercent: 101 }],
  ]) expect(() => validateQuotaHistory({ ...page, intervals }, { account: 'all' })).toThrow();
  expect(() => validateQuotaHistory({ ...page, indexReady: false }, { account: 'all' })).toThrow();
  const pending = { ...page, indexReady: false, intervals: [], next: null };
  expect(validateQuotaHistory(pending, { account: 'all' }).indexReady).toBe(false);
});
it('does not replace a failed real request with mock history', async () => {
  const fetch = vi.fn(async (_url: RequestInfo | URL) => ({ ok: false, status: 503, json: async () => ({ code: 'request_failed' }) }));
  vi.stubGlobal('fetch', fetch);
  await expect(fetchQuotaHistory({ account: 'acct-personal' })).rejects.toThrow();
  expect(fetch).toHaveBeenCalledOnce();
  expect(fetch.mock.calls[0]?.[0]).toContain('/v1/quota-history?account=acct-personal');
});
