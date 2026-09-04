import type { DashboardBundle, MetricKey } from '../../api/types';
import { AttributionCoveragePanel } from '../../components/AttributionCoveragePanel';
import { BreakdownPanel } from '../../components/BreakdownPanel';
import { OverviewSessions } from '../../components/Explorer';
import { MissingAccountEstimatePanel } from '../../components/MissingAccountEstimatePanel';
import { TokenOverview } from '../../components/TokenOverview';
import { ExplorerPulse } from '../../components/Explorer';
import { TrendAndTimeline } from '../../components/TrendAndTimeline';
import { compactNumber, formatPercent } from '../../lib';
import { useI18n } from '../../i18n';

export type OverviewDetailTab = 'projects' | 'models' | 'sessions';

interface OverviewPageProps {
  bundle: DashboardBundle;
  metric: MetricKey;
  detailTab: OverviewDetailTab;
  onDetailTabChange: (tab: OverviewDetailTab) => void;
  onOpenProject: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSelectBreakdown: (dimension: 'account' | 'project' | 'model', id: string) => void;
}

export function OverviewPage({
  bundle,
  metric,
  detailTab,
  onDetailTabChange,
  onOpenProject,
  onOpenSession,
  onSelectBreakdown,
}: OverviewPageProps) {
  const { t } = useI18n();
  const officialTotal = bundle.summary.official.totalTokens;
  const showReconciliation = metric === 'total'
    && bundle.summary.official.authoritativeForAccountTotal
    && officialTotal !== null;

  return (
    <>
      <ExplorerPulse explorer={bundle.explorer} summary={bundle.summary} metric={metric} />
      <AttributionCoveragePanel coverage={bundle.summary.attributionCoverage} />
      <MissingAccountEstimatePanel estimate={bundle.summary.missingAccountEstimate} />
      <TrendAndTimeline data={bundle.timeseries} explorer={bundle.explorer} metric={metric} onOpenProject={onOpenProject} />
      <TokenOverview summary={bundle.summary} metric={metric} />
      <section className="overview-tabs panel">
        <nav aria-label={t('overview.usage_details')}>
          {([['projects', t('components.explorer.projects')], ['models', t('overview.models')], ['sessions', 'Sessions']] as const).map(([id, label]) => (
            <button aria-pressed={detailTab === id} className={detailTab === id ? 'is-active' : ''} key={id} onClick={() => onDetailTabChange(id)} type="button">{label}</button>
          ))}
        </nav>
        <div className="overview-tab-content">
          {detailTab === 'projects' && (
            <>
              {showReconciliation && (
                <div className="project-tab-reconciliation">
                  <span>{t('overview.official_account_total')} <strong>{compactNumber(officialTotal)}</strong></span>
                  <span>{t('components.trend-and-timeline.local_project_sample')} <strong>{compactNumber(bundle.summary.usage.confirmed.total)} · {officialTotal ? formatPercent(bundle.summary.usage.confirmed.total / officialTotal) : '0%'}</strong></span>
                  <span>{t('overview.scope')} <strong>{t('overview.official_data_has_no_project_dimension_gaps')}</strong></span>
                </div>
              )}
              <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['project']} onSelect={onSelectBreakdown} />
            </>
          )}
          {detailTab === 'models' && <BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['model']} onSelect={onSelectBreakdown} />}
          {detailTab === 'sessions' && <OverviewSessions explorer={bundle.explorer} onOpenSession={onOpenSession} />}
        </div>
      </section>
    </>
  );
}
