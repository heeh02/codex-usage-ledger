import { useMemo, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type {
  ExplorerResponse,
  ExplorerSession,
  ExplorerSessionDetail,
  ExplorerSessionNode,
  MetricKey,
  PeriodKey,
  SummaryResponse,
  TokenUsage,
} from '../api/types';
import { compactNumber, exactNumber, formatDateTime, formatPercent, formatPeriodRange, metricLabel, metricValue, periodLabel } from '../lib';
import { useI18n } from '../i18n';
import type { AppPage } from '../page';
import { EmptyState } from './Ui';

function FolderIcon({ open = false }: { open?: boolean }) {
  return (
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <path d={open ? 'M2.8 7.6h14.4l-1.7 7.1H4.3L2.8 7.6Z' : 'M2.8 5.2h5l1.5 1.7h7.9v7.8H2.8V5.2Z'} />
    </svg>
  );
}

function OverviewIcon() {
  return (
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <rect x="3" y="3" width="5" height="5" rx="1" />
      <rect x="12" y="3" width="5" height="5" rx="1" />
      <rect x="3" y="12" width="5" height="5" rx="1" />
      <rect x="12" y="12" width="5" height="5" rx="1" />
    </svg>
  );
}

function ConversationIcon() {
  return (
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <path d="M3.2 4.2h13.6v9.2H9.1L5.3 16v-2.6H3.2V4.2Z" />
    </svg>
  );
}

function AccountIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="7" r="3" /><path d="M4.5 16c.5-3.2 2.3-4.8 5.5-4.8s5 1.6 5.5 4.8" /></svg>;
}

function QualityIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 2.8 16 5v4.6c0 3.7-2.2 6.2-6 7.6-3.8-1.4-6-3.9-6-7.6V5l6-2.2Z" /><path d="m7.2 9.8 1.8 1.8 3.8-4" /></svg>;
}

export function LedgerSidebar({
  explorer,
  selectedProject,
  page,
  period,
  onOverview,
  onProject,
  onAccounts,
  onQuality,
}: {
  explorer: ExplorerResponse | null;
  selectedProject: string;
  page: AppPage;
  period: SummaryResponse['period'] | null;
  onOverview: () => void;
  onProject: (projectId: string) => void;
  onAccounts: () => void;
  onQuality: () => void;
}) {
  const { t } = useI18n();
  type RankingSort = 'tokens' | 'growth' | 'recent' | 'rate' | 'sessions';
  const [rankingSort, setRankingSortState] = useState<RankingSort>(() => {
    const saved = window.localStorage.getItem('codex-usage-project-sort');
    return saved === 'growth' || saved === 'recent' || saved === 'rate' || saved === 'sessions' ? saved : 'tokens';
  });
  const rankingUsage = (project: ExplorerResponse['projects'][number]) => project.periodUsage;
  const displayUsage = (project: ExplorerResponse['projects'][number]) => rankingSort === 'rate' ? project.recent15Usage : rankingUsage(project);
  const previousUsage = (project: ExplorerResponse['projects'][number]) => project.previousPeriodUsage;
  const growth = (project: ExplorerResponse['projects'][number]) => {
    const previous = previousUsage(project).total;
    const current = rankingUsage(project).total;
    return previous > 0 ? (current - previous) / previous : current > 0 ? Number.POSITIVE_INFINITY : 0;
  };
  const rankedProjects = useMemo(() => [...(explorer?.projects ?? [])].sort((left, right) => {
    if (rankingSort === 'growth') return growth(right) - growth(left);
    if (rankingSort === 'recent') return Date.parse(right.lastActiveAt ?? '1970-01-01') - Date.parse(left.lastActiveAt ?? '1970-01-01');
    if (rankingSort === 'rate') return right.recent15Usage.total - left.recent15Usage.total;
    if (rankingSort === 'sessions') return (right.sessionCount + right.historicalSessionCount) - (left.sessionCount + left.historicalSessionCount);
    return rankingUsage(right).total - rankingUsage(left).total;
  }), [explorer, rankingSort]);
  const rankingTotal = rankedProjects.reduce((sum, project) => sum + displayUsage(project).total, 0);
  const officialRankingTotal = explorer?.stats.official.selectedPeriodTokens ?? null;
  const displayedRankingTotal = rankingTotal;
  const setRankingSort = (sort: RankingSort) => {
    setRankingSortState(sort);
    window.localStorage.setItem('codex-usage-project-sort', sort);
  };
  const rankingLabel = rankingSort === 'rate' ? t('components.explorer.last_15_minutes') : period ? periodLabel(period.key) : t('components.explorer.selected_period');
  const rankingWindow = rankingSort === 'rate' ? null : period;
  const standaloneConversation = rankedProjects.find((project) => project.kind === 'standalone_conversations');
  const unmatchedRecords = rankedProjects.find((project) => project.kind === 'unmatched_records');
  const showAccountCoverage = page === 'overview' || page === 'accounts';
  const projectFolders = rankedProjects.filter((project) => project.kind === 'project');
  const activeProjects = projectFolders.filter((project) => displayUsage(project).total > 0 || project.activeSessionCount > 0);
  const inactiveProjects = projectFolders.filter((project) => displayUsage(project).total === 0 && project.activeSessionCount === 0);
  const projectButton = (project: ExplorerResponse['projects'][number]) => (
    <button
      className={selectedProject === project.id ? 'sidebar-item project-item is-active' : 'sidebar-item project-item'}
      key={project.id}
      onClick={() => onProject(project.id)}
      title={project.kind === 'standalone_conversations' ? t('app.standalone_chats') : project.kind === 'unmatched_records' ? t('app.local_unmatched') : project.label}
      type="button"
    >
      {project.kind === 'standalone_conversations' ? <ConversationIcon /> : project.kind === 'unmatched_records' ? <OverviewIcon /> : <FolderIcon open={selectedProject === project.id} />}
      <span>{project.kind === 'standalone_conversations' ? t('app.standalone_chats') : project.kind === 'unmatched_records' ? t('app.local_unmatched') : project.label}</span>
      <small>{compactNumber(displayUsage(project).total)}</small>
    </button>
  );
  return (
    <aside className="ledger-sidebar">
      <div className="sidebar-brand">
        <div className="brand-mark" aria-hidden="true"><span /><span /><span /><span /></div>
        <div><strong>Codex Usage</strong><span>{t('components.explorer.local_ledger')}</span></div>
      </div>

      <nav className="sidebar-nav" aria-label={t('components.explorer.usage_navigation')}>
        <button className={page === 'overview' ? 'sidebar-item is-active' : 'sidebar-item'} onClick={onOverview} type="button">
          <OverviewIcon />
          <span>{t('app.overview')}</span>
          <small>{explorer?.stats.projectCount ?? '—'}</small>
        </button>

        <div className="sidebar-section-heading">
          <span>{t('components.explorer.local_work_evidence')} · {rankingLabel}</span>
          <small>{explorer ? `${projectFolders.length} ${t('components.explorer.projects_chats')} · ${compactNumber(displayedRankingTotal)}` : t('components.explorer.reading_trusted_snapshot')}</small>
        </div>
        {rankingWindow && <div className="sidebar-ranking-window"><span>{formatPeriodRange(rankingWindow)}</span>{rankingWindow.crossesMonth && <strong>{t('app.cross_month')}</strong>}<small>{t('app.local_attribution')}</small></div>}
        <label className="sidebar-sort-select"><span>{t('components.explorer.sort')}</span><select value={rankingSort} onChange={(event) => setRankingSort(event.target.value as RankingSort)}><option value="tokens">{t('components.explorer.token_usage')}</option><option value="growth">{t('components.explorer.fastest_growth')}</option><option value="recent">{t('components.explorer.recently_active')}</option><option value="rate">{t('components.explorer.last_15_minutes_3f237f')}</option><option value="sessions">{t('components.explorer.session_count')}</option></select></label>
        {showAccountCoverage && rankingSort !== 'rate' && explorer?.stats.official.totalIsLowerBound && officialRankingTotal !== null && (
          <div className="sidebar-account-lower-bound"><span>{t('components.explorer.synced')} {explorer.stats.official.knownAccountCount - explorer.stats.official.missingOfficialAccountCount}/{explorer.stats.official.knownAccountCount} {t('components.explorer.accounts')}</span><strong>{t('components.explorer.official')} ≥ {compactNumber(officialRankingTotal)}</strong></div>
        )}
        {showAccountCoverage && rankingSort !== 'rate' && officialRankingTotal !== null && officialRankingTotal > 0 && (
          <div className="sidebar-attribution-coverage">
            <div><span>{t('components.explorer.work_evidence')} {compactNumber(displayedRankingTotal)}</span><strong>{formatPercent(Math.min(displayedRankingTotal / officialRankingTotal, 1))}</strong></div>
            <i><span style={{ width: `${Math.min(displayedRankingTotal / officialRankingTotal, 1) * 100}%` }} /></i>
            <small>{t('components.explorer.relative_to_official_booked_usage_gap_not')}</small>
          </div>
        )}
        {standaloneConversation && (
          <div className="standalone-conversation-nav">
            {projectButton(standaloneConversation)}
            <small>{t('components.attribution-coverage-panel.remaining_after_project_resolution')} · {explorer?.stats.standaloneConversations.current ?? 0} {t('components.explorer.current')} · {explorer?.stats.standaloneConversations.historical ?? 0} {t('components.explorer.historical')} · {explorer?.stats.standaloneConversations.withLocalEvidence ?? 0}/{explorer?.stats.standaloneConversations.indexed ?? 0} {t('components.explorer.with_local_evidence')}</small>
          </div>
        )}
        <div className="sidebar-list-label">{t('components.explorer.projects')}</div>
        <div className="project-nav-list">
          {activeProjects.map(projectButton)}
          {inactiveProjects.length > 0 && <details className="inactive-projects"><summary>{t('components.explorer.inactive_projects')} · {inactiveProjects.length}</summary>{inactiveProjects.map(projectButton)}</details>}
        </div>
        <div className="sidebar-list-label">{t('components.explorer.accounts_diagnostics')}</div>
        <div className="sidebar-page-links">
          <button className={page === 'accounts' ? 'sidebar-item is-active' : 'sidebar-item'} onClick={onAccounts} type="button"><AccountIcon /><span>{t('app.accounts_quota')}</span><small>{explorer?.stats.official.knownAccountCount ?? '—'}</small></button>
          <button className={page === 'quality' ? 'sidebar-item is-active' : 'sidebar-item'} onClick={onQuality} type="button"><QualityIcon /><span>{t('app.data_quality')}</span><small>{!explorer ? '—' : unmatchedRecords && displayUsage(unmatchedRecords).total > 0 ? t('components.explorer.review') : t('components.explorer.view')}</small></button>
        </div>
      </nav>

      <div className="sidebar-footer">
        <div className="sidebar-live"><i /><span>{t('components.explorer.local_service')}</span><strong>{explorer ? `${explorer.stats.activeSessions} ${t('components.explorer.active')}` : t('components.explorer.connecting')}</strong></div>
        <div className="sidebar-counters">
          <span>{explorer ? `${t('components.explorer.current_codex_directory')} ${explorer.stats.sessionCount} sessions` : t('components.explorer.reading_session_directory')}</span>
          <span>{explorer ? `${t('components.explorer.current_codex_directory')} ${explorer.stats.subagentCount} subagents` : t('components.explorer.reading_subagent_directory')}</span>
        </div>
        {explorer && (explorer.stats.historicalSessionCount > 0 || explorer.stats.historicalSubagentCount > 0) && <div className="sidebar-history-count">{t('components.explorer.app_history_retains')} {explorer.stats.historicalSessionCount} sessions · {explorer.stats.historicalSubagentCount} subagents</div>}
      </div>
    </aside>
  );
}

function PulseMetric({ label, value, detail, tone = 'default' }: { label: string; value: string; detail: string; tone?: string }) {
  return (
    <article className={`pulse-metric tone-${tone}`}>
      <div><i /><span>{label}</span></div>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}

export function ExplorerPulse({ explorer, summary, metric }: { explorer: ExplorerResponse; summary: SummaryResponse; metric: MetricKey }) {
  const { t } = useI18n();
  const { stats } = explorer;
  const accountTotal = summary.metrics.accountTotal;
  const selected = accountTotal.value;
  const isLowerBound = accountTotal.status === 'lower_bound';
  const displayDetail = accountTotal.status === 'exact'
    ? t('components.explorer.official_account_total_all_devices')
    : accountTotal.source === 'reconciled'
      ? t('components.explorer.official_booked_local_tail_lower_bound')
      : accountTotal.status === 'lower_bound'
        ? t('components.explorer.local_observable_lower_bound')
        : t('components.explorer.account_total_unknown');
  const previous = summary.official.displayPreviousTotalTokens;
  const delta = summary.official.displayDeltaPercent;
  const start = new Date(summary.period.start).getTime();
  const end = new Date(summary.period.end).getTime();
  const elapsedDays = Number.isFinite(start) && Number.isFinite(end) ? Math.max((end - start) / 86_400_000, 1) : 1;
  const officialAverage = selected !== null && summary.official.points.length > 0;
  const officialPointCount = summary.official.points.length + (summary.official.localTailTokens > 0 ? 1 : 0);
  const averageDivisor = officialAverage
    ? officialPointCount
    : summary.period.key === 'weeks12'
      ? Math.max(elapsedDays / 7, 1)
      : summary.period.key === 'months12' || summary.period.key === 'lifetime'
        ? Math.max(elapsedDays / 30.44, 1)
        : elapsedDays;
  const averageGrain = officialAverage ? summary.official.granularity : summary.period.key === 'weeks12' ? 'week' : summary.period.key === 'months12' || summary.period.key === 'lifetime' ? 'month' : 'day';
  const averageLabel = averageGrain === 'week' ? t('components.explorer.weekly_avg') : averageGrain === 'month' ? t('components.explorer.monthly_avg') : t('components.explorer.daily_avg');
  const recent = metricValue(stats.localRecent15Minutes, metric, stats.localRecent15Events);
  const averageAvailable = selected !== null && accountTotal.coverage.complete;
  return (
    <section className="explorer-pulse" aria-label="Codex usage pulse">
      <PulseMetric label={t('components.attribution-coverage-panel.account_total')} value={selected === null ? '—' : `${isLowerBound ? '≥ ' : ''}${compactNumber(selected)}`} detail={displayDetail} tone="blue" />
      <PulseMetric label={t('components.explorer.vs_previous_period')} value={delta === null ? '—' : `${delta >= 0 ? '+' : ''}${formatPercent(delta)}`} detail={previous === null ? t('components.explorer.no_comparable_coverage') : summary.official.previousDisplayIsLowerBound || isLowerBound ? t('components.explorer.current_or_previous_period_is_only_a') : `${t('components.explorer.previous')} ${compactNumber(previous)}`} tone={delta !== null && delta > 0 ? 'orange' : 'green'} />
      <PulseMetric label={averageLabel} value={averageAvailable && selected !== null ? `${isLowerBound ? '≥ ' : ''}${compactNumber(selected / averageDivisor)}` : '—'} detail={averageAvailable ? `${t('components.explorer.across')} ${officialPointCount} ${t('components.explorer.covered_tail')} ${averageGrain === 'week' ? t('components.explorer.weeks') : averageGrain === 'month' ? t('components.explorer.months') : t('components.explorer.days')}` : t('components.explorer.account_coverage_is_insufficient_for_a_comparable')} tone="green" />
      <PulseMetric label={t('components.explorer.last_15_minutes_3f237f')} value={compactNumber(recent)} detail={t('components.explorer.local_attributed_activity_not_quota_burn_rate')} tone="orange" />
      <PulseMetric label={t('components.explorer.local_composition_sample')} value={compactNumber(summary.usage.confirmed.total)} detail={t('components.explorer.four_bucket_composition_covers_matched_local_events')} tone="purple" />
    </section>
  );
}

function ScopeMetric({ label, usage, detail }: { label: string; usage: TokenUsage; detail: string }) {
  return (
    <article className="scope-metric">
      <span>{label}</span>
      <strong>{compactNumber(usage.total)}</strong>
      <small>{detail}</small>
    </article>
  );
}

function MiniComposition({ usage }: { usage: TokenUsage }) {
  const { t } = useI18n();
  const cached = usage.total ? usage.cached / usage.total : 0;
  const cacheWrite = usage.total ? usage.cacheWrite / usage.total : 0;
  const uncached = usage.total ? usage.uncached / usage.total : 0;
  const output = usage.total ? usage.output / usage.total : 0;
  return (
    <div className="mini-composition" aria-label={t('components.explorer.token_composition')}>
      <span className="mini-uncached" style={{ width: `${uncached * 100}%` }} />
      <span className="mini-cached" style={{ width: `${cached * 100}%` }} />
      <span className="mini-cache-write" style={{ width: `${cacheWrite * 100}%` }} />
      <span className="mini-output" style={{ width: `${output * 100}%` }} />
    </div>
  );
}

function SessionRow({ session, onOpen }: { session: ExplorerSession; onOpen: () => void }) {
  const { t } = useI18n();
  return (
    <button className="session-row" onClick={onOpen} type="button">
      <div className="session-status-icon"><i className={session.active ? 'is-live' : ''} /></div>
      <div className="session-main">
        <div className="session-title-row"><strong>{session.title}</strong>{!session.presentInCodex ? <span className="is-historical">{t('components.explorer.history')}</span> : session.kind === 'orphan_subagent' ? <span className="is-unlinked">{t('components.explorer.unlinked')}</span> : session.active && <span>{t('components.explorer.active_b71b76')}</span>}</div>
        <small>{!session.presentInCodex ? `${t('components.explorer.deleted_from_the_current_codex_directory')} · ` : ''}{session.kind === 'orphan_subagent' ? `${t('components.explorer.parent_session_unknown')} · ` : ''}{session.archived ? 'Archived · ' : ''}{session.model ?? t('app.model_unknown')} · {formatDateTime(session.updatedAt)} · {session.eventCount} events</small>
        <MiniComposition usage={session.treeUsage} />
      </div>
      <div className="session-agents">
        <strong>{session.subagentCount}</strong>
        <span>subagents</span>
      </div>
      <div className="session-usage">
        <strong>{compactNumber(session.treeUsage.total)}</strong>
        <span>{t('components.explorer.with_subtree')}</span>
        <small>{t('components.explorer.own')} {compactNumber(session.ownUsage.total)}</small>
      </div>
      <span className="session-chevron">›</span>
    </button>
  );
}

export function OverviewSessions({ explorer, onOpenSession }: { explorer: ExplorerResponse; onOpenSession: (sessionId: string) => void }) {
  const { t } = useI18n();
  return (
    <section className="session-browser overview-session-browser panel">
      <header className="session-browser-heading">
        <div><p className="eyebrow">Top sessions</p><h2>{t('components.explorer.session_subagent_sources')}</h2></div>
        <div className="session-browser-summary"><span>{explorer.stats.sessionCount} {t('app.current_sessions')}</span><span>{explorer.stats.subagentCount} {t('components.explorer.current_subagents')}</span><span>{explorer.stats.historicalSessionCount + explorer.stats.historicalSubagentCount} {t('components.explorer.historical_nodes')}</span></div>
      </header>
      {explorer.sessions.length ? (
        <div className="session-list">{explorer.sessions.map((session) => <SessionRow key={session.id} session={session} onOpen={() => onOpenSession(session.id)} />)}</div>
      ) : <EmptyState text={t('components.explorer.no_sessions_to_show_for_the_current')} />}
    </section>
  );
}

export function ProjectExplorer({
  explorer,
  period,
  periodWindow,
  scopeKind = 'project',
  tab,
  onTabChange,
  trend,
  modelBreakdown,
  onOpenSession,
}: {
  explorer: ExplorerResponse;
  period: PeriodKey;
  periodWindow: SummaryResponse['period'];
  scopeKind?: ExplorerResponse['projects'][number]['kind'];
  tab: 'overview' | 'sessions';
  onTabChange: (tab: 'overview' | 'sessions') => void;
  trend?: ReactNode;
  modelBreakdown?: ReactNode;
  onOpenSession: (sessionId: string) => void;
}) {
  const { t } = useI18n();
  const { stats } = explorer;
  const standalone = scopeKind === 'standalone_conversations';
  const unmatched = scopeKind === 'unmatched_records';
  const scopeLabel = standalone ? t('app.standalone_chats') : unmatched ? t('components.explorer.unassigned_tokens') : t('components.explorer.project');
  const periodSessions = explorer.sessions.filter((session) => session.treeUsage.total > 0 || session.active);
  return (
    <div className="explorer-page">
      <section className="scope-metric-grid">
        <article className="scope-metric is-primary-scope">
          <div><span>{periodLabel(period)} {t('components.explorer.attributed_usage')}</span><b>{t('components.explorer.source_selected_lower_bound')}</b></div>
          <strong>≥ {compactNumber(stats.selectedPeriod.total)}</strong>
          <small>{formatPeriodRange(periodWindow)} · {scopeLabel}</small>
        </article>
        <article className="scope-metric">
          <span>{t('components.explorer.input_output')}</span>
          <strong>{compactNumber(stats.selectedPeriod.input)}</strong>
          <small>{t('components.explorer.output')} {compactNumber(stats.selectedPeriod.output)} · Reasoning {compactNumber(stats.selectedPeriod.reasoning)}</small>
        </article>
        <article className="scope-metric">
          <span>{standalone ? t('components.explorer.chat_scale') : t('components.explorer.session_scale')}</span>
          <strong>{stats.sessionCount + stats.historicalSessionCount}</strong>
          <small>{stats.sessionCount} {t('components.explorer.current')} · {stats.historicalSessionCount} {t('components.explorer.historical')} · {stats.subagentCount} subagents</small>
        </article>
        <article className="scope-metric live-scope">
          <span>{t('components.explorer.last_15_minutes_3f237f')}</span>
          <strong>{compactNumber(stats.localRecent15Minutes.total)}</strong>
          <small>{t('components.explorer.local_attributed_usage')} · {stats.activeSessions} active sessions</small>
        </article>
      </section>

      <aside className="project-evidence-boundary">
        <strong>{t('components.explorer.only_one_record_source_is_selected_per')}</strong>
        <span>{t('components.explorer.reconstructed_history_is_selected_when_it_is')}</span>
      </aside>

      <nav className="project-view-tabs" aria-label={`${scopeLabel} ${t('components.explorer.details')}`}>
        <button aria-pressed={tab === 'overview'} className={tab === 'overview' ? 'is-active' : ''} onClick={() => onTabChange('overview')} type="button">{t('components.explorer.overview')}</button>
        {!unmatched && <button aria-pressed={tab === 'sessions'} className={tab === 'sessions' ? 'is-active' : ''} onClick={() => onTabChange('sessions')} type="button">{standalone ? t('components.explorer.chats') : 'Sessions'} · {stats.sessionCount + stats.historicalSessionCount}</button>}
      </nav>

      {tab === 'overview' && <div className="project-dashboard-row">
        {trend}
        <section className="project-insights">
          <article className="project-composition panel">
            <header><div><p className="eyebrow">{t('components.explorer.token_composition')}</p><h2>{standalone ? t('components.explorer.standalone_chat_composition') : unmatched ? t('components.explorer.unmatched_record_composition') : t('components.explorer.project_token_composition')}</h2></div><strong>{compactNumber(stats.selectedPeriod.total)} {t('components.explorer.local_sample')} · {t('components.explorer.write_field_coverage')} {formatPercent(stats.selectedPeriod.cacheWriteCoverage)}</strong></header>
            <MiniComposition usage={stats.selectedPeriod} />
            <div><span>{stats.selectedPeriod.cacheWriteCoverage >= 0.999 ? t('components.explorer.input') : t('components.explorer.input_unsplit')} <strong>{compactNumber(stats.selectedPeriod.uncached)}</strong></span><span>{t('components.explorer.cache_read')} <strong>{compactNumber(stats.selectedPeriod.cached)}</strong></span><span>{t('components.explorer.cache_write')} <strong>{stats.selectedPeriod.cacheWriteCoverage > 0 ? `${stats.selectedPeriod.cacheWriteCoverage >= 0.999 ? '' : '≥ '}${compactNumber(stats.selectedPeriod.cacheWrite)}` : '—'}</strong></span><span>{t('components.explorer.output')} <strong>{compactNumber(stats.selectedPeriod.output)}</strong></span><span>{t('components.explorer.reasoning_inside_output')} <strong>{compactNumber(stats.selectedPeriod.reasoning)}</strong></span></div>
          </article>
          {modelBreakdown}
        </section>
      </div>}

      {!unmatched && tab === 'sessions' && <section className="session-browser panel">
        <header className="session-browser-heading">
          <div>
            <p className="eyebrow">{standalone ? 'Standalone conversations' : 'Sessions in project'}</p>
            <h2>{standalone ? t('components.explorer.standalone_chats_subagents') : t('components.explorer.sessions_subagents')}</h2>
          </div>
          <div className="session-browser-summary">
            <span>{stats.sessionCount} {t('components.explorer.current')}</span>
            {stats.historicalSessionCount > 0 && <span>{stats.historicalSessionCount} {t('components.explorer.historical')}</span>}
            <span>{stats.subagentCount} subagents</span>
            {stats.orphanSubagentCount > 0 && <span>{stats.orphanSubagentCount} unlinked</span>}
            <span>{periodLabel(period)}</span>
          </div>
        </header>
        {periodSessions.length ? (
          <div className="session-list">
            {periodSessions.map((session) => <SessionRow key={session.id} session={session} onOpen={() => onOpenSession(session.id)} />)}
          </div>
        ) : (
          <EmptyState text={standalone ? t('components.explorer.no_standalone_chats_have_usage_or_activity') : t('components.explorer.no_root_sessions_have_usage_or_activity')} />
        )}
      </section>}
      <p className="explorer-footnote">{standalone ? t('components.explorer.standalone_evidence_footnote', {
        withEvidence: stats.standaloneConversations.withLocalEvidence,
        indexed: stats.standaloneConversations.indexed,
      }) : unmatched ? t('components.explorer.these_local_token_facts_do_not_belong') : t('components.explorer.project_and_session_values_are_local_attribution')}</p>
    </div>
  );
}

function nodeCacheRate(node: ExplorerSessionNode): string {
  return node.ownUsage.input ? formatPercent(node.ownUsage.cached / node.ownUsage.input) : '0%';
}

function AgentNodeRow({ node, isRoot, isSelected, metric, onSelect }: { node: ExplorerSessionNode; isRoot: boolean; isSelected: boolean; metric: MetricKey; onSelect: () => void }) {
  const { t } = useI18n();
  const ownValue = metricValue(node.ownUsage, metric, node.eventCount);
  const treeValue = metricValue(node.subtreeUsage, metric, node.subtreeEventCount);
  return (
    <button type="button" role="treeitem" aria-level={node.relativeDepth + 1} aria-selected={isSelected} aria-label={`${node.title}, ${t('components.explorer.own_72d733')} ${compactNumber(ownValue)}, ${t('components.explorer.with_subtree_8e7f6e')} ${compactNumber(treeValue)}`} onClick={onSelect} className={`${isRoot ? 'agent-node is-root' : 'agent-node'}${isSelected ? ' is-selected' : ''}`} style={{ '--node-depth': Math.min(node.relativeDepth, 5) } as CSSProperties}>
      <div className="agent-tree-guide"><i /></div>
      <div className="agent-identity">
        <div><strong>{node.title}</strong>{!node.presentInCodex && <span className="is-historical">{t('components.explorer.history')}</span>}{node.agentNickname && <span>{node.agentNickname}</span>}</div>
        <small>{node.model ?? t('app.model_unknown')}{node.agentRole ? ` · ${node.agentRole}` : ''} · {formatDateTime(node.updatedAt)}</small>
        <MiniComposition usage={node.ownUsage} />
      </div>
      <div className="agent-cache"><strong>{nodeCacheRate(node)}</strong><span>cache</span></div>
      <div className="agent-own"><strong>{compactNumber(ownValue)}</strong><span>{t('components.explorer.own')}</span></div>
      <div className="agent-tree-total"><strong>{compactNumber(treeValue)}</strong><span>{t('components.explorer.subtree')}</span></div>
    </button>
  );
}

export type SessionScope = 'own' | 'tree';
export type SessionSort = 'hierarchy' | 'own' | 'tree' | 'recent';
export interface SessionViewState {
  sessionId: string | null;
  search: string;
  sort: SessionSort;
  selectedNode: string | null;
  scope: SessionScope;
}

export function SessionExplorer({ detail, metric, view, onViewChange }: {
  detail: ExplorerSessionDetail | null;
  metric: MetricKey;
  view: SessionViewState;
  onViewChange: (next: SessionViewState) => void;
}) {
  const { t } = useI18n();
  const { search, sort, selectedNode, scope } = view;
  const patchView = (patch: Partial<SessionViewState>) => onViewChange({ ...view, ...patch });
  const visibleNodes = useMemo(() => {
    if (!detail) return [];
    let nodes = [...detail.nodes];
    if (selectedNode) {
      const included = new Set([selectedNode]);
      let changed = true;
      while (changed) {
        changed = false;
        for (const node of nodes) {
          if (node.parentId && included.has(node.parentId) && !included.has(node.id)) {
            included.add(node.id);
            changed = true;
          }
        }
      }
      nodes = detail.nodes.filter((node) => included.has(node.id));
    }
    const query = search.trim().toLocaleLowerCase();
    if (query) {
      const byId = new Map(detail.nodes.map((node) => [node.id, node]));
      const included = new Set(nodes
        .filter((node) => `${node.title} ${node.agentNickname ?? ''} ${node.model ?? ''}`.toLocaleLowerCase().includes(query))
        .map((node) => node.id));
      for (const id of [...included]) {
        let parentId = byId.get(id)?.parentId ?? null;
        while (parentId) {
          included.add(parentId);
          parentId = byId.get(parentId)?.parentId ?? null;
        }
      }
      nodes = nodes.filter((node) => included.has(node.id));
    }
    if (sort !== 'hierarchy' && !query) {
      nodes.sort((left, right) => {
        if (sort === 'recent') return Date.parse(right.updatedAt) - Date.parse(left.updatedAt);
        const leftValue = metricValue(sort === 'own' ? left.ownUsage : left.subtreeUsage, metric, sort === 'own' ? left.eventCount : left.subtreeEventCount);
        const rightValue = metricValue(sort === 'own' ? right.ownUsage : right.subtreeUsage, metric, sort === 'own' ? right.eventCount : right.subtreeEventCount);
        return rightValue - leftValue;
      });
    }
    return nodes;
  }, [detail, metric, search, selectedNode, sort]);
  if (!detail) return <EmptyState text={t('components.explorer.this_session_was_not_found_and_may')} />;
  const focusedTimeline = scope === 'own' ? detail.ownSamplingTimeline : detail.samplingTimeline;
  const timelineMax = Math.max(...focusedTimeline.map((point) => metricValue(point.usage, metric, point.events)), 1);
  const focusedUsage = scope === 'own' ? detail.ownUsage : detail.treeUsage;
  const selectedValue = metricValue(focusedUsage, metric, scope === 'own' ? detail.nodes[0]?.eventCount ?? 0 : detail.nodes.reduce((sum, node) => sum + node.eventCount, 0));
  return (
    <section className="explorer-page session-detail-page" aria-labelledby="session-detail-heading">
      <header className="session-detail-hero">
        <div className="session-detail-title">
          <span className="session-detail-icon">◎</span>
          <div><p className="eyebrow">{t('components.explorer.session_usage_tree')}</p><h2 id="session-detail-heading">{detail.title}</h2><span>{!detail.presentInCodex ? `${t('components.explorer.deleted_from_current_codex_directory_retained_in')} · ` : ''}{detail.model ?? t('app.model_unknown')} · {formatDateTime(detail.updatedAt)}</span></div>
        </div>
        <div className="session-detail-actions">
          <div className="session-scope-toggle" role="group" aria-label={t('components.explorer.session_usage_scope')}><button aria-pressed={scope === 'own'} className={scope === 'own' ? 'is-active' : ''} onClick={() => patchView({ scope: 'own' })} type="button">{t('components.explorer.own')}</button><button aria-pressed={scope === 'tree'} className={scope === 'tree' ? 'is-active' : ''} onClick={() => patchView({ scope: 'tree' })} type="button">{t('components.explorer.subtree')}</button></div>
          <div className="session-detail-total" aria-live="polite"><span>{scope === 'own' ? t('components.explorer.current_session_only') : t('components.explorer.includes_all_subagents')} · {metricLabel(metric)}</span><strong>{compactNumber(selectedValue)}</strong><small>{exactNumber(selectedValue)} {metric === 'requests' ? 'requests' : 'tokens'}</small></div>
        </div>
      </header>
      <section className="scope-metric-grid session-scope-grid">
        <ScopeMetric label={t('components.explorer.session_own')} usage={detail.ownUsage} detail={t('components.explorer.current_session_only_64938d')} />
        <ScopeMetric label={t('components.explorer.entire_task_tree')} usage={detail.treeUsage} detail={t('components.explorer.includes_all_descendants')} />
        <article className="scope-metric"><span>Subagents</span><strong>{detail.subagentCount}</strong><small>{t('components.explorer.loaded')} {detail.nodes.length} {t('components.explorer.nodes')}</small></article>
        <article className="scope-metric"><span>{scope === 'own' ? t('components.explorer.own_four_bucket_composition') : t('components.explorer.subtree_four_bucket_composition')}</span><strong>{compactNumber(focusedUsage.total)}</strong><small>{t('components.explorer.input')} {compactNumber(focusedUsage.uncached)} · {t('components.explorer.read')} {compactNumber(focusedUsage.cached)} · {t('components.explorer.write')} {focusedUsage.cacheWriteCoverage > 0 ? `${focusedUsage.cacheWriteCoverage >= 0.999 ? '' : '≥ '}${compactNumber(focusedUsage.cacheWrite)}` : '—'} · {t('components.explorer.output')} {compactNumber(focusedUsage.output)}</small></article>
      </section>
      {detail.officialThreadUsage && (
        <section className="official-thread-panel panel">
          <header><div><p className="eyebrow">{t('components.explorer.official_thread_calibration')}</p><h2>{t('components.explorer.official_session_calibration')}</h2></div><span>{t('components.account-panel.updated')} {formatDateTime(detail.officialThreadUsage.observedAt)}</span></header>
          <div className="official-thread-groups">
            {detail.officialThreadUsage.groups.length ? detail.officialThreadUsage.groups.map((group, index) => (
              <article key={`${group.model ?? 'unknown'}-${index}`}>
                <div><strong>{group.model ?? t('app.model_unknown')}</strong><span>{group.reasoningEffort ?? t('components.explorer.default_reasoning')} · {group.speed ?? t('components.explorer.standard_speed')}</span></div>
                <div><strong>{group.totalTokens === null ? '—' : compactNumber(group.totalTokens)}</strong><span>total</span></div>
                <div><strong>{group.cachedInputTokens === null ? '—' : compactNumber(group.cachedInputTokens)}</strong><span>{t('components.explorer.cache_read')} · {t('components.explorer.write_58af22')} {group.cacheWriteInputTokens === null ? '—' : compactNumber(group.cacheWriteInputTokens)}</span></div>
                <div><strong>{(group.estimatedUsageCreditsMicros / 1_000_000).toFixed(2)}</strong><span>credits</span></div>
              </article>
            )) : <EmptyState text={t('components.explorer.official_thread_quota_was_returned_without_model')} />}
          </div>
        </section>
      )}
      {!detail.officialThreadUsage && <div className="session-calibration-note">{t('components.explorer.the_official_account_total_remains_valid_the')}</div>}
      <section className="session-sampling-panel panel" aria-labelledby="sampling-trajectory-heading">
        <header><div><p className="eyebrow">{t('components.explorer.valid_usage')}</p><h2 id="sampling-trajectory-heading">{t('components.explorer.usage_trajectory')}</h2></div><span>{scope === 'own' ? t('components.explorer.own') : t('components.explorer.subtree')} · {detail.samplingGrain === 'hour' ? t('components.explorer.hour') : detail.samplingGrain === 'day' ? t('components.explorer.day') : detail.samplingGrain === 'week' ? t('components.explorer.week') : t('components.explorer.month')} · {metricLabel(metric)}</span></header>
        {focusedTimeline.length ? (
          <div className="session-sampling-bars" aria-label={`${scope === 'own' ? t('components.explorer.session_own') : t('components.explorer.session_with_subtree')} ${t('components.explorer.usage_trajectory_9fe0cc')}`}>
            {focusedTimeline.map((point) => {
              const value = metricValue(point.usage, metric, point.events);
              return <i key={point.bucket} title={`${point.bucket}: ${exactNumber(value)}`}><span style={{ height: `${Math.max((value / timelineMax) * 100, value > 0 ? 4 : 0)}%` }} /></i>;
            })}
          </div>
        ) : <EmptyState text={t('components.explorer.no_valid_attributed_usage_trajectory_exists_for')} />}
        {focusedTimeline.length > 0 && <table className="sr-only"><caption>{scope === 'own' ? t('components.explorer.session_own') : t('components.explorer.session_with_subtree')} {t('components.explorer.valid_usage_trajectory_data')}</caption><thead><tr><th>{t('components.explorer.time')}</th><th>{t('components.explorer.usage')}</th><th>{t('components.explorer.requests')}</th></tr></thead><tbody>{focusedTimeline.map((point) => <tr key={point.bucket}><td>{point.bucket}</td><td>{exactNumber(metricValue(point.usage, metric, point.events))}</td><td>{point.events}</td></tr>)}</tbody></table>}
      </section>
      <section className="agent-tree-panel panel" aria-labelledby="agent-tree-heading">
        <header className="agent-tree-heading">
          <div><p className="eyebrow">{t('components.explorer.hierarchy')}</p><h2 id="agent-tree-heading">{t('components.explorer.session_subagent_usage')}</h2></div>
          <div className="agent-tree-controls">
            <input aria-label={t('components.explorer.search_subagents')} placeholder={t('components.explorer.search_subagents')} value={search} onChange={(event) => patchView({ search: event.target.value })} />
            <select aria-label={t('components.explorer.subagent_sort')} value={sort} onChange={(event) => patchView({ sort: event.target.value as SessionSort })}>
              <option value="hierarchy">{t('components.explorer.hierarchy_183fcb')}</option><option value="own">{t('components.explorer.own_usage')}</option><option value="tree">{t('components.explorer.with_subtree')}</option><option value="recent">{t('components.explorer.recent_activity')}</option>
            </select>
            {selectedNode && <button type="button" onClick={() => patchView({ selectedNode: null })}>{t('components.explorer.show_full_tree')}</button>}
          </div>
          <div className="agent-column-labels"><span>{t('components.explorer.cache')}</span><span>{t('components.explorer.own')}</span><span>{t('components.explorer.subtree')}</span></div>
        </header>
        <div className="agent-tree-list" role="tree" aria-label={t('components.explorer.session_and_subagent_hierarchy')} aria-live="polite">
          {visibleNodes.map((node) => <AgentNodeRow key={node.id} node={node} isRoot={node.id === detail.id} isSelected={node.id === selectedNode} metric={metric} onSelect={() => patchView({ selectedNode: node.id === selectedNode ? null : node.id })} />)}
        </div>
        {visibleNodes.length === 0 && <EmptyState text={t('components.explorer.no_matching_sessions_or_subagents_were_found')} />}
        {visibleNodes.length > 0 && <table className="sr-only"><caption>{t('components.explorer.session_and_subagent_usage_table')}</caption><thead><tr><th>{t('components.explorer.level')}</th><th>{t('components.explorer.name')}</th><th>{t('components.explorer.model')}</th><th>{t('components.explorer.own_usage')}</th><th>{t('components.explorer.subtree')}</th></tr></thead><tbody>{visibleNodes.map((node) => <tr key={node.id}><td>{node.relativeDepth + 1}</td><td>{node.title}</td><td>{node.model ?? t('app.model_unknown')}</td><td>{exactNumber(metricValue(node.ownUsage, metric, node.eventCount))}</td><td>{exactNumber(metricValue(node.subtreeUsage, metric, node.subtreeEventCount))}</td></tr>)}</tbody></table>}
        {detail.truncated && <div className="tree-truncated">{t('components.explorer.this_task_tree_is_large_the_latest')}</div>}
      </section>
      <div className="accounting-note"><i />{t('components.explorer.own_includes_only_daily_deduplicated_records_for')}</div>
    </section>
  );
}
