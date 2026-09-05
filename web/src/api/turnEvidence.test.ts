import { afterEach, expect, it, vi } from 'vitest';
import { fetchTurnEvidence } from './turnEvidence';
afterEach(() => vi.unstubAllGlobals());

it('preserves unknown-only turn usage and rejects invalid confirmed counts', async () => {
  const query = { threadId: 'thread', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' };
  const result = { ...query, scope: 'thread_own_retained_turns', selectedAccount: null,
    selectedModel: null, historyComplete: false, nextOffset: null,
    rows: [{ groupId: 'request:one', turnId: null, firstAt: query.start, lastAt: query.start,
      requestCount: 1, confirmedRequestCount: 0, confirmedUsage: null }] };
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => result }));
  expect((await fetchTurnEvidence(query)).rows[0].confirmedUsage).toBeNull();
  result.rows[0].confirmedRequestCount = 2;
  await expect(fetchTurnEvidence(query)).rejects.toThrow('invalid counts');
});
