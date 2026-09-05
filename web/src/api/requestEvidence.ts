import type { RequestEvidenceCursor, RequestEvidenceResponse } from './request-evidence.generated';

export interface RequestEvidenceQuery {
  threadId: string;
  start: string;
  end: string;
  after?: RequestEvidenceCursor | null;
  limit?: number;
}

export async function fetchRequestEvidence(
  query: RequestEvidenceQuery,
  signal?: AbortSignal,
): Promise<RequestEvidenceResponse> {
  const params = new URLSearchParams({
    threadId: query.threadId, start: query.start, end: query.end,
    limit: String(query.limit ?? 100),
  });
  if (query.after) {
    params.set('afterTime', query.after.afterTime);
    params.set('afterId', query.after.afterId);
  }
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/request-evidence?${params}`, {
    signal, headers: { Accept: 'application/json' },
  });
  if (!response.ok) throw new Error(`Request evidence returned HTTP ${response.status}`);
  const value = await response.json() as RequestEvidenceResponse;
  if (value.scope !== 'thread_own_retained_observations'
    || value.attribution !== 'ingest_observed' || value.threadId !== query.threadId
    || Date.parse(value.start) !== Date.parse(query.start)
    || Date.parse(value.end) !== Date.parse(query.end)
    || !Array.isArray(value.rows)) {
    throw new Error('Request evidence response scope mismatch');
  }
  return value;
}
