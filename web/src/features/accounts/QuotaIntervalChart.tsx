import type { QuotaIntervalUsageResponse } from '../../api/quotaIntervalUsage';
import { useI18n } from '../../i18n';
import { formatTokenMillions } from '../../lib';
import './quota-interval-chart.css';

export function QuotaIntervalChart({ data, formatTimestamp }: { data: QuotaIntervalUsageResponse; formatTimestamp: (at: string | null) => string }) {
  const { t } = useI18n();
  const chart = data.chart;
  if (!chart) return null;
  const times = [...chart.observations.map(p => Date.parse(p.at)), ...chart.buckets.flatMap(b => [Date.parse(b.start), Date.parse(b.end)])];
  if (!times.length) return <p>{t('quota_chart.empty')}</p>;
  const start = Math.min(...times), end = Math.max(...times), span = Math.max(1, end - start);
  const x = (at: number) => 70 + 640 * (at - start) / span;
  const y = (used: number) => 38 + used * 1.25;
  const maximum = Math.max(0, ...chart.buckets.map(b => b.usage.total));
  const max = Math.max(1, maximum);
  return <section className="quota-interval-chart" aria-label={t('quota_chart.title')}>
    <h3>{t('quota_chart.title')}</h3>
    <p>{t('quota_chart.scope')}</p>
    {chart.observationsTruncated && <p role="status">{t('quota_chart.truncated')}</p>}
    <div className="quota-interval-chart-scroll" tabIndex={0}>
      <svg viewBox="0 0 740 380" role="img" aria-label={t('quota_chart.title')}>
        <text x="70" y="20">{t('quota_chart.remaining')}</text>
        {[0, 50, 100].map(percent => <g key={percent}>
          <line x1="70" x2="710" y1={y(100 - percent)} y2={y(100 - percent)} className="quota-chart-grid"/>
          <text x="60" y={y(100 - percent) + 4} textAnchor="end">{percent}%</text>
        </g>)}
        {chart.observations.map((point, i) => {
          if (point.usedPercent === null) return null;
          const at = Date.parse(point.at), prev = chart.observations[i - 1];
          const join = prev && prev.usedPercent !== null && at > Date.parse(prev.at) && at - Date.parse(prev.at) <= 15 * 60_000;
          return <g key={`${point.at}-${i}`}>
            {join && <path d={`M ${x(Date.parse(prev.at))} ${y(prev.usedPercent!)} H ${x(at)} V ${y(point.usedPercent)}`} className="quota-chart-step"/>}
            <circle cx={x(at)} cy={y(point.usedPercent)} r="3" className="quota-chart-point"><title>{formatTimestamp(point.at)} · {(100 - point.usedPercent).toFixed(1)}%</title></circle>
          </g>;
        })}
        <text x="70" y="205">{t('quota_chart.tokens')}</text>
        <line x1="70" x2="710" y1="325" y2="325" className="quota-chart-grid"/>
        <text x="60" y="329" textAnchor="end">0 M</text>
        {chart.buckets.length > 0 && <text x="70" y="227">{formatTokenMillions(maximum)}</text>}
        {chart.buckets.map(bucket => {
          const left = x(Date.parse(bucket.start)), right = x(Date.parse(bucket.end));
          const height = bucket.usage.total / max * 90;
          return <rect key={bucket.start} x={left} y={325 - height} width={Math.max(0.5, right - left - 1)} height={height} className="quota-chart-bar">
            <title>{formatTimestamp(bucket.start)} — {formatTimestamp(bucket.end)} · {formatTokenMillions(bucket.usage.total)}</title>
          </rect>;
        })}
        {!chart.buckets.length && <text x="390" y="275" textAnchor="middle">{t('quota_chart.no_tokens')}</text>}
        <text x="70" y="354">{formatTimestamp(new Date(start).toISOString())}</text>
        <text x="710" y="374" textAnchor="end">{formatTimestamp(new Date(end).toISOString())}</text>
      </svg>
    </div>
    <p>{t('quota_chart.gaps')}</p>
    <details><summary>{t('quota_chart.table')}</summary>
      <div className="quota-interval-chart-table" tabIndex={0}>
        <table><caption>{t('quota_chart.remaining')}</caption><thead><tr><th>{t('quota_chart.time')}</th><th>%</th></tr></thead><tbody>
          {chart.observations.map((p, i) => <tr key={i}><td>{formatTimestamp(p.at)}</td><td>{p.usedPercent === null ? '—' : (100 - p.usedPercent).toFixed(1)}</td></tr>)}
        </tbody></table>
        <table><caption>{t('quota_chart.tokens')}</caption><thead><tr><th>{t('quota_chart.time')}</th><th>M</th></tr></thead><tbody>
          {chart.buckets.map(b => <tr key={b.start}><td>{formatTimestamp(b.start)} — {formatTimestamp(b.end)}</td><td title={String(b.usage.total)}>{formatTokenMillions(b.usage.total)}</td></tr>)}
        </tbody></table>
      </div>
    </details>
  </section>;
}
