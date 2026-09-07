import { hasCacheWriteAmount } from '../../shared/cacheWriteDisplay';
import type { RequestEvidenceRow } from '../../api/request-evidence.generated';
import { exactNumber, formatTokenMillions } from '../../lib';

export function requestTokenDisplay(
  row: RequestEvidenceRow,
  field: 'uncached' | 'cached' | 'cacheWrite' | 'output' | 'total' | 'reasoning',
  precision: 'millions' | 'exact' = 'millions',
): string {
  if (row.quality === 'unknown') return '—';
  if (field === 'cacheWrite' && !hasCacheWriteAmount(row.usage)) return '—';
  return precision === 'exact' ? exactNumber(row.usage[field]) : formatTokenMillions(row.usage[field]);
}
