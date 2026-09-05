import type { RequestEvidenceCursor, RequestEvidenceResponse } from './request-evidence.generated';

function validRequestUsage(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const usage = value as Record<string, unknown>;
  const fields = ['input', 'cached', 'cacheWrite', 'cacheWriteObservedInput', 'uncached', 'output', 'reasoning', 'total'];
  if (fields.some(field => typeof usage[field] !== 'number' || !Number.isSafeInteger(usage[field]) || Number(usage[field]) < 0)) return false;
  const n = (field: string) => Number(usage[field]);
  return n('input') + n('output') === n('total')
    && n('cached') + n('cacheWrite') + n('uncached') === n('input')
    && n('reasoning') <= n('output') && n('cacheWriteObservedInput') <= n('input')
    && typeof usage.cacheWriteCoverage === 'number' && Number.isFinite(usage.cacheWriteCoverage)
    && usage.cacheWriteCoverage >= 0 && usage.cacheWriteCoverage <= 1;
}

export interface RequestEvidenceQuery {
  account?: string;
  model?: string;
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
  if (query.account && query.account !== 'all') params.set('account', query.account);
  if (query.model && query.model !== 'all') params.set('model', query.model);
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/request-evidence?${params}`, {
    signal, headers: { Accept: 'application/json' },
  });
  if (!response.ok) throw new Error(`Request evidence returned HTTP ${response.status}`);
  const value = await response.json() as RequestEvidenceResponse;
  if (value.scope !== 'thread_own_retained_observations'
    || value.selectionAttribution !== 'current_ledger'
    || value.selectedAccount !== (query.account && query.account !== 'all' ? query.account : null)
    || value.selectedModel !== (query.model && query.model !== 'all' ? query.model : null)
    || value.attribution !== 'ingest_observed' || value.threadId !== query.threadId
    || Date.parse(value.start) !== Date.parse(query.start)
    || Date.parse(value.end) !== Date.parse(query.end)
    || !Array.isArray(value.rows)) {
    throw new Error('Request evidence response scope mismatch');
  }
  if (value.rows.some(row => !validRequestUsage(row?.usage))) {
    throw new Error('Request evidence contains invalid token dimensions');
  }
  return value;
}
