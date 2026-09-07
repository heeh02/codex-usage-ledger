import type { QuotaIntervalUsageResponse } from './quota-interval-usage.generated';
import type { QuotaHistoryCursor, QuotaHistoryInterval } from './quotaHistory';
import { validRequestUsage } from './requestEvidence';
import { ledgerResponseError } from './errors';
export type { QuotaIntervalUsageResponse } from './quota-interval-usage.generated';
const fields = ['input', 'cached', 'cacheWrite', 'cacheWriteObservedInput', 'output', 'reasoning', 'total', 'uncached'] as const;

export function validateQuotaIntervalUsage(value: QuotaIntervalUsageResponse, row: QuotaHistoryInterval, selection: QuotaHistoryCursor): QuotaIntervalUsageResponse {
  const fail = () => { throw new Error('Quota interval usage scope or conservation failed'); };
  if (!value || value.scope !== 'quota_interval_local_evidence_v1' || value.intervalId !== row.id || value.accountId !== row.accountId
    || !value.quotaView || value.quotaView.instance !== selection.view.instance || value.quotaView.revision !== selection.view.revision
    || value.quotaView.account !== selection.view.account || value.quotaView.asOf !== selection.view.asOf
    || value.start !== row.tokenSampleStart || value.end !== row.tokenSampleEnd || !Number.isFinite(Date.parse(value.observedAt))
    || value.coverageComplete !== false || value.poolAttribution !== false || !Array.isArray(value.models) || !Array.isArray(value.projects)) fail();
  if (value.status !== 'available') {
    if (!['no_safe_interval', 'no_evidence', 'source_overlap_review', 'pending'].includes(value.status)
      || value.usage !== null || value.events !== null || value.models.length || value.projects.length) fail();
    return value;
  }
  if (value.start === null || value.end === null || !validRequestUsage(value.usage) || !Number.isSafeInteger(value.events) || value.events === null || value.events < 1) fail();
  for (const groups of [value.models, value.projects]) {
    const ids = new Set<string>();
    for (const group of groups) {
      if (!group || (group.id !== null && typeof group.id !== 'string') || ids.has(JSON.stringify(group.id))
        || !validRequestUsage(group.usage) || !Number.isSafeInteger(group.events) || group.events < 1) fail();
      ids.add(JSON.stringify(group.id));
    }
    if (groups.reduce((sum, group) => sum + group.events, 0) !== value.events) fail();
    for (const field of fields) if (groups.reduce((sum, group) => sum + group.usage[field], 0) !== value.usage![field]) fail();
  }
  return value;
}

export async function fetchQuotaIntervalUsage(row: QuotaHistoryInterval, selection: QuotaHistoryCursor, signal?: AbortSignal): Promise<QuotaIntervalUsageResponse> {
  const params = new URLSearchParams({ selection: JSON.stringify(selection) });
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/quota-interval-usage?${params}`, { signal, headers: { Accept: 'application/json' } });
  if (!response.ok) throw await ledgerResponseError(response);
  return validateQuotaIntervalUsage(await response.json() as QuotaIntervalUsageResponse, row, selection);
}

/** Explicit synthetic component data, never a real-request fallback. */
export function mockQuotaIntervalUsage(row: QuotaHistoryInterval, selection: QuotaHistoryCursor): QuotaIntervalUsageResponse {
  const usage = { input: 10_000_000, cached: 7_000_000, cacheWrite: 1_000_000, cacheWriteObservedInput: 10_000_000, cacheWriteCoverage: 1, uncached: 2_000_000, output: 2_000_000, reasoning: 500_000, total: 12_000_000 };
  return validateQuotaIntervalUsage({ scope: 'quota_interval_local_evidence_v1', status: 'available', intervalId: row.id, accountId: row.accountId, quotaView: selection.view,
    observedAt: selection.view.asOf, start: row.tokenSampleStart, end: row.tokenSampleEnd, coverageComplete: false, poolAttribution: false, events: 2, usage,
    models: [{ id: 'demo-model', label: 'Demo model', usage, events: 2 }], projects: [{ id: 'demo-project', label: 'Demo project', usage, events: 2 }],
  }, row, selection);
}
