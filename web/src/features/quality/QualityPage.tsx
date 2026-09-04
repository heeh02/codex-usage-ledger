import type { DashboardBundle, MetricKey } from '../../api/types';
import { QualityPanel } from '../../components/QualityPanel';

export function QualityPage({ bundle, metric }: { bundle: DashboardBundle; metric: MetricKey }) {
  return <QualityPanel data={bundle.quality} metric={metric} />;
}
