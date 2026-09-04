import type { DashboardBundle, DashboardFilters, TokenUsage } from './api/types';

// Machine-readable columns intentionally stay stable across interface languages.
export const USAGE_CSV_COLUMNS = [
  'bucket', 'source', 'grain', 'total', 'input_total', 'cache_read',
  'cache_write_observed', 'cache_write_coverage', 'input_noncache_or_unresolved',
  'output', 'reasoning', 'requests', 'account', 'project', 'session', 'model', 'session_scope',
  'period_start', 'period_end', 'timezone', 'status',
] as const;

export function usageExportRows(bundle: DashboardBundle, filters: DashboardFilters, scope: 'own' | 'tree' = 'tree') {
  const window = bundle.summary.period;
  const context = [filters.account, filters.project, filters.session, filters.model,
    filters.session !== 'all' ? scope : '', window.start, window.end, window.timezone];
  const localRow = (bucket: string, grain: string, usage: TokenUsage, requests: number) => [
    bucket, 'local', grain, usage.total, usage.input, usage.cached,
    usage.cacheWriteCoverage > 0 ? usage.cacheWrite : '', usage.cacheWriteCoverage,
    usage.uncached, usage.output, usage.reasoning, requests, ...context,
    usage.cacheWriteCoverage >= 0.999 ? 'local_recorded' : 'local_cache_write_partial',
  ];
  const detail = bundle.explorer.selectedSession;
  const rows = filters.session !== 'all'
    ? (detail ? (scope === 'own' ? detail.ownSamplingTimeline : detail.samplingTimeline)
      .map(point => localRow(point.bucket, detail.samplingGrain, point.usage, point.events)) : [])
    : bundle.timeseries.points.map(point => localRow(point.date, bundle.timeseries.grain, point.confirmed, point.confirmedEvents));

  // Official data has no project/model/session dimensions. Do not include an
  // unfiltered account total in an export claiming one of these local scopes.
  if (filters.project === 'all' && filters.model === 'all' && filters.session === 'all') {
    for (const point of bundle.summary.official.points) {
      rows.push([point.date, 'official', bundle.summary.official.granularity, point.tokens,
        '', '', '', '', '', '', '', '', ...context, 'official_reported']);
    }
  }
  return rows.sort((left, right) => String(left[0]).localeCompare(String(right[0])) || String(left[1]).localeCompare(String(right[1])));
}

export function csvCell(value: string | number): string {
  if (typeof value === 'number') return String(value);
  const safe = /^\s*[=+@-]/.test(value) ? `'${value}` : value;
  return `"${safe.replaceAll('"', '""')}"`;
}

export function exportUsageCsv(bundle: DashboardBundle, filters: DashboardFilters, scope: 'own' | 'tree' = 'tree'): string {
  return [USAGE_CSV_COLUMNS, ...usageExportRows(bundle, filters, scope)]
    .map(row => row.map(csvCell).join(',')).join('\r\n');
}
