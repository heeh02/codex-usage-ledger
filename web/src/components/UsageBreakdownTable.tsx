import type { MetricKey, TokenUsage } from '../api/types';
import { useEffect, useRef } from 'react';
import { exactNumber, formatTokenMillions, metricValue } from '../lib';
import { useI18n } from '../i18n';
import { BreakdownPagination, useBreakdownPage } from './BreakdownPagination';
import './usage-breakdown.css';

export interface UsageBreakdownRow { id: string | null; label?: string | null; events: number; usage: TokenUsage; available?: boolean }
export function UsageBreakdownTable({ rows, identityLabel, resolveLabel, metric = 'total', scopeKey = '', onSelect }: { rows: UsageBreakdownRow[]; identityLabel: string; resolveLabel?: (row: UsageBreakdownRow) => string; metric?: MetricKey; scopeKey?: string; onSelect?: (id: string) => void }) {
  const { t } = useI18n();
  const paging = useBreakdownPage(rows.length, `${scopeKey}:${metric}`);
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => { if (scroller.current) scroller.current.scrollTop = 0; }, [paging.page, scopeKey, metric]);
  const sorted = [...rows].sort((a, b) => Number(b.available !== false) - Number(a.available !== false)
    || metricValue(b.usage, metric, b.events) - metricValue(a.usage, metric, a.events)
    || (a.id ?? '').localeCompare(b.id ?? ''));
  const unsplit = rows.some(row => row.available !== false && row.usage.input > 0 && row.usage.cacheWriteCoverage < 1);
  return <div className="usage-breakdown">
    <p className="usage-breakdown-note">{t('breakdown.local_components')}</p>
    <div ref={scroller} className="usage-breakdown-scroll" tabIndex={0} role="region" aria-label={identityLabel}>
    <table><thead><tr><th>{identityLabel}</th>{(['total', 'uncached', 'cached', 'cacheWrite', 'output', 'events'] as const).map(key =>
      <th key={key}>{key === 'uncached' && unsplit ? t('components.explorer.input_unsplit') : t(`sessions.mix_${key}`)}</th>)}</tr></thead>
      <tbody>{sorted.slice(paging.offset, paging.end).map(row => <tr key={JSON.stringify(row.id)}>
        <td>{onSelect && row.id !== null ? <button className="usage-breakdown-link" type="button" onClick={() => onSelect(row.id!)}>{resolveLabel ? resolveLabel(row) : row.label ?? row.id}</button> : resolveLabel ? resolveLabel(row) : row.id === null ? t('sessions.unknown_dimension') : row.label ?? row.id}
          {row.available === false && <small>{t('usage.no_confirmed_records')}</small>}</td>
        {(['total', 'uncached', 'cached', 'cacheWrite', 'output'] as const).map(key => <td key={key} title={row.available === false || (key === 'cacheWrite' && row.usage.cacheWriteCoverage === 0) ? undefined : exactNumber(row.usage[key])}>
          {row.available === false || (key === 'cacheWrite' && row.usage.cacheWriteCoverage === 0) ? '—' : formatTokenMillions(row.usage[key])}
          {row.available !== false && key === 'cacheWrite' && row.usage.cacheWriteCoverage > 0 && row.usage.cacheWriteCoverage < 1 ? ` (${t('sessions.partial_split')})` : ''}
        </td>)}<td>{exactNumber(row.events)}</td>
      </tr>)}</tbody></table>
    </div><BreakdownPagination count={rows.length} {...paging} />
  </div>;
}
