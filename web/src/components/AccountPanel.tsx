import { type FormEvent, useEffect, useState } from 'react';
import type { BreakdownsResponse, OfficialUsageView, TimelineEvent } from '../api/types';
import { compactNumber, formatDateTime } from '../lib';
import { EmptyState, Panel } from './Ui';
import { useI18n } from '../i18n';

export function AccountPanel({ data, official, timeline, onConfirmAccountCount }: { data: BreakdownsResponse; official: OfficialUsageView; timeline: TimelineEvent[]; onConfirmAccountCount: (count: number) => Promise<void> }) {
  const { language, t } = useI18n();
  const [accountCountDraft, setAccountCountDraft] = useState(official.userConfirmedAccountCount ?? official.knownAccountCount);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setAccountCountDraft(official.userConfirmedAccountCount ?? official.knownAccountCount);
  }, [official.userConfirmedAccountCount, official.knownAccountCount]);
  const saveAccountCount = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const count = Math.max(official.observedAccountCount, Math.min(64, Math.round(accountCountDraft)));
    setSaving(true);
    try {
      await onConfirmAccountCount(count);
    } finally {
      setSaving(false);
    }
  };
  const periodValue = (tokens: number | null, lowerBound: boolean) => tokens === null ? t('components.account-panel.pending') : `${lowerBound ? '≥ ' : ''}${compactNumber(tokens)}`;
  return (
    <section className="account-detail-grid">
      <Panel title={t('components.account-panel.account_usage')} eyebrow={t('components.account-panel.official_account_ledger')} meta={t('components.account-panel.isolated_by_login_account_workspace')} className="account-ledgers-panel">
        <div className="account-coverage-summary" aria-label={t('components.account-panel.multi_account_official_coverage_summary')}>
          <div><span>{t('components.account-panel.common_exact_coverage')}</span><strong>{official.commonCoverageStart && official.commonCoverageThrough ? `${official.commonCoverageStart} → ${official.commonCoverageThrough}` : t('components.account-panel.no_common_coverage')}</strong></div>
          <div><span>{t('components.account-panel.latest_account')}</span><strong>{official.latestCoverageThrough ?? '—'}</strong></div>
          <div><span>{t('components.account-panel.captured_identities')}</span><strong>{official.observedAccountCount}/{official.knownAccountCount}</strong></div>
          <div><span>{t('components.account-panel.official_archives')}</span><strong>{official.accountCount}/{official.knownAccountCount}</strong></div>
          <div><span>{t('components.account-panel.current_status')}</span><strong>{official.coverageComplete && official.accountCoverageComplete && official.identityScopeComplete ? t('components.account-panel.complete') : t('components.account-panel.lower_bound_pending')}</strong></div>
        </div>
        <form className="account-calibration" onSubmit={saveAccountCount}>
          <div>
            <strong>{t('components.account-panel.account_completeness_calibration')}</strong>
            <span>{t('components.account-panel.enter_the_number_of_accounts_used_on')}</span>
          </div>
          <label>
            <span>{t('components.account-panel.confirmed_total')}</span>
            <input type="number" min={Math.max(1, official.observedAccountCount)} max="64" step="1" value={accountCountDraft} onChange={(event) => setAccountCountDraft(Number(event.target.value))} />
          </label>
          <button disabled={saving || accountCountDraft < official.observedAccountCount || accountCountDraft > 64} type="submit">{saving ? t('components.account-panel.saving') : t('components.account-panel.save_calibration')}</button>
        </form>
        {official.unobservedAccountCount > 0 && <div className="account-identity-warning"><strong>{official.unobservedAccountCount} {t('components.account-panel.accounts_not_yet_captured')}</strong><span>{t('components.account-panel.they_are_not_mixed_into_the_official')}</span></div>}
        {official.provisionalIdentityCount > 0 && <div className="account-identity-warning"><strong>{official.provisionalIdentityCount} {t('components.account-panel.historical_accounts_need_calibration')}</strong><span>{compactNumber(official.provisionalLocalTokens)} {t('components.account-panel.local_tokens_are_included_as_the_all')}</span></div>}
        {data.officialAccounts.length ? (
          <div className="account-ledger-list">
            {data.officialAccounts.map((account) => (
              <article key={account.id} className={account.officialAvailable ? '' : 'is-awaiting-calibration'}>
                <header><div><strong>{account.label}</strong>{account.planType && <span>{account.planType}</span>}{account.active && <span>{t('components.account-panel.current_account')}</span>}{!account.officialAvailable && <span>{t('components.account-panel.awaiting_official_calibration')}</span>}</div><small>{account.officialAvailable ? `${t('components.account-panel.updated')} ${formatDateTime(account.observedAt)}` : t('components.account-panel.syncs_automatically_next_time_this_account_is')}</small></header>
                <div className="account-period-grid">
                  <div><span>{t('components.account-panel.today')}</span><strong>{periodValue(account.todayTokens, account.todayIsLowerBound)}</strong></div>
                  <div><span>{t('components.account-panel.this_week')}</span><strong>{periodValue(account.weekTokens, account.weekIsLowerBound)}</strong><small>{t('components.account-panel.from_monday_00_00_may_cross_months')}</small></div>
                  <div><span>{t('components.account-panel.this_month')}</span><strong>{periodValue(account.monthTokens, account.monthIsLowerBound)}</strong><small>{t('components.account-panel.from_day_1_at_00_00')}</small></div>
                  <div><span>{t('components.account-panel.lifetime')}</span><strong>{periodValue(account.lifetimeTokens, account.lifetimeIsLowerBound)}</strong></div>
                </div>
                <footer><span>{account.authEpochCount} {t('components.account-panel.login_epochs')}</span><span>{account.officialAvailable ? `${t('components.account-panel.official_coverage')} ${account.coverageStart ?? '—'} → ${account.coverageThrough ?? '—'}` : t('components.account-panel.local_account_boundaries_are_reconstructed_official_tota')}</span></footer>
              </article>
            ))}
          </div>
        ) : <EmptyState text={t('components.account-panel.official_account_usage_has_not_been_synced')} />}
      </Panel>

      <Panel title={t('components.account-panel.account_timeline')} eyebrow={t('components.account-panel.switches_resets')} meta={t('components.account-panel.login_log_reconstruction_live_capture')} className="account-timeline-panel">
        {timeline.length ? (
          <div className="account-timeline-list">
            {timeline.map((event) => (
              <article key={event.id}><i>{event.kind === 'account_switch' ? '⇄' : '↺'}</i><div><strong>{event.kind === 'account_switch' ? t('components.account-panel.account_switch') : t('components.account-panel.quota_reset')}</strong><span>{formatDateTime(event.at)}</span><p>{language === 'zh-CN' ? event.detail : event.kind === 'account_switch' ? 'A locally observed login boundary moved usage to a different account identity.' : 'An official quota percentage reset was observed for this account.'}</p></div></article>
            ))}
          </div>
        ) : <EmptyState text={t('components.account-panel.no_account_switches_or_quota_resets_occurred')} />}
      </Panel>
    </section>
  );
}
