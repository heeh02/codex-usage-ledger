import type { DashboardBundle, MetricKey } from '../../api/types';
import { BreakdownPanel } from '../../components/BreakdownPanel';
import { UsageTrendPanel } from '../../components/TrendAndTimeline';
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
    <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['model']} onSelect={onSelect} />
  </section>;
}
