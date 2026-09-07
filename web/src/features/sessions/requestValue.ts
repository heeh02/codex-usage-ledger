import type { RequestEvidenceRow } from '../../api/request-evidence.generated';
import { exactNumber, formatTokenMillions } from '../../lib';

export function requestTokenDisplay(
  row: RequestEvidenceRow,
  field: 'uncached' | 'cached' | 'cacheWrite' | 'output' | 'total' | 'reasoning',
  precision: 'millions' | 'exact' = 'millions',
): string {
  if (row.quality === 'unknown') return '—';
  if (field === 'cacheWrite' && row.usage.cacheWriteCoverage <= 0) return '—';
  return precision === 'exact' ? exactNumber(row.usage[field]) : formatTokenMillions(row.usage[field]);
}
