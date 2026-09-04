import type { DashboardBundle, MetricKey } from '../../api/types';
import { SessionExplorer, type SessionViewState } from '../../components/Explorer';

interface SessionPageProps {
  bundle: DashboardBundle;
  metric: MetricKey;
  view: SessionViewState;
  onViewChange: (view: SessionViewState) => void;
}

export function SessionPage({ bundle, metric, view, onViewChange }: SessionPageProps) {
  return <SessionExplorer detail={bundle.explorer.selectedSession ?? null} metric={metric} view={view} onViewChange={onViewChange} />;
}
