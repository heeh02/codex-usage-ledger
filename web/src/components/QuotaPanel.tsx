import type { CSSProperties } from 'react';
import type { QuotaCycle, QuotaPool } from '../api/types';
import { compactNumber, formatDateTime, formatPercent, relativeReset } from '../lib';
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
  const coverage = cycle.localCoverageRatio === null ? t('components.quota-panel.coverage_unknown') : `${t('components.quota-panel.local_observation_coverage')} ${formatPercent(cycle.localCoverageRatio)}`;
  return (
    <article className="quota-cycle-row">
      <div className="quota-cycle-identity">
        <span>{cycle.accountLabel}</span>
        <strong>{cycle.label}</strong>
        <small>{cycle.windowKind === 'weekly' ? t('components.quota-panel.official_weekly_quota_cycle') : cycle.windowKind === 'short' ? t('components.quota-panel.official_short_window') : t('components.quota-panel.official_custom_window')} · {cycle.sampleCount} {t('components.quota-panel.snapshots')}</small>
      </div>
      <div><span>{t('components.quota-panel.cycle_used')}</span><strong>{cycle.usedPercent === null ? '—' : `${cycle.usedPercent.toFixed(1)}%`}</strong><small>{cycle.usedDeltaPercent === null ? t('components.quota-panel.no_comparable_starting_point') : `${t('components.quota-panel.since_first_observation')} ${cycle.usedDeltaPercent >= 0 ? '+' : ''}${cycle.usedDeltaPercent.toFixed(1)}pp`}</small></div>
      <div><span>{t('components.quota-panel.local_token_sample')}</span><strong>{compactNumber(cycle.localUsage.total)}</strong><small>{coverage} · {cycle.localEvents} requests</small></div>
      <div><span>{t('components.quota-panel.four_bucket_composition')}</span><strong>{compactNumber(cycle.localUsage.cached)} {t('components.quota-panel.cache_read')}</strong><small>{t('components.explorer.input')} {compactNumber(cycle.localUsage.uncached)} · {t('components.explorer.write_58af22')} {cycle.localUsage.cacheWriteCoverage > 0 ? `${cycle.localUsage.cacheWriteCoverage >= 0.999 ? '' : '≥ '}${compactNumber(cycle.localUsage.cacheWrite)}` : '—'} · {t('components.quota-panel.output')} {compactNumber(cycle.localUsage.output)}</small></div>
      <div><span>{t('components.quota-panel.observed_correlation')}</span><strong>{cycle.empiricalTokensPerUsedPercent === null ? '—' : `${compactNumber(cycle.empiricalTokensPerUsedPercent)} / pp`}</strong><small>{t('components.quota-panel.describes_the_local_sample_only_not_a')}</small></div>
      <div><span>{t('components.quota-panel.cycle_end')}</span><strong>{relativeReset(cycle.cycleEnd)}</strong><small>{cycle.cycleStart ? `${formatDateTime(cycle.cycleStart)} ${t('components.quota-panel.start')}` : t('components.quota-panel.cycle_start_unknown')}</small></div>
    </article>
  );
}

export function QuotaPanel({ pools, cycles }: { pools: QuotaPool[]; cycles: QuotaCycle[] }) {
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
      <section className="quota-cycle-section">
        <header><div><strong>{t('components.quota-panel.quota_cycle_local_tokens')}</strong><span>{t('components.quota-panel.archived_per_account_by_official_reset_time')}</span></div><small>{t('components.quota-panel.percentages_are_not_converted_to_tokens_at')}</small></header>
        {cycles.length ? <div className="quota-cycle-list">{cycles.map((cycle) => <QuotaCycleRow key={cycle.id} cycle={cycle} />)}</div> : <EmptyState text={t('components.quota-panel.not_enough_quota_cycle_snapshots_yet_they')} />}
      </section>
    </Panel>
  );
}
