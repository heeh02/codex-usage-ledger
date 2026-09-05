import { useEffect, useRef, useState } from 'react';
import type { MetricKey, TimeseriesResponse } from '../api/types';
import { bucketDateRange, contiguous, timeRatio, trendSeries, type TrendPoint } from '../charts/time';
import { compactNumber, metricLabel, shortDate } from '../lib';
import { useI18n } from '../i18n';
import { EmptyState } from './Ui';

export function UsageTrendChart({ data, metric, onInspectRange }: { data: TimeseriesResponse; metric: MetricKey; onInspectRange?: (range: { startDate: string; endDate: string }) => void }) {
  const { t } = useI18n();
  const container = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(640);
  const [selected, setSelected] = useState<number | null>(null);
  useEffect(() => {
    if (!container.current) return;
    const observer = new ResizeObserver(entries => setWidth(Math.max(280, Math.round(entries[0].contentRect.width))));
    observer.observe(container.current);
    return () => observer.disconnect();
  }, []);
  const { points, grain, account, domain } = trendSeries(data, metric);
  const height = 260, left = 66, right = 18, top = 24, bottom = 36;
  const max = Math.max(1, ...points.flatMap(p => [p.value ?? 0, p.previous ?? 0, p.local ?? 0]));
  const x = (date: string) => left + timeRatio(date, domain) * (width - left - right);
  const y = (value: number) => height - bottom - value / max * (height - top - bottom);
  const segments = (field: 'value' | 'previous' | 'local') => contiguous(points, grain, p => p[field] !== null);
  const path = (segment: TrendPoint[], field: 'value' | 'previous' | 'local') => segment.map(p => `${x(p.date)},${y(p[field]!)}`).join(' ');
  const index = Math.max(0, Math.min(points.length - 1, selected ?? points.length - 1));
  const active = points[index];
  const peak = points.reduce<TrendPoint | null>((best, point) => point.value !== null && (best?.value == null || point.value > best.value) ? point : best, null);
  const total = account ? data.official.displayTotalTokens : points.reduce((sum, p) => sum + (p.value ?? 0), 0);
  const name = account ? t('components.trend-and-timeline.daily_account_reconciliation') : t('app.local_attribution');
  const format = (value: number | null) => value === null ? '—' : compactNumber(value);
  const labels = [...new Set([0, Math.floor((points.length - 1) / 2), points.length - 1])].filter(i => i >= 0);
  return <div className="usage-time-chart" ref={container}>
    {!active ? <EmptyState text={t('components.trend-and-timeline.once_collection_starts_a_trusted_daily_token')} /> : <>
      <div className="trend-kpis">
        <div><span>{name} · {metricLabel(metric)}</span><strong>{account && data.official.displayIsLowerBound ? '≥ ' : ''}{format(total)}</strong></div>
        <div><span>{t('components.trend-and-timeline.peak')} · {peak ? shortDate(peak.date) : '—'}</span><strong>{format(peak?.value ?? null)}</strong></div>
      </div>
      <div className="chart-canvas">
        <svg className="trend-chart" viewBox={`0 0 ${width} ${height}`} role="img" tabIndex={0}
          aria-label={`${name} · ${metricLabel(metric)} · ${t('chart.keyboard_hint')}`}
          onKeyDown={event => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
            event.preventDefault();
            setSelected(event.key === 'Home' ? 0 : event.key === 'End' ? points.length - 1
              : Math.max(0, Math.min(points.length - 1, index + (event.key === 'ArrowLeft' ? -1 : 1))));
          }}
          onPointerMove={event => {
            const rect = event.currentTarget.getBoundingClientRect();
            const target = (event.clientX - rect.left) / rect.width * width;
            let nearest = 0;
            points.forEach((point, i) => { if (Math.abs(x(point.date) - target) < Math.abs(x(points[nearest].date) - target)) nearest = i; });
            setSelected(nearest);
          }}>
          {[0, 0.25, 0.5, 0.75, 1].map(ratio => <g key={ratio}>
            <line className="chart-gridline" x1={left} x2={width - right} y1={y(max * ratio)} y2={y(max * ratio)} />
            <text className="chart-axis-label" x={left - 9} y={y(max * ratio) + 4} textAnchor="end">{compactNumber(max * ratio)}</text>
          </g>)}
          {(['value', 'previous', 'local'] as const).map(field => segments(field).map((segment, i) =>
            <g key={`${field}-${i}`}>
              <polyline points={path(segment, field)} className={`trend-line ${field === 'value' ? 'confirmed-line' : field === 'previous' ? 'comparison-line' : 'quarantined-line'}`} />
              {segment.length === 1 && <circle cx={x(segment[0].date)} cy={y(segment[0][field]!)} r={3} className={field === 'value' ? 'active-point' : 'series-single-point'} />}
            </g>))}
          <line className="chart-crosshair" x1={x(active.date)} x2={x(active.date)} y1={top} y2={height - bottom} />
          {active.value !== null && <circle className="active-point" cx={x(active.date)} cy={y(active.value)} r={4} />}
          {labels.map(i => <text className="chart-date-label" key={i} x={x(points[i].date)} y={height - 12} textAnchor={i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle'}>{shortDate(points[i].date)}</text>)}
        </svg>
      </div>
      <div className="usage-time-readout" aria-live="polite">
        <span>{active.date}</span><strong>{format(active.value)}</strong>
        <span>{t('components.explorer.previous')} {format(active.previous)}</span>
        {account && <span>{t('app.local_attribution')} {format(active.local)}</span>}
        {onInspectRange && !account && <button type="button" onClick={() => onInspectRange(bucketDateRange(active.date, grain))}>{t(grain === 'hour' ? 'chart.open_day_chats' : 'chart.open_range_chats')}</button>}
      </div>
      <div className="chart-footer"><div className="chart-legend">
        <span><i className="legend-confirmed" />{name}</span>
        {points.some(p => p.previous !== null) && <span><i className="legend-comparison" />{t('components.trend-and-timeline.previous_period')}</span>}
        {points.some(p => p.local !== null) && <span><i className="legend-quarantined" />{t('app.local_attribution')}</span>}
      </div><span>{t('chart.gaps_unknown')}</span></div>
      <details className="chart-data-table"><summary>{t('components.trend-and-timeline.view_trend_data_table')}</summary>
        <div><table><thead><tr><th>{t('components.explorer.time')}</th><th>{name}</th><th>{t('components.explorer.previous')}</th>{account && <th>{t('app.local_attribution')}</th>}</tr></thead>
          <tbody>{points.map(point => <tr key={point.date}><td>{point.date}</td><td>{point.value?.toLocaleString() ?? '—'}</td><td>{point.previous?.toLocaleString() ?? '—'}</td>{account && <td>{point.local?.toLocaleString() ?? '—'}</td>}</tr>)}</tbody>
        </table></div>
      </details>
    </>}
  </div>;
}
