import { validRequestUsage, normalizedEvidenceSelection, type RequestEvidenceQuery } from './requestEvidence';
import type { TurnEvidenceResponse } from './turn-evidence.generated';

export type TurnEvidenceQuery = Omit<RequestEvidenceQuery, 'after'> & { offset?: number };

export async function fetchTurnEvidence(query: TurnEvidenceQuery, signal?: AbortSignal): Promise<TurnEvidenceResponse> {
  const account = normalizedEvidenceSelection(query.account);
  const model = normalizedEvidenceSelection(query.model);
  const params = new URLSearchParams({ threadId: query.threadId, start: query.start,
    end: query.end, offset: String(query.offset ?? 0), limit: String(query.limit ?? 100) });
  if (account) params.set('account', account);
  if (model) params.set('model', model);
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/turn-evidence?${params}`, { signal, headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error(`Turn evidence returned HTTP ${response.status}`);
  const value = await response.json() as TurnEvidenceResponse;
  if (value.scope !== 'thread_own_retained_turns' || value.threadId !== query.threadId
    || Date.parse(value.start) !== Date.parse(query.start) || Date.parse(value.end) !== Date.parse(query.end)
    || value.selectedAccount !== account
    || value.selectedModel !== model
    || !Array.isArray(value.rows)) throw new Error('Turn evidence response scope mismatch');
  if (value.rows.some(row => !Number.isSafeInteger(row.requestCount) || row.requestCount < 1
    || !Number.isSafeInteger(row.confirmedRequestCount) || row.confirmedRequestCount < 0
    || row.confirmedRequestCount > row.requestCount
    || (row.confirmedRequestCount === 0 ? row.confirmedUsage !== null : !validRequestUsage(row.confirmedUsage)))) {
    throw new Error('Turn evidence contains invalid counts or dimensions');
  }
  if (value.nextOffset !== null && (!Number.isSafeInteger(value.nextOffset)
    || value.nextOffset <= (query.offset ?? 0) || value.rows.length === 0)) {
    throw new Error('Turn evidence contains invalid pagination');
  }
  if (value.rows.some(row => typeof row.groupId !== 'string' || row.groupId.length === 0)
    || new Set(value.rows.map(row => row.groupId)).size !== value.rows.length) {
    throw new Error('Turn evidence contains duplicate or missing groups');
  }
  return value;
}
