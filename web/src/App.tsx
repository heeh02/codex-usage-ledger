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
import { compactNumber, formatDateTime, formatPeriodRange } from './lib';
import { useI18n } from './i18n';
import { requestNativePngExport } from './nativeBridge';
import type { AppPage } from './page';

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

function restoredSessionValue<T>(key: string, fallback: T): T {
  try {
    const encoded = window.sessionStorage.getItem(key);
    return encoded ? { ...fallback, ...JSON.parse(encoded) } : fallback;
  } catch {
    return fallback;
  }
}

function App() {
  const { language, setLanguage, t } = useI18n();
  const api = useMemo(() => createLedgerApi(), []);
  const [filters, setFilters] = useState<DashboardFilters>(() => restoredSessionValue('ledger.filters', INITIAL_FILTERS));
  const [appliedFilters, setAppliedFilters] = useState<DashboardFilters>(() => restoredSessionValue('ledger.filters', INITIAL_FILTERS));
  const [bundle, setBundle] = useState<DashboardBundle | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [refreshKey, setRefreshKey] = useState(0);
  const [manualRefreshing, setManualRefreshing] = useState(false);
  const [refreshFeedback, setRefreshFeedback] = useState('');
  const [detailTab, setDetailTab] = useState<OverviewDetailTab>(() => restoredSessionValue('ledger.overviewTab', { value: 'projects' as const }).value);
  const [projectDetailTab, setProjectDetailTab] = useState<'overview' | 'sessions'>(() => restoredSessionValue('ledger.projectTab', { value: 'overview' as const }).value);
  const [primaryPage, setPrimaryPage] = useState<'overview' | 'accounts' | 'quality'>(() => restoredSessionValue('ledger.primaryPage', { value: 'overview' as const }).value);
  const [sessionView, setSessionView] = useState<SessionViewState>(() => restoredSessionValue('ledger.sessionView', INITIAL_SESSION_VIEW));
  const [privacyMode, setPrivacyMode] = useState(false);
  const lastRevision = useRef<string | null>(null);
  const backgroundRefreshTimer = useRef<number | null>(null);
  const savedScrollTop = useRef(0);

  useEffect(() => {
    window.sessionStorage.setItem('ledger.filters', JSON.stringify(appliedFilters));
    window.sessionStorage.setItem('ledger.overviewTab', JSON.stringify({ value: detailTab }));
    window.sessionStorage.setItem('ledger.projectTab', JSON.stringify({ value: projectDetailTab }));
    window.sessionStorage.setItem('ledger.primaryPage', JSON.stringify({ value: primaryPage }));
    window.sessionStorage.setItem('ledger.sessionView', JSON.stringify(sessionView));
  }, [appliedFilters, detailTab, primaryPage, projectDetailTab, sessionView]);

  useEffect(() => {
    const controller = new AbortController();
    setError('');
    setLoading(true);

    loadDashboardBundle(api, filters, controller.signal)
      .then((nextBundle) => {
        setBundle(nextBundle);
        setAppliedFilters(filters);
      })
      .catch((reason: unknown) => {
        if (reason instanceof DOMException && reason.name === 'AbortError') return;
        setError(reason instanceof Error ? reason.message : t('app.unknown_dashboard_error'));
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setManualRefreshing(false);
          setLoading(false);
        }
      });

    return () => controller.abort();
  }, [api, filters, refreshKey]);

  useEffect(() => {
    if (api.mode !== 'http') return;
    const source = new EventSource('/v1/changes');
    source.addEventListener('ledger-change', (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as { revision?: string | number };
      const revision = String(payload.revision ?? '');
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

  const retry = async () => {
    setManualRefreshing(true);
    setRefreshFeedback(t('app.syncing_official_usage_for_the_current_account'));
    try {
      await api.refreshOfficial();
      setRefreshFeedback(`${t('app.sync_complete')} · ${new Date().toLocaleTimeString(language === 'zh-CN' ? 'zh-CN' : 'en-US', { hour: '2-digit', minute: '2-digit' })}`);
    } catch (reason) {
      setRefreshFeedback(reason instanceof Error ? `${t('app.sync_failed')} · ${reason.message}` : t('app.official_usage_sync_failed'));
    } finally {
      setRefreshKey((value) => value + 1);
    }
  };
  const confirmAccountCount = async (count: number) => {
    setManualRefreshing(true);
    try {
      await api.setUserConfirmedAccountCount(count);
      setRefreshKey((value) => value + 1);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : t('app.account_count_calibration_failed'));
    } finally {
      setManualRefreshing(false);
    }
  };
  const openOverview = () => {
    setPrimaryPage('overview');
    setFilters((value) => ({ ...value, project: 'all', model: 'all', session: 'all' }));
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
    setPrimaryPage('overview');
    setProjectDetailTab('overview');
    setFilters((value) => ({ ...value, account: 'all', project, session: 'all' }));
  };
  const openSession = (session: string) => {
    setFilters((value) => ({ ...value, session }));
    api.refreshOfficialThread(session)
      .then(() => setRefreshKey((value) => value + 1))
      .catch(() => { /* Thread billing detail can legitimately be unavailable. */ });
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
      download(`codex-usage-${stamp}.json`, 'application/json', JSON.stringify({ filters: appliedFilters, bundle }, null, 2));
      return;
    }
    if (format === 'csv') {
      const local = new Map(bundle.timeseries.points.map((point) => [point.date, point]));
      const rows: Array<Array<string | number>> = [['bucket', 'official_total', 'local_total', 'input', 'cached_input', 'uncached_input', 'output', 'reasoning', 'sampling_requests']];
      for (const point of bundle.summary.official.points) {
        const sample = local.get(point.date);
        rows.push([point.date, point.tokens, sample?.confirmed.total ?? '', sample?.confirmed.input ?? '', sample?.confirmed.cached ?? '', sample?.confirmed.uncached ?? '', sample?.confirmed.output ?? '', sample?.confirmed.reasoning ?? '', sample?.confirmedEvents ?? '']);
      }
      download(`codex-usage-${stamp}.csv`, 'text/csv;charset=utf-8', rows.map((row) => row.join(',')).join('\n'));
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
  const pageIdentity = `${currentPage}:${appliedFilters.project}:${appliedFilters.session}:${projectDetailTab}`;
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      const scroller = document.querySelector<HTMLElement>('.workspace-scroll');
      if (scroller) scroller.scrollTo({ top: 0, behavior: 'auto' });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [pageIdentity]);
  const pageTitle = currentPage === 'accounts'
    ? t('app.accounts_quota')
    : currentPage === 'quality'
      ? t('app.data_quality')
      : selectedSession?.title ?? (selectedProject?.kind === 'standalone_conversations' ? t('app.standalone_chats') : selectedProject?.kind === 'unmatched_records' ? t('app.local_unmatched') : selectedProject?.label) ?? t('app.overview');
  const periodCaption = bundle ? `${formatPeriodRange(bundle.summary.period)}${bundle.summary.period.crossesMonth ? ` · ${t('app.cross_month')}` : ''}` : t('app.loading_time_range');
  const pageCaption = currentPage === 'accounts'
    ? t('app.official_account_archives_quota_cycles_reset_times')
    : currentPage === 'quality'
      ? t('app.sources_freshness_unmatched_records_reconstruction_and_r')
      : selectedSession
    ? `${t('app.local_attribution')} · ${periodCaption} · ${selectedSession.subagentCount} subagents · ${selectedSession.model ?? t('app.model_unknown')}`
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
  const mobilePageValue = currentPage === 'overview' || currentPage === 'accounts' || currentPage === 'quality'
    ? currentPage
    : selectedProject?.id ?? 'overview';
  const navigateMobile = (value: string) => {
    if (value === 'overview') openOverview();
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

  return (
    <div className="app-shell">
      <LedgerSidebar
        explorer={bundle?.explorer ?? null}
        selectedProject={appliedFilters.project}
        page={currentPage}
        period={bundle?.summary.period ?? null}
        onOverview={openOverview}
        onProject={openProject}
        onAccounts={openAccounts}
        onQuality={openQuality}
      />

      <main className="workspace-shell">
        <header className="workspace-topbar">
          <div className="workspace-heading">
            <div className="workspace-breadcrumb">
              <button onClick={openOverview} type="button">Usage</button>
              {selectedProject && <><span>›</span><button onClick={() => openProject(selectedProject.id)} type="button">{selectedProject.label}</button></>}
              {selectedSession && <><span>›</span><strong>Session</strong></>}
              {currentPage === 'accounts' && <><span>›</span><strong>{t('app.accounts_quota')}</strong></>}
              {currentPage === 'quality' && <><span>›</span><strong>{t('app.data_quality')}</strong></>}
            </div>
            <h1>{pageTitle}</h1>
            <p>{pageCaption}</p>
          </div>
          <div className="topbar-actions">
            <select className="mobile-page-select" aria-label={t('app.page_navigation')} value={mobilePageValue} onChange={(event) => navigateMobile(event.target.value)}>
              <option value="overview">{t('app.overview')}</option>
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

        <div className={`workspace-scroll ${viewClass}`}>
          {api.mode === 'mock' && (
            <aside className="demo-notice"><strong>{t('app.interactive_demo_mode')}</strong><span>{t('app.projects_sessions_and_subagents_use_demo_data')}</span></aside>
          )}

          {bundle && (
            <FilterBar catalog={bundle.summary.filters} value={appliedFilters} page={currentPage} contextLabel={pageTitle} refreshing={manualRefreshing} onChange={setFilters} onRefresh={retry} />
          )}

          {bundle && loading && <div className="view-updating" role="status">{JSON.stringify(filters) === JSON.stringify(appliedFilters) ? t('app.updating_the_current_snapshot_the_previous_trusted') : t('app.applying_the_new_page_scope_and_time')}</div>}

          {bundle && <DataStatusStrip summary={bundle.summary} page={currentPage} />}

          {refreshFeedback && <div className={refreshFeedback.startsWith('同步失败') || refreshFeedback.startsWith('Sync failed') ? 'refresh-feedback is-error' : 'refresh-feedback'} role="status">{refreshFeedback}</div>}

          {bundle?.summary.official.totalIsLowerBound && (currentPage === 'overview' || currentPage === 'accounts') && (
            <aside className="account-coverage-alert">
              <div><strong>{coverageAlertTitle}</strong><span>{t('app.primary_kpi_explanation', {
                tail: compactNumber(bundle.summary.official.localTailTokens),
                missing: compactNumber(bundle.summary.official.missingAccountLocalTokens),
                residual: compactNumber(bundle.summary.missingAccountEstimate.totalUsage.total),
              })}</span></div>
              <button onClick={openAccounts} type="button">{t('app.review_account_calibration')}</button>
            </aside>
          )}

          {bundle && <CollectionProgress status={bundle.collection} />}

          {!bundle && !error && <LoadingState />}
          {!bundle && error && <ErrorState message={error} onRetry={retry} />}

          {bundle && (
            <>
              {error && <div className="inline-error">{t('app.update_failed')}: {error}. {t('app.the_previous_trusted_snapshot_remains_visible')}</div>}
              {currentPage === 'overview' && <OverviewPage bundle={bundle} metric={appliedFilters.metric} detailTab={detailTab} onDetailTabChange={setDetailTab} onOpenProject={openProject} onOpenSession={openSession} onSelectBreakdown={selectBreakdown} />}
              {currentPage === 'accounts' && <AccountsPage bundle={bundle} onConfirmAccountCount={confirmAccountCount} />}
              {currentPage === 'quality' && <QualityPage bundle={bundle} metric={appliedFilters.metric} />}
              {(currentPage === 'project' || currentPage === 'conversation' || currentPage === 'unmatched') && (
                <ProjectPage bundle={bundle} page={currentPage} projectId={appliedFilters.project} metric={appliedFilters.metric} period={appliedFilters.period} tab={projectDetailTab} onTabChange={setProjectDetailTab} onOpenSession={openSession} onSelectBreakdown={selectBreakdown} />
              )}
              {currentPage === 'session' && <SessionPage bundle={bundle} metric={appliedFilters.metric} view={sessionView} onViewChange={setSessionView} />}
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
