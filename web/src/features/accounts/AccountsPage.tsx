import type { DashboardBundle } from '../../api/types';
import { useId } from 'react';
import { AccountPanel } from '../../components/AccountPanel';
import { MissingAccountEstimatePanel } from '../../components/MissingAccountEstimatePanel';
import { QuotaPanel } from '../../components/QuotaPanel';
import { DimensionCompareChart, UsageTrendPanel } from '../../components/TrendAndTimeline';
import { useI18n } from '../../i18n';
import { QuotaHistoryPanel } from './QuotaHistoryPanel';
import { accountScopeLabel } from '../../components/AccountSwitcher';

interface AccountsPageProps {
  bundle: DashboardBundle;
  accountId: string;
  demo: boolean;
  view: 'usage' | 'history';
  onViewChange: (view: 'usage' | 'history') => void;
  onConfirmAccountCount: (count: number) => Promise<void>;
}

export function AccountsPage({ bundle, accountId, demo, view, onViewChange, onConfirmAccountCount }: AccountsPageProps) {
  const { t } = useI18n();
  const tabId = useId();
  const breakdowns = { ...bundle.breakdowns, officialAccounts: bundle.breakdowns.officialAccounts.filter(account => accountId === 'all' || account.id === accountId) };
  return (
    <>
      <nav className="account-view-tabs" role="tablist" aria-label={t('history.account_views')}>
        {(['usage', 'history'] as const).map((value, index) => <button key={value} type="button" role="tab" id={`${tabId}-${value}`} aria-controls={`${tabId}-panel`} aria-selected={view === value} tabIndex={view === value ? 0 : -1}
          onClick={() => onViewChange(value)} onKeyDown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); onViewChange(index === 0 ? 'history' : 'usage'); event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[index === 0 ? 1 : 0]?.focus(); } }}>
          {t(value === 'usage' ? 'history.usage_tab' : 'history.history_tab')}
        </button>)}
      </nav>
      <div role="tabpanel" id={`${tabId}-panel`} aria-labelledby={`${tabId}-${view}`}>
      {view === 'history' ? <QuotaHistoryPanel key={`${demo}:${accountId}`} account={accountId} demo={demo} timezone={bundle.summary.period.timezone}
        accountName={id => accountScopeLabel(id, bundle.summary.filters.accounts, bundle.breakdowns.officialAccounts, t('components.ui.all_accounts'))} /> : <>
      <UsageTrendPanel data={bundle.timeseries} metric="total" title={t('accounts.trend')} allowProjectCompare={false} />
      {accountId === 'all' && <section className="panel">
        <header className="panel-heading"><h2>{t('accounts.compare_local')}</h2></header>
        <p>{t('accounts.compare_scope')}</p>
        <DimensionCompareChart data={{ ...bundle.timeseries, projectSeries: bundle.timeseries.accountSeries ?? [], official: { ...bundle.timeseries.official, primaryScope: false } }} metric="total" />
      </section>}
      <AccountPanel data={breakdowns} official={bundle.summary.official} timeline={bundle.timeseries.timeline} onConfirmAccountCount={onConfirmAccountCount} />
      <QuotaPanel pools={bundle.summary.quotaPools} cycles={bundle.summary.quotaCycles} showPreview={false} />
      <MissingAccountEstimatePanel estimate={bundle.summary.missingAccountEstimate} />
      </>}
      </div>
    </>
  );
}
