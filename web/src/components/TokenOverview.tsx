import type { MetricKey, SummaryResponse } from '../api/types';
import { compactNumber, exactNumber, formatPercent, metricLabel, periodLabel, tokenComposition } from '../lib';
import { useI18n } from '../i18n';

export function TokenOverview({ summary, metric }: { summary: SummaryResponse; metric: MetricKey }) {
  const { t } = useI18n();
  const confirmed = summary.usage.confirmed;
  const official = summary.official;
  const accountTotal = summary.metrics.accountTotal;
  const displayTotal = accountTotal.value;
  const isLowerBound = accountTotal.status === 'lower_bound';
  const comparison = official.displayDeltaPercent;
  const composition = tokenComposition(confirmed);
  const cacheReadShare = confirmed.total ? confirmed.cached / confirmed.total : 0;
  const cacheWriteShare = confirmed.total ? confirmed.cacheWrite / confirmed.total : 0;
  const uncachedShare = confirmed.total ? confirmed.uncached / confirmed.total : 0;
  const outputShare = confirmed.total ? confirmed.output / confirmed.total : 0;
  const reasoningShare = confirmed.output ? confirmed.reasoning / confirmed.output : 0;
  const cacheWriteComplete = confirmed.cacheWriteCoverage >= 0.999;
  const cacheWriteObserved = confirmed.cacheWriteCoverage > 0;
  const accountHeading = accountTotal.status === 'exact'
    ? t('components.token-overview.official_account_usage')
    : accountTotal.source === 'reconciled'
      ? t('components.token-overview.live_account_usage_lower_bound')
      : accountTotal.status === 'lower_bound'
        ? t('components.explorer.local_observable_lower_bound')
        : t('components.token-overview.account_usage_unknown');
  const accountScopeLabel = official.displayTotalKind === 'local_lower_bound'
    ? t('components.token-overview.local_observation_only')
    : accountTotal.machineScope === 'all_devices'
      ? t('components.token-overview.all_account_devices')
      : t('components.token-overview.local_machine');

  return (
    <section className="token-overview" aria-label={t('components.token-overview.codex_token_summary')}>
      <div className="overview-heading">
        <div>
          <p className="eyebrow">{t('components.token-overview.official_account_usage')}</p>
          <h2>{accountHeading}</h2>
        </div>
        <span className="overview-period">{periodLabel(summary.period.key)} · {t('components.token-overview.trend_metric')} {metricLabel(metric)}</span>
      </div>

      <div className="overview-body">
        <article className="total-stat">
          <span>{isLowerBound ? t('components.token-overview.selected_period_live_lower_bound') : t('components.token-overview.selected_period_account_total')}</span>
          <strong>{displayTotal === null ? '—' : `${isLowerBound ? '≥ ' : ''}${compactNumber(displayTotal)}`}</strong>
          <small>{displayTotal === null ? t('components.token-overview.account_total_cannot_be_determined_for_this') : `${exactNumber(displayTotal)} tokens · ${accountScopeLabel}`}</small>
          <div className="total-stat-meta">
            <div>
              <strong>{comparison === null ? '—' : `${isLowerBound ? '≥ ' : ''}${comparison >= 0 ? '+' : ''}${formatPercent(comparison)}`}</strong>
              <span>{t('components.token-overview.vs_previous_period')}</span>
            </div>
            <div>
              <strong>{official.commonCoverageThrough ?? '—'}</strong>
              <span>{t('components.token-overview.common_coverage_through')}</span>
            </div>
          </div>
        </article>

        <div className="composition-block">
          <div className="composition-heading">
            <div>
              <strong>{t('components.token-overview.local_token_composition_four_exclusive_buckets')}</strong>
              <span>{t('components.token-overview.input_cache_read_cache_write_output_local')}</span>
            </div>
            <span>{compactNumber(confirmed.total)} {t('components.token-overview.local_sample_different_scope_from_the_official')}</span>
          </div>
          <div className="composition-track" aria-label={t('components.explorer.token_composition')}>
            <span className="composition-uncached" style={{ width: `${uncachedShare * 100}%` }} />
            <span className="composition-cached" style={{ width: `${cacheReadShare * 100}%` }} />
            <span className="composition-cache-write" style={{ width: `${cacheWriteShare * 100}%` }} />
            <span className="composition-output" style={{ width: `${outputShare * 100}%` }} />
          </div>
          <div className="composition-legend">
            <div><i className="legend-uncached" /><span>{t('components.explorer.input')}</span><strong>{formatPercent(uncachedShare)}</strong></div>
            <div><i className="legend-cached" /><span>{t('components.explorer.cache_read')}</span><strong>{formatPercent(cacheReadShare)}</strong></div>
            <div><i className="legend-cache-write" /><span>{t('components.explorer.cache_write')}</span><strong>{cacheWriteObserved ? `${cacheWriteComplete ? '' : '≥ '}${formatPercent(cacheWriteShare)}` : '—'}</strong></div>
            <div><i className="legend-output" /><span>{t('components.explorer.output')}</span><strong>{formatPercent(outputShare)}</strong></div>
          </div>
          <p className="cache-write-coverage">{confirmed.cacheWriteCoverage >= 0.999
            ? t('components.token-overview.the_cache_write_field_covers_the_current')
            : t('components.token-overview.cache_write_coverage_note', { coverage: formatPercent(confirmed.cacheWriteCoverage) })}</p>
          <div className="reasoning-meter">
            <div>
              <span>{t('components.explorer.reasoning_inside_output')}</span>
              <strong>{formatPercent(reasoningShare)}</strong>
            </div>
            <div className="reasoning-track"><span style={{ width: `${reasoningShare * 100}%` }} /></div>
            <small>{t('components.token-overview.reasoning_is_an_output_detail_and_is')}</small>
          </div>
        </div>
      </div>

      <div className="metric-card-grid">
        {composition.map((metric) => {
          const cacheWriteMetric = metric.key === 'cacheWrite';
          const value = cacheWriteMetric && !cacheWriteObserved ? '—' : `${cacheWriteMetric && !cacheWriteComplete ? '≥ ' : ''}${compactNumber(metric.value)}`;
          const exact = cacheWriteMetric && !cacheWriteComplete
            ? t('components.token-overview.field_coverage_lower_bound', { coverage: formatPercent(confirmed.cacheWriteCoverage) })
            : exactNumber(metric.value);
          const label = metric.key === 'uncached' && !cacheWriteComplete ? t('components.token-overview.input_incl_unsplit') : metric.label;
          return (
          <article className={`metric-card metric-${metric.key}`} key={metric.key}>
            <div className="metric-label-row">
              <span>{label} · {t('components.explorer.local_sample')}</span>
              <i aria-hidden="true" />
            </div>
            <strong>{value}</strong>
            <small>{exact}</small>
          </article>
          );
        })}
      </div>

      <div className="trust-strip">
        <div>
          <span className="trust-dot trusted" />
          <span>{t('components.token-overview.official_booked')}</span>
          <strong>{official.totalTokens === null ? '—' : compactNumber(official.totalTokens)}</strong>
        </div>
        <div>
          <span className="trust-dot quarantined" />
          <span>{t('components.token-overview.local_complement_floor')}</span>
          <strong>{compactNumber(official.localComplementTokens)}</strong>
        </div>
        <div>
          <span className="trust-dot unknown" />
          <span>{t('components.token-overview.project_attribution_sample')}</span>
          <strong>{compactNumber(confirmed.total)}</strong>
        </div>
        <p>{official.missingOfficialAccountCount > 0 ? t('components.token-overview.primary_ledger_additions', {
          tail: compactNumber(official.localTailTokens),
          missing: compactNumber(official.missingAccountLocalTokens),
        }) : official.localTailTokens > 0 ? t('components.token-overview.official_coverage_has_not_reached_the_tail') : t('components.token-overview.official_data_covers_the_selected_period')}</p>
      </div>
    </section>
  );
}
