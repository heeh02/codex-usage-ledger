import { useId } from 'react';
import type { ReactNode } from 'react';
import type { CollectionStatus, DashboardFilters, FilterCatalog, SummaryResponse } from '../api/types';
import { exactNumber, formatDateTime, formatPercent, formatPeriodRange } from '../lib';
import { periodLabel } from '../lib';
import { useI18n } from '../i18n';
import type { AppPage } from '../page';

export function Panel({
  title,
  eyebrow,
  meta,
  className = '',
  children,
}: {
  title: string;
  eyebrow?: string;
  meta?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section className={`panel ${className}`}>
      <header className="panel-heading">
        <div>
          {eyebrow && <p className="eyebrow">{eyebrow}</p>}
          <h2>{title}</h2>
        </div>
        {meta && <div className="panel-meta">{meta}</div>}
      </header>
      {children}
    </section>
  );
}

function FilterSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: Array<{ id: string; label: string }>;
  onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <label className="filter-field" htmlFor={id}>
      <span>{label}</span>
      <select id={id} aria-label={label} value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

export function FilterBar({
  catalog,
  value,
  page,
  contextLabel,
  refreshing,
  onChange,
  onRefresh,
}: {
  catalog: FilterCatalog;
  value: DashboardFilters;
  page: AppPage;
  contextLabel?: string;
  refreshing: boolean;
  onChange: (value: DashboardFilters) => void;
  onRefresh: () => void;
}) {
  const { language, t } = useI18n();
  const showAccount = page === 'overview' || page === 'accounts' || page === 'quality';
  const showModel = page === 'project' || page === 'conversation' || page === 'unmatched';
  const showMetric = page !== 'accounts';
  const showGrain = page !== 'accounts' && page !== 'quality';
  const accountOptions = catalog.accounts.map((option) => {
    if (option.id === 'all') return { ...option, label: t('components.ui.all_accounts') };
    if (language === 'zh-CN') return option;
    return { ...option, label: option.label.replace(/^当前账号\s*·\s*/, 'Current · ').replace(/^已校准账号\s*·\s*/, 'Calibrated · ').replace(/^历史账号\s*·\s*/, 'Historical · ') };
  });
  const modelOptions = catalog.models.map((option) => option.id === 'all' ? { ...option, label: t('components.ui.all_models') } : option);
  return (
    <section className={`filter-bar filter-bar-${page}`} aria-label={t('components.ui.current_page_filters')}>
      <div className="filter-scope filter-account-scope">
        <div className="filter-scope-label">
          <strong>{showAccount ? t('components.ui.account_scope') : t('components.ui.current_object')}</strong>
          <span>{showAccount ? t('components.ui.official_total_all_devices') : contextLabel ?? t('app.local_attribution')}</span>
        </div>
        {showAccount && <div className="account-filter-control">
          <FilterSelect
            label={t('components.ui.account')}
            value={value.account}
            options={accountOptions}
            onChange={(account) => onChange({ ...value, account })}
          />
        </div>}
        {!showAccount && <div className="context-filter-label"><strong>{contextLabel ?? t('app.local_attribution')}</strong><span>{t('components.ui.page_scope_is_fixed')}</span></div>}
        <div className="period-control" role="group" aria-label={t('components.ui.reporting_period')}>
          {catalog.periods.map((period) => (
            <button
              aria-pressed={value.period === period.id}
              className={value.period === period.id ? 'period-option is-selected' : 'period-option'}
              key={period.id}
              onClick={() => onChange({ ...value, period: period.id })}
              type="button"
            >
              {periodLabel(period.id)}
            </button>
          ))}
        </div>
        <button aria-label={refreshing ? t('components.ui.refreshing_official_account_usage') : t('components.ui.refresh_official_account_usage')} className="refresh-button" onClick={onRefresh} disabled={refreshing} type="button">
          <svg className={refreshing ? 'refresh-icon spinning' : 'refresh-icon'} viewBox="0 0 20 20" aria-hidden="true">
            <path d="M16.6 7.2A7 7 0 1 0 17 11" />
            <path d="m13.5 4.3 3.4 3.2 1.9-4.1" />
          </svg>
          <span>{refreshing ? t('components.ui.syncing') : t('components.ui.refresh')}</span>
        </button>
      </div>
      {(showModel || showMetric || showGrain) && <div className="filter-scope filter-local-scope">
        <div className="filter-scope-label"><strong>{t('components.ui.analysis_dimensions')}</strong><span>{t('components.ui.applies_only_to_this_page')}</span></div>
        <div className="local-filter-grid">
          {showModel && <FilterSelect
            label={t('components.explorer.model')}
            value={value.model}
            options={modelOptions}
            onChange={(model) => onChange({ ...value, model })}
          />}
          {showMetric && <FilterSelect
            label={t('components.ui.metric')}
            value={value.metric}
            options={[
              { id: 'total', label: t('components.ui.total') },
              { id: 'input', label: t('components.ui.input_incl_cache') },
              { id: 'uncached', label: t('components.ui.input_uncached') },
              { id: 'cached', label: t('components.explorer.cache_read') },
              { id: 'cacheWrite', label: t('components.explorer.cache_write') },
              { id: 'output', label: t('components.explorer.output') },
              { id: 'reasoning', label: 'Reasoning' },
              { id: 'requests', label: t('components.ui.requests') },
            ]}
            onChange={(metric) => onChange({ ...value, metric: metric as DashboardFilters['metric'] })}
          />}
          {showGrain && <FilterSelect
            label={t('components.ui.trend_grain')}
            value={value.grain}
            options={[
              { id: 'auto', label: t('components.ui.auto') },
              { id: 'hour', label: t('components.ui.hour') },
              { id: 'day', label: t('components.ui.day') },
              { id: 'week', label: t('components.ui.week') },
              { id: 'month', label: t('components.ui.month') },
            ]}
            onChange={(grain) => onChange({ ...value, grain: grain as DashboardFilters['grain'] })}
          />}
        </div>
      </div>}
    </section>
  );
}

export function CollectionProgress({ status }: { status: CollectionStatus }) {
  const { t } = useI18n();
  if (status.phase === 'live') return null;
  const active = ['optimizing', 'compacting', 'backfill', 'syncing'].includes(status.phase);
  const total = Math.max(status.itemsTotal, 0);
  const completed = Math.min(Math.max(status.itemsCompleted, 0), total || status.itemsCompleted);
  const ratio = total ? completed / total : 0;
  const label = status.phase === 'optimizing'
    ? t('components.ui.building_historical_rollups')
    : status.phase === 'compacting'
      ? t('components.ui.compacting_redundant_details')
      : status.phase === 'backfill' || status.phase === 'syncing'
        ? t('components.ui.syncing_session_increments')
        : t('components.ui.collection_paused');
  const detail = active
    ? status.message ?? t('components.ui.current_values_reflect_completed_work_and_will')
    : t('components.ui.the_directory_can_still_update_but_token');
  return (
    <aside className={`collection-progress phase-${status.phase}`} role={active ? 'status' : 'note'}>
      <div className="collection-progress-copy">
        <div>
          <span className="collection-state-dot" aria-hidden="true" />
          <strong>{label}</strong>
          {active && total > 0 && <b>{Math.round(ratio * 100)}%</b>}
        </div>
        <p>{detail}</p>
      </div>
      {active && (
        <div className="collection-progress-meter" aria-label={`${label} ${Math.round(ratio * 100)}%`}>
          <span style={{ width: `${ratio * 100}%` }} />
        </div>
      )}
    </aside>
  );
}

export function DataStatusStrip({ summary, page }: { summary: SummaryResponse; page: AppPage }) {
  const { t } = useI18n();
  const watermarkLag = summary.latestConfirmedAt
    ? Math.max(0, Math.round((Date.now() - Date.parse(summary.latestConfirmedAt)) / 1_000))
    : null;
  const coverage = summary.attributionCoverage;
  const scopeRange = `${t('components.ui.account')} ${coverage.officialWindowStart ?? '—'}—${coverage.officialWindowThrough ?? '—'} · ${t('components.ui.local')} ${coverage.localWindowStart ?? '—'}—${coverage.localWindowThrough ?? '—'}`;
  const compactScope = `${periodLabel(summary.period.key)} · ${formatPeriodRange(summary.period)}`;
  const globalPage = page === 'overview' || page === 'accounts' || page === 'quality';
  return (
    <aside className="data-status-strip" aria-label={t('components.ui.data_status')}>
      <span className="period-window-label" title={`${t('components.ui.exact_local_window')} ${formatPeriodRange(summary.period)}; ${scopeRange}`}><strong>{globalPage ? periodLabel(summary.period.key) : compactScope}</strong>{globalPage ? ` ${scopeRange}` : ''}{summary.period.crossesMonth && <b>{t('app.cross_month')}</b>}{summary.period.partial && <em>{t('components.ui.in_progress')}</em>}</span>
      <span title={t('components.ui.calculated_by_request_count_confirmed_confirmed_quaranti')}><i className={summary.matchRate >= 0.98 ? 'is-good' : 'is-warning'} />{t('components.ui.request_match')} <strong>{formatPercent(summary.matchRate)}</strong></span>
      <span>{t('components.ui.unmatched')} <strong>{exactNumber(summary.unmatchedEvents)}</strong></span>
      {globalPage && <span title={`${t('components.ui.common_coverage')} ${summary.official.commonCoverageStart ?? '—'} → ${summary.official.commonCoverageThrough ?? '—'}; ${t('components.ui.latest_account')} ${summary.official.latestCoverageThrough ?? '—'}`}>{t('components.ui.official_accounts')} <strong>{summary.official.accountCount}/{summary.official.knownAccountCount}</strong></span>}
      <span>{t('components.ui.last_sync')} <strong>{formatDateTime(summary.official.observedAt)}</strong></span>
      <span>{t('components.ui.local_watermark_lag')} <strong>{watermarkLag === null ? '—' : watermarkLag < 60 ? `${watermarkLag}s` : `${Math.round(watermarkLag / 60)}m`}</strong></span>
    </aside>
  );
}

export function LoadingState() {
  const { t } = useI18n();
  return (
    <div className="loading-state" role="status">
      <span className="loading-orbit" aria-hidden="true" />
      <div>
        <strong>{t('components.ui.loading_trusted_summaries')}</strong>
        <p>{t('components.ui.reading_summary_timeseries_breakdown_and_quality_contrac')}</p>
      </div>
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <div className="error-state" role="alert">
      <div>
        <strong>{t('components.ui.dashboard_data_is_temporarily_unavailable')}</strong>
        <p>{message}</p>
      </div>
      <button type="button" onClick={onRetry}>
        {t('components.ui.retry')}
      </button>
    </div>
  );
}

export function EmptyState({ text }: { text: string }) {
  return <div className="empty-state">{text}</div>;
}
