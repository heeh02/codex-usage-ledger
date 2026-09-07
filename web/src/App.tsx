import { useEffect, useMemo, useRef, useState } from 'react';
import { createLedgerApi, loadDashboardBundle } from './api/client';
import type { DashboardBundle, DashboardFilters } from './api/types';
import {
  LedgerSidebar,
  type SessionViewState,
} from './components/Explorer';
import { CollectionProgress, DataStatusStrip, ErrorState, FilterBar, LoadingState } from './components/Ui';
import { AccountsPage } from './features/accounts/AccountsPage';
import { OverviewPage, type OverviewDetailTab } from './features/overview/OverviewPage';
import { ProjectPage } from './features/projects/ProjectPage';
import { QualityPage } from './features/quality/QualityPage';
import { SessionPage } from './features/sessions/SessionPage';
import { ConversationsPage } from './features/conversations/ConversationsPage';
import { ModelsPage } from './features/models/ModelsPage';
import { formatTokenMillions, formatDateTime, formatPeriodRange } from './lib';
import { useI18n } from './i18n';
import { requestNativePngExport } from './nativeBridge';
import type { AppPage } from './page';
import { exportUsageCsv, exportUsageJson } from './export';
import { runScopedRequest } from './shared/requestLifecycle';
import { parseChangeRevision } from './shared/changeRevision';
import { oneOf, readSessionObject, writeSessionObjects } from './shared/sessionPreferences';
import { restoreDashboardFilters } from './shared/dashboardPreferences';
import { AccountSwitcher, accountScopeLabel } from './components/AccountSwitcher';
import { requestFailureMessage } from './shared/requestFailureMessage';
import { LedgerRequestError } from './api/errors';
import { parentSessionFilters, type SessionTrailEntry } from './shared/sessionTrail';

const INITIAL_FILTERS: DashboardFilters = {
  account: 'all',
  project: 'all',
  model: 'all',
  period: 'rolling30',
  session: 'all',
  metric: 'total',
  grain: 'auto',
};

const INITIAL_SESSION_VIEW: SessionViewState = {
  sessionId: null,
  search: '',
  sort: 'hierarchy',
  selectedNode: null,
  scope: 'tree',
};

function restoreFilters(): DashboardFilters {
  return restoreDashboardFilters(INITIAL_FILTERS);
}

function App() {
  const { language, setLanguage, t } = useI18n();
  const api = useMemo(() => createLedgerApi(), []);
  const [filters, setFilters] = useState<DashboardFilters>(restoreFilters);
  const [appliedFilters, setAppliedFilters] = useState<DashboardFilters>(restoreFilters);
  const [bundle, setBundle] = useState<DashboardBundle | null>(null);
  const [loading, setLoading] = useState(true);
  const [requestFailure, setRequestFailure] = useState<unknown>(null);
  const error = requestFailure === null ? '' : requestFailureMessage(requestFailure, t);
  const timePrecisionFailure = requestFailure instanceof LedgerRequestError && requestFailure.code === 'insufficient_time_precision';
  const [refreshKey, setRefreshKey] = useState(0);
  const [manualRefreshing, setManualRefreshing] = useState(false);
  const [refreshFeedback, setRefreshFeedback] = useState('');
  const [officialSyncing, setOfficialSyncing] = useState(false);
  const [officialSyncFailed, setOfficialSyncFailed] = useState(false);
  const [accountView, setAccountView] = useState<'usage' | 'history'>(() => readSessionObject('ledger.accountView', { value: 'usage' as const }, { value: oneOf('usage', 'history') }).value);
  const [detailTab, setDetailTab] = useState<OverviewDetailTab>(() => readSessionObject('ledger.overviewTab', { value: 'projects' as const }, { value: oneOf('projects', 'models', 'sessions') }).value);
  const [projectDetailTab, setProjectDetailTab] = useState<'overview' | 'sessions'>(() => readSessionObject('ledger.projectTab', { value: 'overview' as const }, { value: oneOf('overview', 'sessions') }).value);
  const [primaryPage, setPrimaryPage] = useState<'overview' | 'accounts' | 'quality' | 'chats' | 'models'>(() => readSessionObject('ledger.primaryPage', { value: 'overview' as const }, { value: oneOf('overview', 'accounts', 'quality', 'chats', 'models') }).value);
  const [sessionView, setSessionView] = useState<SessionViewState>(() => readSessionObject('ledger.sessionView', INITIAL_SESSION_VIEW, { scope: oneOf('own', 'tree'), sort: oneOf('hierarchy', 'own', 'tree', 'recent') }));
  const [privacyMode, setPrivacyMode] = useState(false);
  const [sessionTrail, setSessionTrail] = useState<Array<SessionTrailEntry<SessionViewState>>>([]);
  const pendingSessionReturn = useRef<SessionTrailEntry<SessionViewState> | null>(null);
  const returnedScroll = useRef<{ id: string; top: number } | null>(null);
  const lastRevision = useRef<string | null>(null);
  const backgroundRefreshTimer = useRef<number | null>(null);
  const savedScrollTop = useRef(0);

  useEffect(() => {
    writeSessionObjects({ 'ledger.filters': appliedFilters, 'ledger.overviewTab': { value: detailTab },
      'ledger.projectTab': { value: projectDetailTab }, 'ledger.primaryPage': { value: primaryPage }, 'ledger.sessionView': sessionView, 'ledger.accountView': { value: accountView } });
  }, [appliedFilters, detailTab, primaryPage, projectDetailTab, sessionView, accountView]);

  useEffect(() => {
    const controller = new AbortController();
    if (pendingSessionReturn.current?.id !== filters.session) pendingSessionReturn.current = null;
    setRequestFailure(null);
    setLoading(true);

    void runScopedRequest(controller.signal,
      () => loadDashboardBundle(api, filters, controller.signal), {
      success: (nextBundle) => {
        setBundle(nextBundle);
        setAppliedFilters(filters);
        const nextSession = nextBundle.explorer.selectedSession;
        if (nextSession) {
          const restore = pendingSessionReturn.current;
          if (restore?.id === nextSession.id) {
            setSessionView(restore.view);
            returnedScroll.current = { id: restore.id, top: restore.scrollTop };
            pendingSessionReturn.current = null;
          } else {
            setSessionView(value => value.sessionId === nextSession.id ? value : { ...INITIAL_SESSION_VIEW, sessionId: nextSession.id });
          }
        }
      },
      failure: (reason: unknown) => {
        setRequestFailure(() => reason ?? new Error());
      },
      settled: () => {
        setManualRefreshing(false);
        setLoading(false);
      },
      });

    return () => controller.abort();
  }, [api, filters, refreshKey]);

  useEffect(() => {
    if (api.mode !== 'http') return;
    const source = new EventSource('/v1/changes');
    source.addEventListener('ledger-change', (event) => {
      const revision = parseChangeRevision((event as MessageEvent<string>).data);
      if (revision === null) return;
      if (lastRevision.current === null) {
        lastRevision.current = revision;
      } else if (revision && revision !== lastRevision.current) {
        lastRevision.current = revision;
        if (backgroundRefreshTimer.current === null) {
          backgroundRefreshTimer.current = window.setTimeout(() => {
            backgroundRefreshTimer.current = null;
            setRefreshKey((value) => value + 1);
          }, 10_000);
        }
      }
    });
    return () => {
      source.close();
      if (backgroundRefreshTimer.current !== null) {
        window.clearTimeout(backgroundRefreshTimer.current);
        backgroundRefreshTimer.current = null;
      }
    };
  }, [api]);

  useEffect(() => {
    if (privacyMode) return;
    const frame = window.requestAnimationFrame(() => {
      const scroller = document.querySelector<HTMLElement>('.workspace-scroll');
      if (scroller) scroller.scrollTop = savedScrollTop.current;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [privacyMode]);

  const retry = () => {
    setManualRefreshing(true);
    setRefreshKey((value) => value + 1);
  };
  const showToday = () => setFilters(value => ({ ...value, period: 'today', startDate: undefined, endDate: undefined, nodeOffset: 0, sessionOffset: 0 }));
  const syncOfficial = async () => {
    if (officialSyncing) return;
    setOfficialSyncing(true);
    setOfficialSyncFailed(false);
    setRefreshFeedback(t('app.syncing_official_usage_for_the_current_account'));
    try {
      await api.refreshOfficial();
      setRefreshFeedback(`${t('app.sync_complete')} · ${new Date().toLocaleTimeString(language === 'zh-CN' ? 'zh-CN' : 'en-US', { hour: '2-digit', minute: '2-digit' })}`);
    } catch (reason) {
      setOfficialSyncFailed(true);
      setRefreshFeedback(reason instanceof Error ? `${t('app.sync_failed')} · ${reason.message}` : t('app.official_usage_sync_failed'));
    } finally {
      setOfficialSyncing(false);
      setRefreshKey((value) => value + 1);
    }
  };
  const confirmAccountCount = async (count: number) => {
    setManualRefreshing(true);
    try {
      await api.setUserConfirmedAccountCount(count);
      setRefreshKey((value) => value + 1);
    } catch (reason) {
      setRequestFailure(() => reason ?? new Error());
    } finally {
      setManualRefreshing(false);
    }
  };
  const openOverview = () => {
    setPrimaryPage('overview');
    setFilters((value) => ({ ...value, project: 'all', session: 'all' }));
  };
  const openChats = () => {
    setPrimaryPage('chats');
    setSessionTrail([]);
    setFilters(value => ({ ...value, project: 'all', session: 'all', sessionSearch: '', sessionOffset: 0 }));
  };
  const openModels = () => {
    setPrimaryPage('models');
    setSessionTrail([]);
    setFilters(value => ({ ...value, project: 'all', session: 'all', sessionSearch: '', sessionOffset: 0 }));
  };
  const openAccounts = () => {
    setPrimaryPage('accounts');
    setFilters((value) => ({ ...value, project: 'all', model: 'all', session: 'all' }));
  };
  const openQuality = () => {
    setPrimaryPage('quality');
    setFilters((value) => ({ ...value, account: 'all', project: 'all', model: 'all', session: 'all' }));
  };
  const openProject = (project: string) => {
    setSessionTrail([]);
    setPrimaryPage('overview');
    setProjectDetailTab('overview');
    setFilters((value) => ({ ...value, project, session: 'all', sessionOffset: 0, sessionSearch: '' }));
  };
  const openSession = (session: string) => {
    const current = bundle?.explorer.selectedSession;
    if (current && current.id !== session) {
      setSessionTrail(trail => [...trail, { id: current.id, title: current.title, view: { ...sessionView, sessionId: current.id },
        nodes: { nodeOffset: appliedFilters.nodeOffset, nodeLimit: appliedFilters.nodeLimit, nodeSearch: appliedFilters.nodeSearch },
        scrollTop: document.querySelector<HTMLElement>('.workspace-scroll')?.scrollTop ?? 0 }]);
    }
    setFilters((value) => ({ ...value, session, nodeOffset: 0, nodeSearch: '' }));
    if (bundle?.collection.mode === 'union-preview') return;
    api.refreshOfficialThread(session)
      .then(() => setRefreshKey((value) => value + 1))
      .catch(() => { /* Thread billing detail can legitimately be unavailable. */ });
  };
  const returnToSession = (index: number) => {
    const parent = sessionTrail[index];
    if (!parent) return;
    pendingSessionReturn.current = parent;
    setSessionTrail(trail => trail.slice(0, index));
    setFilters(value => parentSessionFilters(value, parent));
  };
  const selectBreakdown = (dimension: 'account' | 'project' | 'model', id: string) => {
    if (dimension === 'project') {
      if (id === 'unassigned') {
        openQuality();
        return;
      }
      openProject(id);
      return;
    }
    if (dimension === 'model') {
      setFilters((value) => ({ ...value, model: id, session: 'all' }));
      return;
    }
    setFilters((value) => ({ ...value, account: id, session: 'all' }));
  };
  const download = (name: string, type: string, content: string) => {
    const url = URL.createObjectURL(new Blob([content], { type }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1_000);
  };
  const exportData = (format: 'json' | 'csv' | 'png') => {
    if (!bundle) return;
    const stamp = new Date().toISOString().slice(0, 10);
    if (format === 'json') {
      download(`codex-usage-${stamp}.json`, 'application/json', exportUsageJson(bundle, appliedFilters, sessionView.scope));
      return;
    }
    if (format === 'csv') {
      download(`codex-usage-${stamp}.csv`, 'text/csv;charset=utf-8', exportUsageCsv(bundle, appliedFilters, sessionView.scope));
      return;
    }
    requestNativePngExport({ privacyMode, suggestedName: `codex-usage-${stamp}.png` });
  };
  const selectedProject = bundle?.explorer.projects.find((project) => project.id === appliedFilters.project);
  const selectedSession = bundle?.explorer.selectedSession ?? null;
  useEffect(() => {
    if (!selectedSession) return;
    setSessionView((value) => value.sessionId === selectedSession.id
      ? value
      : { ...INITIAL_SESSION_VIEW, sessionId: selectedSession.id });
  }, [selectedSession?.id]);
  const currentPage: AppPage = appliedFilters.session !== 'all'
    ? 'session'
    : appliedFilters.project !== 'all'
      ? selectedProject?.kind === 'standalone_conversations'
        ? 'conversation'
        : selectedProject?.kind === 'unmatched_records'
          ? 'unmatched'
          : 'project'
      : primaryPage;
  const pageIdentity = `${currentPage}:${appliedFilters.project}:${appliedFilters.session}:${projectDetailTab}:${currentPage === 'accounts' ? `${accountView}:${appliedFilters.account}` : ''}`;
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      const scroller = document.querySelector<HTMLElement>('.workspace-scroll');
      const restore = returnedScroll.current;
      if (scroller) scroller.scrollTo({ top: restore?.id === appliedFilters.session ? restore.top : 0, behavior: 'auto' });
      returnedScroll.current = null;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [pageIdentity]);
  const quotaHistoryActive = currentPage === 'accounts' && accountView === 'history';
  const pageTitle = currentPage === 'models' ? t('models.title') : currentPage === 'chats' ? t('chats.title') : currentPage === 'accounts'
    ? t('app.accounts_quota')
    : currentPage === 'quality'
      ? t('app.data_quality')
      : selectedSession?.title ?? (selectedProject?.kind === 'standalone_conversations' ? t('app.standalone_chats') : selectedProject?.kind === 'unmatched_records' ? t('app.local_unmatched') : selectedProject?.label) ?? t('app.overview');
  const periodCaption = bundle ? `${formatPeriodRange(bundle.summary.period)}${bundle.summary.period.crossesMonth ? ` · ${t('app.cross_month')}` : ''}` : t('app.loading_time_range');
  const pageCaption = currentPage === 'overview' || currentPage === 'chats' || currentPage === 'models'
    ? `${t('app.local_attribution')} · ${periodCaption}`
    : currentPage === 'accounts'
    ? t('app.official_account_archives_quota_cycles_reset_times')
    : currentPage === 'quality'
      ? t('app.sources_freshness_unmatched_records_reconstruction_and_r')
      : selectedSession
    ? `${t('app.local_attribution')} · ${periodCaption} · ${selectedSession.subagentCount} subagents`
    : selectedProject
      ? `${selectedProject.kind === 'standalone_conversations' ? t('app.standalone_conversation_attribution') : selectedProject.kind === 'unmatched_records' ? t('app.local_unmatched_records') : t('app.local_project_attribution')} · ${periodCaption} · ${bundle?.explorer.stats.sessionCount ?? 0} ${t('app.current_sessions')} · ${bundle?.explorer.stats.historicalSessionCount ?? 0} ${t('app.historical_sessions')} · ${bundle?.explorer.stats.subagentCount ?? 0} subagents`
      : t('app.official_account_totals_with_local_project_session');
  const coverageAlertTitle = bundle?.summary.official.missingOfficialAccountCount || bundle?.summary.official.provisionalIdentityCount
    ? t('app.account_capture_summary', {
      known: bundle.summary.official.knownAccountCount,
      observed: bundle.summary.official.observedAccountCount,
      official: bundle.summary.official.accountCount,
      uncapturedSuffix: bundle.summary.official.unobservedAccountCount > 0
        ? t('app.switch_to_capture_more_accounts', { count: bundle.summary.official.unobservedAccountCount })
        : '',
      provisionalSuffix: bundle.summary.official.provisionalIdentityCount > 0
        ? t('app.provisional_identities_need_calibration', { count: bundle.summary.official.provisionalIdentityCount })
        : '',
    })
    : t('app.official_daily_coverage_tail', { date: bundle?.summary.official.commonCoverageThrough ?? t('app.unknown') });
  const viewClass = `view-${currentPage}`;
  const mobilePageValue = currentPage === 'overview' || currentPage === 'accounts' || currentPage === 'quality' || currentPage === 'chats' || currentPage === 'models'
    ? currentPage
    : selectedProject?.id ?? 'overview';
  const navigateMobile = (value: string) => {
    if (value === 'overview') openOverview();
    else if (value === 'chats') openChats();
    else if (value === 'models') openModels();
    else if (value === 'accounts') openAccounts();
    else if (value === 'quality') openQuality();
    else openProject(value);
  };

  if (privacyMode) {
    return (
      <main className="privacy-shield" role="dialog" aria-modal="true" aria-label={t('app.privacy_mode_is_on')}>
        <section>
          <div className="brand-mark" aria-hidden="true"><span /><span /><span /><span /></div>
          <h1>{t('app.privacy_mode_is_on')}</h1>
          <p>{t('app.projects_accounts_sessions_subagents_and_usage_data')}</p>
          <button onClick={() => setPrivacyMode(false)} type="button">{t('app.return_to_dashboard')}</button>
        </section>
      </main>
    );
  }

  const accountControl = <AccountSwitcher options={bundle?.summary.filters.accounts ?? []}
    rows={bundle?.breakdowns.officialAccounts ?? []} selected={appliedFilters.account}
    pending={loading} onSelect={account => setFilters(value => ({ ...value, account, sessionOffset: 0, nodeOffset: 0 }))}
    onAccounts={openAccounts} />;

  return (
    <div className="app-shell">
      <LedgerSidebar
        explorer={bundle?.explorer ?? null}
        selectedProject={appliedFilters.project}
        page={currentPage}
        period={bundle?.summary.period ?? null}
        onOverview={openOverview}
        onChats={openChats}
        onModels={openModels}
        onProject={openProject}
        onAccounts={openAccounts}
        onQuality={openQuality}
        accountControl={accountControl}
      />

      <main className="workspace-shell">
        <header className="workspace-topbar">
          <div className="workspace-heading">
            <div className="workspace-breadcrumb">
              <button onClick={openOverview} type="button">Usage</button>
              {selectedProject && <><span>›</span><button onClick={() => openProject(selectedProject.id)} type="button">{selectedProject.label}</button></>}
              {selectedSession && <><span>›</span><strong>Session</strong></>}
              {selectedSession && sessionTrail.map((ancestor, index) => <button key={`${ancestor.id}-${index}`} onClick={() => returnToSession(index)} type="button">{ancestor.title}</button>)}
              {currentPage === 'accounts' && <><span>›</span><strong>{t('app.accounts_quota')}</strong></>}
              {currentPage === 'quality' && <><span>›</span><strong>{t('app.data_quality')}</strong></>}
            </div>
            <h1>{pageTitle}</h1>
            <div className="viewed-account-label">{t('account-switcher.viewing')} {accountScopeLabel(appliedFilters.account, bundle?.summary.filters.accounts ?? [], bundle?.breakdowns.officialAccounts ?? [], t('components.ui.all_accounts'))}</div>
            <p>{pageCaption}</p>
          </div>
          <div className="topbar-actions">
            <div className="mobile-account-switcher">{accountControl}</div>
            {currentPage === 'accounts' && !quotaHistoryActive && <button type="button" onClick={syncOfficial} disabled={officialSyncing}>{t('app.sync_official')}</button>}
            <select className="mobile-page-select" aria-label={t('app.page_navigation')} value={mobilePageValue} onChange={(event) => navigateMobile(event.target.value)}>
              <option value="overview">{t('app.overview')}</option>
              <option value="chats">{t('chats.title')}</option>
              <option value="models">{t('models.title')}</option>
              <optgroup label={t('app.work')}>{bundle?.explorer.projects.filter((project) => project.kind !== 'unmatched_records').map((project) => <option key={project.id} value={project.id}>{project.kind === 'standalone_conversations' ? t('app.standalone_chats') : project.label}</option>)}</optgroup>
              <option value="accounts">{t('app.accounts_quota')}</option>
              <option value="quality">{t('app.data_quality')}</option>
            </select>
            <label className="language-select"><span className="sr-only">{t('app.interface_language')}</span><select aria-label={t('app.interface_language')} value={language} onChange={(event) => setLanguage(event.target.value as 'zh-CN' | 'en')}><option value="zh-CN">中文</option><option value="en">English</option></select></label>
            <button onClick={(event) => { event.currentTarget.blur(); savedScrollTop.current = document.querySelector<HTMLElement>('.workspace-scroll')?.scrollTop ?? 0; setPrivacyMode(true); }} type="button">{t('app.privacy')}</button>
            <div className="export-menu">
              <button type="button">{t('app.export')}</button>
              <div><button onClick={() => exportData('csv')} type="button">CSV</button><button onClick={() => exportData('json')} type="button">JSON</button><button onClick={() => exportData('png')} type="button">PNG</button></div>
            </div>
          <div className="topbar-status">
            <span className={`service-indicator mode-${api.mode}`} aria-hidden="true" />
            <div><span>{api.mode === 'mock' ? 'Demo data' : t('app.official_local_ledger')}</span><strong>{formatDateTime(bundle?.summary.official.observedAt ?? bundle?.summary.latestConfirmedAt ?? null)}</strong></div>
          </div>
          </div>
        </header>

        <div className={`workspace-scroll ${viewClass}`} tabIndex={0} role="region" aria-label={t('app.usage_workspace')}>
          {api.mode === 'mock' && (
            <aside className="demo-notice"><strong>{t('app.interactive_demo_mode')}</strong><span>{t('app.projects_sessions_and_subagents_use_demo_data')}</span></aside>
          )}

          {bundle && !quotaHistoryActive && (
            <FilterBar catalog={bundle.summary.filters} value={appliedFilters} page={currentPage} refreshing={manualRefreshing} onChange={setFilters} onRefresh={retry} />
          )}

          {bundle && loading && <div className="view-updating" role="status">{JSON.stringify(filters) === JSON.stringify(appliedFilters) ? t('app.updating_the_current_snapshot_the_previous_trusted') : t('app.applying_the_new_page_scope_and_time')}</div>}

          {bundle && !quotaHistoryActive && <DataStatusStrip summary={bundle.summary} page={currentPage} />}

          {refreshFeedback && currentPage === 'accounts' && !quotaHistoryActive && <div className={officialSyncFailed ? 'refresh-feedback is-error' : 'refresh-feedback'} role="status">{refreshFeedback}</div>}

          {bundle?.summary.official.totalIsLowerBound && currentPage === 'accounts' && !quotaHistoryActive && (
            <aside className="account-coverage-alert">
              <div><strong>{coverageAlertTitle}</strong><span>{t('app.primary_kpi_explanation', {
                tail: formatTokenMillions(bundle.summary.official.localTailTokens),
                missing: formatTokenMillions(bundle.summary.official.missingAccountLocalTokens),
                residual: formatTokenMillions(bundle.summary.missingAccountEstimate.totalUsage.total),
              })}</span></div>
              <button onClick={openAccounts} type="button">{t('app.review_account_calibration')}</button>
            </aside>
          )}

          {bundle?.collection.mode === 'union-preview' && <aside className="account-coverage-alert" role="status"><div><strong>{t('app.union_preview_title')}</strong><span>{t('app.union_preview_description')}</span></div></aside>}
          {bundle && bundle.collection.mode !== 'union-preview' && !quotaHistoryActive && <CollectionProgress status={bundle.collection} />}

          {!bundle && !error && <LoadingState />}
          {!bundle && error && <ErrorState message={error} onRetry={timePrecisionFailure ? showToday : retry}
            title={timePrecisionFailure ? t('app.time_precision_unavailable_title') : undefined}
            actionLabel={timePrecisionFailure ? t('app.show_today_usage') : undefined} />}

          {bundle && (
            <>
              {error && <div className="inline-error" role="alert">{t('app.update_failed')}: {error} {t('app.the_previous_trusted_snapshot_remains_visible')}</div>}
              {currentPage === 'overview' && <OverviewPage filters={appliedFilters} onFiltersChange={setFilters} bundle={bundle} metric={appliedFilters.metric} detailTab={detailTab} onDetailTabChange={setDetailTab} onOpenProject={openProject} onOpenSession={openSession} onSelectBreakdown={selectBreakdown} />}
              {currentPage === 'accounts' && <AccountsPage bundle={bundle} accountId={appliedFilters.account} demo={api.mode === 'mock'} view={accountView} onViewChange={setAccountView} onConfirmAccountCount={confirmAccountCount} />}
              {currentPage === 'chats' && <ConversationsPage bundle={bundle} filters={appliedFilters} onChange={setFilters} onOpenSession={openSession} />}
              {currentPage === 'models' && <ModelsPage bundle={bundle} metric={appliedFilters.metric} onSelect={selectBreakdown} />}
              {currentPage === 'quality' && <QualityPage bundle={bundle} metric={appliedFilters.metric} />}
              {(currentPage === 'project' || currentPage === 'conversation' || currentPage === 'unmatched') && (
                <ProjectPage filters={appliedFilters} onFiltersChange={setFilters} bundle={bundle} page={currentPage} projectId={appliedFilters.project} metric={appliedFilters.metric} period={appliedFilters.period} tab={projectDetailTab} onTabChange={setProjectDetailTab} onOpenSession={openSession} onSelectBreakdown={selectBreakdown} />
              )}
              {currentPage === 'session' && <SessionPage dataMode={api.mode} filters={appliedFilters} onFiltersChange={setFilters} bundle={bundle} metric={appliedFilters.metric} view={sessionView} onViewChange={setSessionView} onOpenSession={openSession} onBack={sessionTrail.length ? () => returnToSession(sessionTrail.length - 1) : undefined} />}
            </>
          )}

          <footer className="footer">
            <div><strong>Codex Usage Ledger</strong><span>{t('app.account_totals_come_from_official_codex_usage')}</span></div>
            <p>{t('app.updated')} {bundle ? formatDateTime(bundle.summary.generatedAt) : '—'} · Asia/Shanghai</p>
          </footer>
        </div>
      </main>
    </div>
  );
}

export default App;
