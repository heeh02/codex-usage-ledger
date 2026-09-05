import type { DashboardBundle, DashboardFilters, MetricKey } from '../../api/types';
import { ConversationControls } from '../../components/ConversationControls';
import { AttributionCoveragePanel } from '../../components/AttributionCoveragePanel';
import { BreakdownPanel } from '../../components/BreakdownPanel';
import { OverviewSessions } from '../../components/Explorer';
import { MissingAccountEstimatePanel } from '../../components/MissingAccountEstimatePanel';
import { LocalComposition } from '../../components/LocalComposition';
import { LocalUsagePulse } from '../../components/Explorer';
import { TrendAndTimeline } from '../../components/TrendAndTimeline';
import { useI18n } from '../../i18n';

export type OverviewDetailTab = 'projects' | 'models' | 'sessions';

interface OverviewPageProps {
  filters: DashboardFilters;
  onFiltersChange: (filters: DashboardFilters) => void;
  bundle: DashboardBundle;
  metric: MetricKey;
  detailTab: OverviewDetailTab;
  onDetailTabChange: (tab: OverviewDetailTab) => void;
  onOpenProject: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSelectBreakdown: (dimension: 'account' | 'project' | 'model', id: string) => void;
}

export function OverviewPage({
  filters,
  onFiltersChange,
  bundle,
  metric,
  detailTab,
  onDetailTabChange,
  onOpenProject,
  onOpenSession,
  onSelectBreakdown,
}: OverviewPageProps) {
  const { t } = useI18n();

  return (
    <>
      <LocalUsagePulse explorer={bundle.explorer} summary={bundle.summary} metric={metric} />
      <TrendAndTimeline data={{ ...bundle.timeseries, official: { ...bundle.timeseries.official, primaryScope: false } }} explorer={bundle.explorer} metric={metric} onOpenProject={onOpenProject} />
      <section className="overview-tabs panel">
        <nav aria-label={t('overview.usage_details')}>
          {([['projects', t('components.explorer.projects')], ['models', t('overview.models')], ['sessions', 'Sessions']] as const).map(([id, label]) => (
            <button aria-pressed={detailTab === id} className={detailTab === id ? 'is-active' : ''} key={id} onClick={() => onDetailTabChange(id)} type="button">{label}</button>
          ))}
        </nav>
        <div className="overview-tab-content">
          {detailTab === 'projects' && (
            <>
              <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['project']} onSelect={onSelectBreakdown} />
            </>
          )}
          {detailTab === 'models' && <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['model']} onSelect={onSelectBreakdown} />}
          {detailTab === 'sessions' && <><ConversationControls explorer={bundle.explorer} filters={filters} onChange={onFiltersChange} /><OverviewSessions explorer={bundle.explorer} onOpenSession={onOpenSession} /></>}
        </div>
      </section>
      <details className="overview-secondary panel">
        <summary>{t('overview.composition_details')}</summary>
        <LocalComposition usage={bundle.summary.usage.confirmed} />
      </details>
      <details className="overview-secondary panel">
        <summary>{t('overview.evidence_details')}</summary>
        <AttributionCoveragePanel coverage={bundle.summary.attributionCoverage} />
        <MissingAccountEstimatePanel estimate={bundle.summary.missingAccountEstimate} />
      </details>
    </>
  );
}
