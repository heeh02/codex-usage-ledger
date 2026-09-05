import type { RequestEvidenceRow } from '../../api/request-evidence.generated';

export function requestTokenDisplay(
  row: RequestEvidenceRow,
  field: 'uncached' | 'cached' | 'cacheWrite' | 'output' | 'total' | 'reasoning',
): string {
  if (row.quality === 'unknown') return '—';
  if (field === 'cacheWrite' && row.usage.cacheWriteCoverage <= 0) return '—';
  return row.usage[field].toLocaleString();
}
