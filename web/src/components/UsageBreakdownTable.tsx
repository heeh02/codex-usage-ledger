import type { TokenUsage } from '../api/types';
import { exactNumber, formatTokenMillions } from '../lib';
import { useI18n } from '../i18n';
import './usage-breakdown.css';

export interface UsageBreakdownRow { id: string | null; label?: string | null; events: number; usage: TokenUsage }
export function UsageBreakdownTable({ rows, identityLabel, resolveLabel }: { rows: UsageBreakdownRow[]; identityLabel: string; resolveLabel?: (row: UsageBreakdownRow) => string }) {
  const { t } = useI18n();
  const unsplit = rows.some(row => row.usage.cacheWriteCoverage < 1);
  return <div className="usage-breakdown-scroll" tabIndex={0} role="region" aria-label={identityLabel}>
    <table><thead><tr><th>{identityLabel}</th>{(['total', 'uncached', 'cached', 'cacheWrite', 'output', 'events'] as const).map(key =>
      <th key={key}>{key === 'uncached' && unsplit ? t('components.explorer.input_unsplit') : t(`sessions.mix_${key}`)}</th>)}</tr></thead>
      <tbody>{[...rows].sort((a, b) => b.usage.total - a.usage.total).map(row => <tr key={JSON.stringify(row.id)}>
        <td>{resolveLabel ? resolveLabel(row) : row.id === null ? t('sessions.unknown_dimension') : row.label ?? row.id}</td>
        {(['total', 'uncached', 'cached', 'cacheWrite', 'output'] as const).map(key => <td key={key} title={key === 'cacheWrite' && row.usage.cacheWriteCoverage === 0 ? undefined : exactNumber(row.usage[key])}>
          {key === 'cacheWrite' && row.usage.cacheWriteCoverage === 0 ? '—' : formatTokenMillions(row.usage[key])}
          {key === 'cacheWrite' && row.usage.cacheWriteCoverage > 0 && row.usage.cacheWriteCoverage < 1 ? ` (${t('sessions.partial_split')})` : ''}
        </td>)}<td>{exactNumber(row.events)}</td>
      </tr>)}</tbody></table>
  </div>;
}
