import { afterEach, expect, it, vi } from 'vitest';
import { fetchRequestEvidence } from './requestEvidence';

afterEach(() => vi.unstubAllGlobals());
const query = { threadId: 'thread & one', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' };

it('encodes scope and cursor and propagates cancellation', async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({
    scope: 'thread_own_retained_observations', attribution: 'ingest_observed',
    selectionAttribution: 'current_ledger', selectedAccount: null, selectedModel: null,
    threadId: query.threadId, start: query.start, end: query.end,
    rows: [], next: null, historyComplete: false,
  }) });
  vi.stubGlobal('fetch', fetch);
  const controller = new AbortController();
  const result = await fetchRequestEvidence({ ...query, after: { afterId: 'a&b', afterTime: query.start } }, controller.signal);
  const params = new URL(String(fetch.mock.calls[0][0]), 'http://localhost').searchParams;
  expect(params.get('threadId')).toBe(query.threadId);
  expect(params.get('afterId')).toBe('a&b');
  expect(fetch.mock.calls[0][1].signal).toBe(controller.signal);
  expect(result.historyComplete).toBe(false);
});

it('does not turn server errors or wrong-scope data into an empty page', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }));
  await expect(fetchRequestEvidence(query)).rejects.toThrow('HTTP 500');
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({
    scope: 'thread_own_retained_observations', attribution: 'ingest_observed',
    selectionAttribution: 'current_ledger', selectedAccount: null, selectedModel: null,
    threadId: 'different', rows: [],
  }) }));
  await expect(fetchRequestEvidence(query)).rejects.toThrow('scope mismatch');
});

it('rejects nonconserved and unsafe token numbers instead of displaying them', async () => {
  const usage = { input: 100, cached: 40, cacheWrite: 10, cacheWriteObservedInput: 100,
    uncached: 50, output: 20, reasoning: 5, total: 120, cacheWriteCoverage: 1 };
  const response = (next: object) => ({ ok: true, json: async () => ({
    scope: 'thread_own_retained_observations', attribution: 'ingest_observed',
    selectionAttribution: 'current_ledger', selectedAccount: null, selectedModel: null,
    threadId: query.threadId, start: query.start, end: query.end, rows: [{ usage: next }],
  }) });
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response(usage)));
  await expect(fetchRequestEvidence(query)).resolves.toBeDefined();
  for (const invalid of [{ ...usage, total: 121 }, { ...usage, cached: -1 },
    { ...usage, input: Number.MAX_SAFE_INTEGER + 1 }, { ...usage, reasoning: 21 },
    { ...usage, cacheWrite: 20 }, { ...usage, cacheWriteCoverage: 2 }]) {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response(invalid)));
    await expect(fetchRequestEvidence(query)).rejects.toThrow('invalid token dimensions');
  }
});
