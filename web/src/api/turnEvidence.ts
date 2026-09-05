import { validRequestUsage, type RequestEvidenceQuery } from './requestEvidence';
import type { TurnEvidenceResponse } from './turn-evidence.generated';

export type TurnEvidenceQuery = Omit<RequestEvidenceQuery, 'after'> & { offset?: number };

export async function fetchTurnEvidence(query: TurnEvidenceQuery, signal?: AbortSignal): Promise<TurnEvidenceResponse> {
  const params = new URLSearchParams({ threadId: query.threadId, start: query.start,
    end: query.end, offset: String(query.offset ?? 0), limit: String(query.limit ?? 100) });
  if (query.account && query.account !== 'all') params.set('account', query.account);
  if (query.model && query.model !== 'all') params.set('model', query.model);
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/turn-evidence?${params}`, { signal, headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error(`Turn evidence returned HTTP ${response.status}`);
  const value = await response.json() as TurnEvidenceResponse;
  if (value.scope !== 'thread_own_retained_turns' || value.threadId !== query.threadId
    || Date.parse(value.start) !== Date.parse(query.start) || Date.parse(value.end) !== Date.parse(query.end)
    || value.selectedAccount !== (query.account && query.account !== 'all' ? query.account : null)
    || value.selectedModel !== (query.model && query.model !== 'all' ? query.model : null)
    || !Array.isArray(value.rows)) throw new Error('Turn evidence response scope mismatch');
  if (value.rows.some(row => !Number.isSafeInteger(row.requestCount) || row.requestCount < 1
    || !Number.isSafeInteger(row.confirmedRequestCount) || row.confirmedRequestCount < 0
    || row.confirmedRequestCount > row.requestCount
    || (row.confirmedRequestCount === 0 ? row.confirmedUsage !== null : !validRequestUsage(row.confirmedUsage)))) {
    throw new Error('Turn evidence contains invalid counts or dimensions');
  }
  return value;
}
