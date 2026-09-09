import type { DashboardBundle, MetricKey } from '../../api/types';
import { BreakdownPanel } from '../../components/BreakdownPanel';
import { DimensionCompareChart, UsageTrendPanel } from '../../components/TrendAndTimeline';
import { useI18n } from '../../i18n';

export function ModelsPage({ bundle, metric, onSelect }: {
  bundle: DashboardBundle;
  metric: MetricKey;
  onSelect: (dimension: 'account' | 'project' | 'model', id: string) => void;
}) {
  const { t } = useI18n();
  const local = { ...bundle.timeseries, official: { ...bundle.timeseries.official, primaryScope: false } };
  return <section className="models-page">
    <UsageTrendPanel data={local} metric={metric} title={t('models.trend')} allowProjectCompare={false} />
    <section className="panel"><header className="panel-heading"><h2>{t('models.compare')}</h2></header><DimensionCompareChart data={{ ...local, projectSeries: bundle.timeseries.modelSeries ?? [] }} metric={metric} /></section>
    <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['model']} onSelect={onSelect} />
  </section>;
}
