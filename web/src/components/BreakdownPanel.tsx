import type { BreakdownDimension, BreakdownRow, BreakdownsResponse, MetricKey } from '../api/types';
import { compactNumber, dimensionLabel, formatPercent, metricLabel, metricValue } from '../lib';
import { EmptyState, Panel } from './Ui';
import { useI18n } from '../i18n';

function BreakdownList({ rows, metric, onSelect }: { rows: BreakdownRow[]; metric: MetricKey; onSelect?: (id: string) => void }) {
  const { t } = useI18n();
  if (!rows.length) return <EmptyState text={t('components.breakdown-panel.no_attributable_records_match_the_current_filters')} />;
  const selectedTotal = rows.reduce((sum, row) => sum + metricValue(row.usage.confirmed, metric, row.confirmedEvents), 0);
  return (
    <div className="breakdown-list">
      {[...rows].sort((left, right) => metricValue(right.usage.confirmed, metric, right.confirmedEvents) - metricValue(left.usage.confirmed, metric, left.confirmedEvents)).slice(0, 7).map((row, index) => {
        const value = metricValue(row.usage.confirmed, metric, row.confirmedEvents);
        const share = selectedTotal ? value / selectedTotal : 0;
        const label = dimensionLabel(row.id, row.label);
        const content = <>
          <span className="breakdown-rank">{String(index + 1).padStart(2, '0')}</span>
          <div className="breakdown-name">
            <strong>{label}</strong>
            <small>{row.description ?? `${row.confirmedEvents} ${t('components.breakdown-panel.confirmed_records')}`}</small>
          </div>
          <div className="breakdown-values">
            <strong>{compactNumber(value)}</strong>
            <span>{formatPercent(share)}</span>
          </div>
          <div className="breakdown-track" aria-label={`${label} ${t('components.breakdown-panel.share_of_confirmed_usage')} ${formatPercent(share)}`}>
            <span className="breakdown-confirmed" style={{ width: `${Math.min(100, share * 100)}%` }}><i /></span>
          </div>
          <div className="breakdown-quality">
            <span>{t('components.breakdown-panel.quarantined')} {compactNumber(row.usage.quarantined.total)}</span>
            <span>{t('components.breakdown-panel.see_data_quality_for_unknowns')}</span>
          </div>
        </>;
        return onSelect
          ? <button className="breakdown-row is-clickable" key={row.id} onClick={() => onSelect(row.id)} type="button">{content}</button>
          : <article className="breakdown-row" key={row.id}>{content}</article>;
      })}
    </div>
  );
}

export function BreakdownPanel({ data, metric, dimensions = ['account', 'project', 'model'], onSelect }: { data: BreakdownsResponse; metric: MetricKey; dimensions?: BreakdownDimension[]; onSelect?: (dimension: BreakdownDimension, id: string) => void }) {
  const { t } = useI18n();
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
          <BreakdownList rows={data[dimension]} metric={metric} onSelect={onSelect ? (id) => onSelect(dimension, id) : undefined} />
        </Panel>
      ))}
    </section>
  );
}
