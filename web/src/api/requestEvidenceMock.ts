import type { RequestEvidenceQuery } from './requestEvidence';
import type { RequestEvidenceResponse, RequestEvidenceRow } from './request-evidence.generated';

/** Explicit synthetic requests for UI acceptance, never real attribution. */
export function mockRequestEvidence(query: RequestEvidenceQuery): RequestEvidenceResponse {
  const start = Date.parse(query.start), end = Date.parse(query.end);
  const rows: RequestEvidenceRow[] = Array.from({ length: 205 }, (_, index): RequestEvidenceRow => ({
    id: `demo-request-${String(index).padStart(4, '0')}`,
    at: new Date(start + index * 1000).toISOString(),
    turnId: index % 10 === 0 ? null : `demo-turn-${Math.floor(index / 3)}`,
    model: 'demo-model', observedAccount: null, observedProject: null,
    accountConfidence: 'unknown', projectConfidence: 'unknown', quality: 'confirmed',
    usage: { input: 100, cached: 40, cacheWrite: 10, cacheWriteObservedInput: 100,
      cacheWriteCoverage: 1, uncached: 50, output: 20, reasoning: 5, total: 120 },
  })).filter(row => Date.parse(row.at) < end
    && (!query.model || query.model === 'all' || row.model === query.model)
    && (!query.account || query.account === 'all'));
  const after = query.after;
  const remaining = rows.filter(row => !after || row.at > after.afterTime ||
    (row.at === after.afterTime && row.id > after.afterId));
  const limit = query.limit ?? 100;
  const page = remaining.slice(0, limit);
  const last = page.at(-1);
  return { scope: 'thread_own_retained_observations', attribution: 'ingest_observed',
    selectionAttribution: 'current_ledger',
    selectedAccount: query.account && query.account !== 'all' ? query.account : null,
    selectedModel: query.model && query.model !== 'all' ? query.model : null,
    historyComplete: false, threadId: query.threadId, start: query.start, end: query.end,
    rows: page, next: remaining.length > limit && last ? { afterTime: last.at, afterId: last.id } : null };
}
