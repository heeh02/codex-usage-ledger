import type { DashboardBundle, MetricKey } from '../../api/types';
import { SessionExplorer, type SessionViewState } from '../../components/Explorer';
import { useI18n } from '../../i18n';
import { UsageTrendPanel } from '../../components/TrendAndTimeline';

interface SessionPageProps {
  bundle: DashboardBundle;
  metric: MetricKey;
  view: SessionViewState;
  onViewChange: (view: SessionViewState) => void;
  onOpenSession: (session: string) => void;
  onBack?: () => void;
}

export function SessionPage({ bundle, metric, view, onViewChange, onOpenSession, onBack }: SessionPageProps) {
  const { t } = useI18n();
  const detail = bundle.explorer.selectedSession;
  const timeline = detail ? view.scope === 'own' ? detail.ownSamplingTimeline : detail.samplingTimeline : [];
  const empty = { input: 0, cached: 0, cacheWrite: 0, cacheWriteObservedInput: 0, cacheWriteCoverage: 0, uncached: 0, output: 0, reasoning: 0, total: 0 };
  const series = {
    ...bundle.timeseries,
    grain: detail?.samplingGrain ?? bundle.timeseries.grain,
    official: { ...bundle.timeseries.official, primaryScope: false },
    comparisonPoints: [], projectSeries: [],
    points: timeline.map(point => ({ date: point.bucket, confirmed: point.usage, confirmedEvents: point.events,
      quarantined: empty, unknown: empty, quarantinedEvents: 0, unknownEvents: 0 })),
  };
  return <>{onBack && <button className="session-parent-back" type="button" onClick={onBack}>{t('chats.back_parent')}</button>}<SessionExplorer detail={detail ?? null} metric={metric} view={view} onViewChange={onViewChange} onOpenSession={onOpenSession}
    trend={<UsageTrendPanel data={series} metric={metric} allowProjectCompare={false} title={t('components.explorer.usage_trajectory')} />} /></>;
}
