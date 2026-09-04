import type { MissingAccountEstimate, TokenUsage } from '../api/types';
import { compactNumber, dimensionLabel, formatPercent } from '../lib';
import { useI18n } from '../i18n';

function sparklinePoints(values: number[]): string {
  if (!values.length) return '';
  const maximum = Math.max(...values, 1);
  const width = 220;
  const height = 56;
  return values.map((value, index) => {
    const x = values.length === 1 ? width / 2 : index * width / (values.length - 1);
    const y = height - (value / maximum) * (height - 6) - 3;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(' ');
}

function TokenComposition({ usage }: { usage: TokenUsage }) {
  const { t } = useI18n();
  const cacheWriteComplete = usage.cacheWriteCoverage >= 0.999;
  const cacheWriteObserved = usage.cacheWriteCoverage > 0;
  const cachedShare = usage.total ? usage.cached / usage.total : 0;
  const cacheWriteShare = usage.total ? usage.cacheWrite / usage.total : 0;
  const uncachedShare = usage.total ? usage.uncached / usage.total : 0;
  const outputShare = usage.total ? usage.output / usage.total : 0;
  return (
    <div className="missing-composition">
      <div className="missing-composition-bar" aria-label={t('components.missing-account-estimate-panel.estimated_token_composition')}>
        <i className="is-uncached" style={{ width: `${uncachedShare * 100}%` }} />
        <i className="is-cached" style={{ width: `${cachedShare * 100}%` }} />
        <i className="is-cache-write" style={{ width: `${cacheWriteShare * 100}%` }} />
        <i className="is-output" style={{ width: `${outputShare * 100}%` }} />
      </div>
      <div className="missing-composition-legend">
        <span><i className="is-uncached" />{cacheWriteComplete ? t('components.explorer.input') : t('components.explorer.input_unsplit')} <strong>{compactNumber(usage.uncached)}</strong></span>
        <span><i className="is-cached" />{t('components.explorer.cache_read')} <strong>{compactNumber(usage.cached)}</strong></span>
        <span><i className="is-cache-write" />{t('components.explorer.cache_write')} <strong>{cacheWriteObserved ? `${cacheWriteComplete ? '' : '≥ '}${compactNumber(usage.cacheWrite)}` : '—'}</strong></span>
        <span><i className="is-output" />{t('components.explorer.output')} <strong>{compactNumber(usage.output)}</strong></span>
      </div>
      <small>{t('components.missing-account-estimate-panel.cache_write_field_coverage')} {formatPercent(usage.cacheWriteCoverage)} · Reasoning {compactNumber(usage.reasoning)} {t('components.missing-account-estimate-panel.output_subset')}</small>
    </div>
  );
}

export function MissingAccountEstimatePanel({ estimate }: { estimate: MissingAccountEstimate }) {
  const { language, t } = useI18n();
  if (!estimate.applicable || estimate.combinedUnobservedAccountCount === 0) return null;
  const selected = estimate.selectedUsage.total !== estimate.totalUsage.total;
  const hasCoverage = estimate.alignedAccountDays > 0;
  const visibleUsage = selected ? estimate.selectedUsage : estimate.totalUsage;
  const points = sparklinePoints(estimate.byDay.map((point) => point.usage.total));
  const maxProject = Math.max(...estimate.byProject.map((project) => project.usage.total), 1);
  const coverage = estimate.coverageStart && estimate.coverageThrough
    ? `${estimate.coverageStart} — ${estimate.coverageThrough}`
    : t('components.missing-account-estimate-panel.no_overlapping_daily_buckets');

  return (
    <section className="missing-estimate-panel panel" aria-labelledby="missing-estimate-title">
      <header className="missing-estimate-heading">
        <div>
          <span>{t('components.missing-account-estimate-panel.account_residual_estimate')}</span>
          <h2 id="missing-estimate-title">{t('components.missing-account-estimate-panel.combined_estimate_for_uncaptured_accounts')}</h2>
          <p>{t('components.missing-account-estimate-panel.calibration_explanation', { count: estimate.capturedAccountCount })}</p>
        </div>
        <strong>{hasCoverage ? t('components.missing-account-estimate-panel.conservative_floor') : t('components.missing-account-estimate-panel.waiting_for_comparable_days')} · {estimate.combinedUnobservedAccountCount} {estimate.combinedUnobservedAccountCount === 1 ? t('components.missing-account-estimate-panel.account_combined_one') : t('components.missing-account-estimate-panel.accounts_combined_many')}</strong>
      </header>

      <div className="missing-estimate-grid">
        <div className="missing-estimate-total">
          <span>{selected ? t('components.missing-account-estimate-panel.current_project_model_filter') : t('components.missing-account-estimate-panel.identifiable_combined_lower_bound')}</span>
          <strong>{hasCoverage ? compactNumber(visibleUsage.total) : '—'}</strong>
          <small>{hasCoverage ? `${visibleUsage.total.toLocaleString(language === 'zh-CN' ? 'zh-CN' : 'en-US')} tokens` : t('components.missing-account-estimate-panel.missing_official_dates_cannot_be_treated_as')}</small>
          <dl>
            <div><dt>{t('components.missing-account-estimate-panel.aligned_range')}</dt><dd>{coverage}</dd></div>
            <div><dt>{t('components.missing-account-estimate-panel.comparable_account_days')}</dt><dd>{estimate.alignedAccountDays}</dd></div>
            <div><dt>{t('components.missing-account-estimate-panel.positive_residual_days')}</dt><dd>{estimate.excessAccountDays}</dd></div>
          </dl>
        </div>

        <div className="missing-estimate-trend">
          <div><strong>{t('components.missing-account-estimate-panel.residual_trend')}</strong><span>{t('components.missing-account-estimate-panel.daily_only_dates_with_both_official_and')}</span></div>
          {hasCoverage && points ? (
            <svg viewBox="0 0 220 62" role="img" aria-label={t('components.missing-account-estimate-panel.daily_estimate_trend_for_uncaptured_accounts')}>
              <line x1="0" y1="59" x2="220" y2="59" />
              <polyline points={points} />
            </svg>
          ) : <p>{hasCoverage ? t('components.missing-account-estimate-panel.no_allocatable_positive_residual_exists_in_this') : t('components.missing-account-estimate-panel.waiting_for_official_daily_buckets_and_local')}</p>}
          {hasCoverage && <TokenComposition usage={visibleUsage} />}
        </div>

        <div className="missing-estimate-projects">
          <div><strong>{t('components.missing-account-estimate-panel.estimated_project_attribution')}</strong><span>{!hasCoverage ? t('components.missing-account-estimate-panel.waiting_for_coverage') : selected ? t('components.missing-account-estimate-panel.all_project_population') : `Top ${Math.min(estimate.byProject.length, 5)}`}</span></div>
          {estimate.byProject.slice(0, 5).map((project, index) => (
            <article key={project.id}>
              <span>{index + 1}</span>
              <div>
                <div><strong>{dimensionLabel(project.id, project.label)}</strong><b>{compactNumber(project.usage.total)}</b></div>
                <i><span style={{ width: `${project.usage.total / maxProject * 100}%` }} /></i>
              </div>
            </article>
          ))}
          {!estimate.byProject.length && <p>{hasCoverage ? t('components.missing-account-estimate-panel.no_allocatable_projects_yet') : t('components.missing-account-estimate-panel.no_comparable_official_daily_buckets_exist_in')}</p>}
        </div>
      </div>

      {estimate.sourceAccountExcess.length > 0 && (
        <div className="missing-source-audit" aria-label={t('components.missing-account-estimate-panel.per_account_calibration_audit')}>
          <span>{estimate.capturedAccountCount} {t('components.missing-account-estimate-panel.captured_accounts_calibrated_daily')}</span>
          {estimate.sourceAccountExcess.map((source) => (
            <article key={source.accountId}>
              <strong>{source.accountLabel}</strong>
              <small>{source.alignedDays} {source.alignedDays === 1 ? t('components.missing-account-estimate-panel.comparable_day_one') : t('components.missing-account-estimate-panel.comparable_days_many')} · {source.excessDays} {source.excessDays === 1 ? t('components.missing-account-estimate-panel.positive_residual_day_one') : t('components.missing-account-estimate-panel.positive_residual_days_many')}</small>
              <b>{compactNumber(source.estimatedTokens)}</b>
            </article>
          ))}
        </div>
      )}

      <footer className="missing-estimate-foot">
        <span>{t('components.missing-account-estimate-panel.method_per_account_and_day_compute_max')}</span>
        <strong>{t('components.missing-account-estimate-panel.this_cannot_split_the_third_and_fourth')}</strong>
      </footer>
    </section>
  );
}
