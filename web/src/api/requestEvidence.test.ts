import { afterEach, expect, it, vi } from 'vitest';
import { fetchRequestEvidence } from './requestEvidence';

afterEach(() => vi.unstubAllGlobals());
const query = { threadId: 'thread & one', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' };

it('encodes scope and cursor and propagates cancellation', async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({
    scope: 'thread_own_retained_observations', attribution: 'ingest_observed',
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
    threadId: 'different', rows: [],
  }) }));
  await expect(fetchRequestEvidence(query)).rejects.toThrow('scope mismatch');
});
