import type { BreakdownDimension, BreakdownRow, BreakdownsResponse, MetricKey } from '../api/types';
import { exactNumber, formatMetricAmount, formatTokenMillions, dimensionLabel, formatPercent, metricLabel, metricValue } from '../lib';
import { EmptyState, Panel } from './Ui';
import { useI18n } from '../i18n';
import { UsageBreakdownTable } from './UsageBreakdownTable';
import { BreakdownPagination, useBreakdownPage } from './BreakdownPagination';

function BreakdownList({ rows, metric, scopeKey, onSelect }: { rows: BreakdownRow[]; metric: MetricKey; scopeKey: string; onSelect?: (id: string) => void }) {
  const { t } = useI18n();
  const paging = useBreakdownPage(rows.length, `${scopeKey}:${metric}`);
  if (!rows.length) return <EmptyState text={t('components.breakdown-panel.no_attributable_records_match_the_current_filters')} />;
  const selectedTotal = rows.reduce((sum, row) => sum + metricValue(row.usage.confirmed, metric, row.confirmedEvents), 0);
  return (
    <div className="breakdown-list">
      {[...rows].sort((left, right) => metricValue(right.usage.confirmed, metric, right.confirmedEvents) - metricValue(left.usage.confirmed, metric, left.confirmedEvents) || left.id.localeCompare(right.id)).slice(paging.offset, paging.end).map((row, index) => {
        const value = metricValue(row.usage.confirmed, metric, row.confirmedEvents);
        const share = selectedTotal ? value / selectedTotal : 0;
        const label = dimensionLabel(row.id, row.label);
        const content = <>
          <span className="breakdown-rank">{String(paging.offset + index + 1).padStart(2, '0')}</span>
          <div className="breakdown-name">
            <strong>{label}</strong>
            <small>{row.description ?? `${row.confirmedEvents} ${t('components.breakdown-panel.confirmed_records')}`}</small>
          </div>
          <div className="breakdown-values">
            <strong title={row.confirmedEvents ? exactNumber(value) : undefined}>{row.confirmedEvents ? formatMetricAmount(value, metric) : '—'}</strong>
            <span>{row.confirmedEvents ? formatPercent(share) : t('usage.no_confirmed_records')}</span>
          </div>
          <div className="breakdown-track" aria-label={`${label} ${t('components.breakdown-panel.share_of_confirmed_usage')} ${formatPercent(share)}`}>
            <span className="breakdown-confirmed" style={{ width: `${Math.min(100, share * 100)}%` }}><i /></span>
          </div>
          <div className="breakdown-quality">
            <span>{t('components.breakdown-panel.quarantined')} {formatTokenMillions(row.usage.quarantined.total)}</span>
            <span>{t('components.breakdown-panel.see_data_quality_for_unknowns')}</span>
          </div>
        </>;
        return onSelect
          ? <button className="breakdown-row is-clickable" key={row.id} onClick={() => onSelect(row.id)} type="button">{content}</button>
          : <article className="breakdown-row" key={row.id}>{content}</article>;
      })}
      <BreakdownPagination count={rows.length} {...paging} />
    </div>
  );
}

export function BreakdownPanel({ data, metric, dimensions = ['account', 'project', 'model'], scopeKey = '', onSelect }: { data: BreakdownsResponse; metric: MetricKey; dimensions?: BreakdownDimension[]; scopeKey?: string; onSelect?: (dimension: BreakdownDimension, id: string) => void }) {
  const { t } = useI18n();
  // Live end/coverage timestamps advance on refresh; they must not reset browsing.
  const pageScope = `${scopeKey}:${data.period.key}:${data.period.timezone}`;
  const titles: Record<BreakdownDimension, { title: string; eyebrow: string }> = {
    account: { title: t('components.breakdown-panel.account_distribution'), eyebrow: t('components.breakdown-panel.by_account') },
    project: { title: t('components.breakdown-panel.project_distribution'), eyebrow: t('components.breakdown-panel.by_project') },
    model: { title: t('components.breakdown-panel.model_distribution'), eyebrow: t('components.breakdown-panel.by_model') },
  };
  return (
    <section className="breakdown-grid">
      {dimensions.map((dimension) => (
        <Panel
          key={dimension}
          title={titles[dimension].title}
          eyebrow={titles[dimension].eyebrow}
          meta={<span className="definition-chip">{metricLabel(metric)}</span>}
          className={`breakdown-panel breakdown-${dimension}`}
        >
          {dimension === 'model' && data.model.length > 0
            ? <UsageBreakdownTable rows={data.model.map(row => ({ id: row.id, label: dimensionLabel(row.id, row.label), usage: row.usage.confirmed, events: row.confirmedEvents, available: row.confirmedEvents > 0 }))}
                identityLabel={titles.model.title} metric={metric} scopeKey={pageScope} onSelect={onSelect ? id => onSelect('model', id) : undefined} />
            : <BreakdownList rows={data[dimension]} metric={metric} scopeKey={pageScope} onSelect={onSelect ? (id) => onSelect(dimension, id) : undefined} />}
        </Panel>
      ))}
    </section>
  );
}
