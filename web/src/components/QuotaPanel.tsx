import { hasCacheWriteAmount } from '../shared/cacheWriteDisplay';
import type { CSSProperties } from 'react';
import type { QuotaCycle, QuotaPool } from '../api/types';
import { formatTokenMillions, formatDateTime, formatPercent, relativeReset } from '../lib';
import { EmptyState, Panel } from './Ui';
import { useI18n } from '../i18n';

function QuotaCard({ pool }: { pool: QuotaPool }) {
  const { t } = useI18n();
  const used = pool.usedPercent ?? 0;
  const ringStyle = { '--quota-used': Math.min(100, used) } as CSSProperties;
  return (
    <article className={`quota-card quota-${pool.status}`}>
      <header>
        <div>
          <span className="quota-account">{pool.accountLabel}</span>
          <h3>{pool.label}</h3>
        </div>
        <span className={`quota-freshness ${pool.stale ? 'is-stale' : ''}`}>
          <i />{pool.stale ? t('components.quota-panel.stale') : t('components.quota-panel.live')}
        </span>
      </header>
      <div className="quota-main">
        <div className="quota-ring" style={ringStyle}>
          <div>
            <strong>{pool.usedPercent == null ? '—' : `${Math.round(pool.usedPercent)}%`}</strong>
            <span>{t('components.quota-panel.used')}</span>
          </div>
        </div>
        <div className="quota-reset">
          <span>{t('components.quota-panel.next_reset')}</span>
          <strong>{relativeReset(pool.resetsAt)}</strong>
          <small>{pool.detail ?? 'Codex allowance'}</small>
        </div>
      </div>
      <div className="quota-foot">
        <span>{t('components.quota-panel.remaining')} {pool.usedPercent == null ? '—' : `${Math.max(0, 100 - Math.round(pool.usedPercent))}%`}</span>
        <span>{t('components.quota-panel.observed')} {formatDateTime(pool.observedAt)}</span>
      </div>
    </article>
  );
}

function QuotaCycleRow({ cycle }: { cycle: QuotaCycle }) {
  const { t } = useI18n();
  const boundary = cycle.boundaryKind === 'observed_decrease' ? t('quota.boundary_decrease')
    : cycle.boundaryKind === 'deadline_change' ? t('quota.boundary_deadline')
    : cycle.boundaryKind === 'window_change' ? t('quota.boundary_window')
    : cycle.boundaryKind === 'conflicting_timestamp' ? t('quota.boundary_conflict')
    : cycle.boundaryKind === 'first_observation' ? t('quota.boundary_first') : t('quota.boundary_unknown');
  const coverage = cycle.localCoverageRatio === null ? t('components.quota-panel.coverage_unknown') : `${t('components.quota-panel.local_observation_coverage')} ${formatPercent(cycle.localCoverageRatio)}`;
  return (
    <article className="quota-cycle-row">
      <div className="quota-cycle-identity">
        <span>{cycle.accountLabel}</span>
        <strong>{cycle.label}</strong>
        {cycle.boundaryKind && <small>{boundary}</small>}
        {cycle.boundaryAfter && <small>{t('quota.boundary_observations')}: {formatDateTime(cycle.boundaryAfter)} — {formatDateTime(cycle.firstObservedAt)}</small>}
        <small>{cycle.windowKind === 'weekly' ? t('components.quota-panel.official_weekly_quota_cycle') : cycle.windowKind === 'short' ? t('components.quota-panel.official_short_window') : t('components.quota-panel.official_custom_window')} · {cycle.sampleCount} {t('components.quota-panel.snapshots')}</small>
      </div>
      <div><span>{t('quota.last_observed_used')}</span><strong>{cycle.usedPercent === null ? '—' : `${cycle.usedPercent.toFixed(1)}%`}</strong><small>{formatDateTime(cycle.lastObservedAt)} · {cycle.usedDeltaPercent === null ? t('components.quota-panel.no_comparable_starting_point') : `${t('components.quota-panel.since_first_observation')} ${cycle.usedDeltaPercent >= 0 ? '+' : ''}${cycle.usedDeltaPercent.toFixed(1)}pp`}</small></div>
      <div><span>{t('components.quota-panel.local_token_sample')}</span><strong>{cycle.localEvents > 0 ? formatTokenMillions(cycle.localUsage.total) : '—'}</strong><small>{coverage} · {cycle.localEvents} requests</small>{cycle.localObservationEnd && <small>{t('quota.observation_interval')}: {formatDateTime(cycle.localObservationStart)} — {formatDateTime(cycle.localObservationEnd)}</small>}</div>
      {cycle.localEvents > 0 ? <div><span>{t('components.quota-panel.four_bucket_composition')}</span><strong>{formatTokenMillions(cycle.localUsage.cached)} {t('components.quota-panel.cache_read')}</strong><small>{t('components.explorer.input')} {formatTokenMillions(cycle.localUsage.uncached)} · {t('components.explorer.write_58af22')} {hasCacheWriteAmount(cycle.localUsage) ? `${cycle.localUsage.cacheWriteCoverage >= 1 ? '' : '≥ '}${formatTokenMillions(cycle.localUsage.cacheWrite)}` : '—'} · {t('components.quota-panel.output')} {formatTokenMillions(cycle.localUsage.output)}</small></div> : <div><span>{t('components.quota-panel.four_bucket_composition')}</span><strong>—</strong><small>{t('usage.no_confirmed_records')}</small></div>}
      <div><span>{t('components.quota-panel.observed_correlation')}</span><strong>—</strong><small>{t('components.quota-panel.pool_attribution_unavailable')}</small></div>
      <div><span>{t('components.quota-panel.cycle_end')}</span><strong>{cycle.cycleEnd && Date.parse(cycle.cycleEnd) <= Date.now() ? formatDateTime(cycle.cycleEnd) : relativeReset(cycle.cycleEnd)}</strong><small>{cycle.cycleStart ? `${formatDateTime(cycle.cycleStart)} ${t('components.quota-panel.start')}` : t('components.quota-panel.cycle_start_unknown')}</small></div>
    </article>
  );
}

export function QuotaPanel({ pools, cycles, showPreview = true }: { pools: QuotaPool[]; cycles: QuotaCycle[]; showPreview?: boolean }) {
  const { t } = useI18n();
  return (
    <Panel
      title={t('components.quota-panel.quota_pools')}
      eyebrow={t('components.quota-panel.live_quota')}
      meta={<span className="definition-chip">{t('components.quota-panel.independent_per_account_never_added_across_pools')}</span>}
      className="quota-panel"
    >
      {pools.length ? (
        <div className="quota-grid">
          {pools.map((pool) => (
            <QuotaCard key={pool.id} pool={pool} />
          ))}
        </div>
      ) : (
        <EmptyState text={t('components.quota-panel.the_current_account_has_no_trusted_quota')} />
      )}
      {showPreview && <section className="quota-cycle-section">
        <header><div><strong>{t('quota.history_preview')}</strong><span>{t('quota.history_preview_scope')}</span></div><small>{t('components.quota-panel.percentages_are_not_converted_to_tokens_at')}</small></header>
        {cycles.some(cycle => cycle.historyLimited) && <p role="status">{t('quota.history_limited')}</p>}
        {cycles.length ? <div className="quota-cycle-list">{cycles.map((cycle) => <QuotaCycleRow key={cycle.id} cycle={cycle} />)}</div> : <EmptyState text={t('components.quota-panel.not_enough_quota_cycle_snapshots_yet_they')} />}
      </section>}
    </Panel>
  );
}
