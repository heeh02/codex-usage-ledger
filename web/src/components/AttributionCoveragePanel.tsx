import type { AttributionCoverage } from '../api/types';
import { compactNumber, exactNumber, formatPercent } from '../lib';
import { useI18n } from '../i18n';

export function AttributionCoveragePanel({ coverage }: { coverage: AttributionCoverage }) {
  const { language, t } = useI18n();
  const total = Math.max(coverage.localAttributedTokens, 1);
  const namedShare = Math.min(coverage.namedProjectTokens / total, 1);
  const standaloneShare = Math.min(coverage.standaloneConversationTokens / total, 1 - namedShare);
  const unassignedShare = Math.min(coverage.unassignedTokens / total, 1 - namedShare - standaloneShare);
  const gapShare = Math.max(1 - namedShare - standaloneShare - unassignedShare, 0);
  const ratio = coverage.coverageRatio ?? 0;

  return (
    <section className="attribution-coverage panel" aria-labelledby="attribution-coverage-title">
      <header>
        <div>
          <span>{t('components.attribution-coverage-panel.local_attribution_completeness')}</span>
          <h2 id="attribution-coverage-title">{t('components.attribution-coverage-panel.project_attribution_coverage')}</h2>
          <p>{t('components.attribution-coverage-panel.the_project_list_explains_valid_source_selected')}</p>
        </div>
        <div className="attribution-coverage-rate">
          <strong>{formatPercent(ratio)}</strong>
          <span>{t('components.attribution-coverage-panel.locally_explained_share')}</span>
        </div>
      </header>

      <div className="attribution-coverage-summary">
        <article><span>{t('components.attribution-coverage-panel.valid_local_attribution')}</span><strong>{compactNumber(coverage.localAttributedTokens)}</strong><small>{t('components.attribution-coverage-panel.shared_denominator_for_projects_standalone_chats_and')}</small></article>
        <article><span>{t('components.attribution-coverage-panel.attributed_to_projects')}</span><strong>{compactNumber(coverage.namedProjectTokens)}</strong><small>{exactNumber(coverage.namedProjectTokens)} tokens</small></article>
        <article><span>{t('app.standalone_chats')}</span><strong>{compactNumber(coverage.standaloneConversationTokens)}</strong><small>{t('components.attribution-coverage-panel.remaining_after_project_resolution')} · {coverage.standaloneConversations.withLocalEvidence}/{coverage.standaloneConversations.indexed} {t('components.attribution-coverage-panel.with_local_evidence')}</small></article>
        <article><span>{t('app.local_unmatched')}</span><strong>{compactNumber(coverage.unassignedTokens)}</strong><small>{t('components.attribution-coverage-panel.token_facts_outside_all_indexed_chats_and')}</small></article>
        <article className="is-gap"><span>{t('components.attribution-coverage-panel.no_classification_evidence')}</span><strong>{compactNumber(coverage.unattributedTokens)}</strong><small>{t('components.attribution-coverage-panel.never_allocated_proportionally')}</small></article>
      </div>

      <div className="attribution-coverage-track" aria-label={t('components.attribution-coverage-panel.project_attribution_coverage_ratio')}>
        <i className="is-named" style={{ width: `${namedShare * 100}%` }} />
        <i className="is-standalone" style={{ width: `${standaloneShare * 100}%` }} />
        <i className="is-unassigned" style={{ width: `${unassignedShare * 100}%` }} />
        <i className="is-gap" style={{ width: `${gapShare * 100}%` }} />
      </div>
      <div className="attribution-coverage-legend">
        <span><i className="is-named" />{t('components.attribution-coverage-panel.projects')} {formatPercent(namedShare)}</span>
        <span><i className="is-standalone" />{t('components.attribution-coverage-panel.standalone')} {formatPercent(standaloneShare)}</span>
        <span><i className="is-unassigned" />{t('app.local_unmatched')} {formatPercent(unassignedShare)}</span>
        <span><i className="is-gap" />{t('components.attribution-coverage-panel.no_project_evidence')} {formatPercent(gapShare)}</span>
      </div>

      <div className="attribution-equation" aria-label={t('components.attribution-coverage-panel.valid_local_attribution_identity')}>
        <span>{compactNumber(coverage.namedProjectTokens)} {t('components.attribution-coverage-panel.projects_7fd0e9')}</span>
        <b>+</b>
        <span>{compactNumber(coverage.standaloneConversationTokens)} {t('components.attribution-coverage-panel.standalone_dcd45f')}</span>
        <b>+</b>
        <span>{compactNumber(coverage.unassignedTokens)} {t('components.attribution-coverage-panel.local_unmatched')}</span>
        <b>+</b>
        <span>{compactNumber(coverage.unattributedTokens)} {t('components.attribution-coverage-panel.unclassified')}</span>
        <b>=</b>
        <strong>{compactNumber(coverage.localAttributedTokens)} {t('components.attribution-coverage-panel.valid_local_attribution_a4b512')}</strong>
      </div>

      <div className="attribution-gap-buckets">
        {coverage.gapBuckets.map((bucket) => (
          <article key={bucket.id}>
            <div><strong>{bucket.id === 'official_before_local_evidence' ? t('components.attribution-coverage-panel.official_usage_before_local_evidence') : bucket.id === 'official_days_without_local_evidence' ? t('components.attribution-coverage-panel.official_days_without_local_sampling_evidence') : t('components.attribution-coverage-panel.remaining_net_gap')}</strong><span>{bucket.tokens < 0 ? t('components.attribution-coverage-panel.net_excess') : t('components.attribution-coverage-panel.gap')}</span></div>
            <b>{bucket.tokens < 0 ? '−' : ''}{compactNumber(Math.abs(bucket.tokens))}</b>
            <small>{language === 'zh-CN' ? bucket.detail : bucket.id === 'official_before_local_evidence' ? 'Official account usage predates recoverable local project evidence.' : bucket.id === 'official_days_without_local_evidence' ? 'Official usage exists on dates without matching local sampling evidence.' : 'Remaining difference after comparing aligned official and local evidence.'}</small>
          </article>
        ))}
      </div>

      <footer>
        <span>{t('components.attribution-coverage-panel.account_total')} {coverage.accountTotalTokens === null ? t('components.attribution-coverage-panel.no_official_data') : compactNumber(coverage.accountTotalTokens)} · {t('components.attribution-coverage-panel.compared_independently_with_local_attribution')}</span>
        <span>{t('components.attribution-coverage-panel.official_range')} {coverage.officialWindowStart ?? '—'}—{coverage.officialWindowThrough ?? '—'}</span>
        <span>{t('components.attribution-coverage-panel.local_evidence')} {coverage.localWindowStart ?? '—'}—{coverage.localWindowThrough ?? '—'}</span>
        <strong>{t('components.attribution-coverage-panel.the_gap_may_include_other_devices_and')}</strong>
      </footer>
    </section>
  );
}
