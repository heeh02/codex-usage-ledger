import { validateQuotaHistory, type QuotaHistoryQuery, type QuotaHistoryResponse, type QuotaHistoryInterval } from './quotaHistory';

/** Explicit demo-only data. Never a fallback for a failed HTTP history request. */
export function mockQuotaHistory(query: QuotaHistoryQuery): QuotaHistoryResponse {
  const accounts = ['acct-personal', 'acct-research', 'acct-unknown'];
  const view = query.cursor?.view ?? { account: query.account, revision: 1, instance: 'synthetic-history', asOf: '2026-09-01T00:00:00Z' };
  const rows: QuotaHistoryInterval[] = Array.from({ length: 45 }, (_, index) => {
    const at = new Date(Date.UTC(2026, 0, 1 + index)).toISOString();
    const end = new Date(Date.parse(at) + 3_600_000).toISOString();
    const snapshotId = `synthetic-${String(index).padStart(3, '0')}`;
    return { id: `${snapshotId}:0`, accountId: accounts[index % 3], key: { at, snapshotId, ordinal: 0 },
      streamKey: 'synthetic-weekly', poolKey: 'synthetic-pool', limitId: 'synthetic-weekly', limitName: 'Demo weekly pool', role: 'primary', windowSeconds: '604800',
      boundaryKind: index % 2 === 0 ? 'deadline_change' : 'observed_decrease', boundaryAfter: new Date(Date.parse(at) - 3_600_000).toISOString(),
      firstObservedAt: at, lastObservedAt: end, sampleCount: 3, firstUsedPercent: 10, lastUsedPercent: 40,
      nominalStart: new Date(Date.parse(end) - 7 * 86400_000).toISOString(), reportedReset: end, tokenSampleStart: at, tokenSampleEnd: end };
  }).filter(row => query.account === 'all' || row.accountId === query.account).reverse();
  const remaining = query.cursor ? rows.filter(row => row.key.at < query.cursor!.before.at) : rows;
  const intervals = remaining.slice(0, query.limit ?? 20);
  const next = remaining.length > intervals.length ? { view, before: intervals.at(-1)!.key, signature: 'a'.repeat(64) } : null;
  return validateQuotaHistory({ scope: 'quota_observation_history_v1', indexReady: true, sourceHistoryComplete: false, view, intervals, next }, query);
}
