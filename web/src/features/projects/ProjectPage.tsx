import type { DashboardBundle, DashboardFilters, MetricKey } from '../../api/types';
import { BreakdownPanel } from '../../components/BreakdownPanel';
import { ProjectExplorer } from '../../components/Explorer';
import { UsageTrendPanel } from '../../components/TrendAndTimeline';
import { useI18n } from '../../i18n';
import type { AppPage } from '../../page';

interface ProjectPageProps {
  bundle: DashboardBundle;
  page: Extract<AppPage, 'project' | 'conversation' | 'unmatched'>;
  projectId: string;
  metric: MetricKey;
  period: DashboardFilters['period'];
  tab: 'overview' | 'sessions';
  onTabChange: (tab: 'overview' | 'sessions') => void;
  onOpenSession: (sessionId: string) => void;
  onSelectBreakdown: (dimension: 'account' | 'project' | 'model', id: string) => void;
}

export function ProjectPage({
  bundle,
  page,
  projectId,
  metric,
  period,
  tab,
  onTabChange,
  onOpenSession,
  onSelectBreakdown,
}: ProjectPageProps) {
  const { t } = useI18n();
  const selectedProject = bundle.explorer.projects.find((project) => project.id === projectId);
  const title = page === 'conversation'
    ? t('projects.standalone_conversation_trend')
    : page === 'unmatched'
      ? t('projects.unassigned_trend')
      : t('projects.project_trend');

  return (
    <ProjectExplorer
      key={selectedProject?.id ?? projectId}
      explorer={bundle.explorer}
      period={period}
      periodWindow={bundle.summary.period}
      scopeKind={selectedProject?.kind ?? 'project'}
      tab={tab}
      onTabChange={onTabChange}
      trend={<UsageTrendPanel data={bundle.timeseries} metric={metric} title={title} allowProjectCompare={false} className="project-usage-trend" />}
      modelBreakdown={<BreakdownPanel data={bundle.breakdowns} metric={metric} dimensions={['model']} onSelect={onSelectBreakdown} />}
      onOpenSession={onOpenSession}
    />
  );
}
