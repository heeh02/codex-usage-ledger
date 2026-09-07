import { ledgerResponseError } from './errors';
import type { QuotaHistoryResponse, QuotaHistoryCursor, QuotaHistoryView, QuotaHistoryKey } from './quota-history.generated';
export type { QuotaHistoryResponse, QuotaHistoryCursor, QuotaHistoryInterval } from './quota-history.generated';

export interface QuotaHistoryQuery { account: string; cursor?: QuotaHistoryCursor | null; limit?: number }
const validDate = (value: unknown): value is string => typeof value === 'string' && Number.isFinite(Date.parse(value));
const integer = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0;
const text = (value: unknown): value is string => typeof value === 'string' && value.length > 0;
const viewMatches = (a: QuotaHistoryView, b: QuotaHistoryView) => a.instance === b.instance && a.revision === b.revision && a.account === b.account && a.asOf === b.asOf;
const keyMatches = (a: QuotaHistoryKey, b: QuotaHistoryKey) => a.at === b.at && a.snapshotId === b.snapshotId && a.ordinal === b.ordinal;
function compare(a: QuotaHistoryKey, b: QuotaHistoryKey): number {
  return Date.parse(a.at) - Date.parse(b.at) || (a.snapshotId < b.snapshotId ? -1 : a.snapshotId > b.snapshotId ? 1 : a.ordinal - b.ordinal);
}

export function validateQuotaHistory(value: QuotaHistoryResponse, query: QuotaHistoryQuery): QuotaHistoryResponse {
  const fail = () => { throw new Error('Quota history response is invalid or belongs to another view'); };
  if (!value || value.scope !== 'quota_observation_history_v1' || typeof value.indexReady !== 'boolean'
    || value.sourceHistoryComplete !== false || !value.view || value.view.account !== query.account
    || !text(value.view.instance) || !integer(value.view.revision) || !validDate(value.view.asOf)
    || !Array.isArray(value.intervals) || value.intervals.length > (query.limit ?? 20)) fail();
  if (query.cursor && !viewMatches(value.view, query.cursor.view)) fail();
  if (!value.indexReady && (value.intervals.length !== 0 || value.next !== null)) fail();
  const ids = new Set<string>();
  for (const [index, row] of value.intervals.entries()) {
    if (!row || !text(row.id) || ids.has(row.id) || !text(row.accountId) || !text(row.limitId)
      || (query.account !== 'all' && row.accountId !== query.account) || !row.key || !validDate(row.key.at)
      || !text(row.key.snapshotId) || !integer(row.key.ordinal) || !integer(row.sampleCount) || row.sampleCount < 1
      || !validDate(row.firstObservedAt) || !validDate(row.lastObservedAt)
      || Date.parse(row.lastObservedAt) < Date.parse(row.firstObservedAt) || row.key.at !== row.firstObservedAt
      || Date.parse(row.lastObservedAt) > Date.parse(value.view.asOf)) fail();
    for (const amount of [row.firstUsedPercent, row.lastUsedPercent]) if (amount !== null && (typeof amount !== 'number' || !Number.isFinite(amount) || amount < 0 || amount > 100)) fail();
    if (row.windowSeconds !== null && (typeof row.windowSeconds !== 'string' || !/^\d{1,20}$/.test(row.windowSeconds))) fail();
    for (const at of [row.boundaryAfter, row.nominalStart, row.reportedReset, row.tokenSampleStart, row.tokenSampleEnd]) if (at !== null && !validDate(at)) fail();
    if ((row.tokenSampleStart === null) !== (row.tokenSampleEnd === null)
      || (row.tokenSampleStart !== null && row.tokenSampleEnd !== null && Date.parse(row.tokenSampleEnd) <= Date.parse(row.tokenSampleStart))) fail();
    if (index > 0 && compare(value.intervals[index - 1].key, row.key) <= 0) fail();
    if (query.cursor && compare(row.key, query.cursor.before) >= 0) fail();
    ids.add(row.id);
  }
  if (value.next !== null && (!value.next || !value.next.view || !value.next.before || !viewMatches(value.next.view, value.view)
    || value.intervals.length === 0 || !keyMatches(value.next.before, value.intervals.at(-1)!.key)
    || !/^[a-f0-9]{64}$/.test(value.next.signature))) fail();
  return value;
}

export async function fetchQuotaHistory(query: QuotaHistoryQuery, signal?: AbortSignal): Promise<QuotaHistoryResponse> {
  const params = new URLSearchParams({ account: query.account, limit: String(query.limit ?? 20) });
  if (query.cursor) params.set('cursor', JSON.stringify(query.cursor));
  const base = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
  const response = await fetch(`${base}/v1/quota-history?${params}`, { signal, headers: { Accept: 'application/json' } });
  if (!response.ok) throw await ledgerResponseError(response);
  return validateQuotaHistory(await response.json() as QuotaHistoryResponse, query);
}
