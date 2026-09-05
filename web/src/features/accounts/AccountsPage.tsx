import type { DashboardBundle } from '../../api/types';
import { AccountPanel } from '../../components/AccountPanel';
import { MissingAccountEstimatePanel } from '../../components/MissingAccountEstimatePanel';
import { QuotaPanel } from '../../components/QuotaPanel';
import { UsageTrendPanel } from '../../components/TrendAndTimeline';
import { useI18n } from '../../i18n';

interface AccountsPageProps {
  bundle: DashboardBundle;
  accountId: string;
  onConfirmAccountCount: (count: number) => Promise<void>;
}

export function AccountsPage({ bundle, accountId, onConfirmAccountCount }: AccountsPageProps) {
  const { t } = useI18n();
  const breakdowns = { ...bundle.breakdowns, officialAccounts: bundle.breakdowns.officialAccounts.filter(account => accountId === 'all' || account.id === accountId) };
  return (
    <>
      <UsageTrendPanel data={bundle.timeseries} metric="total" title={t('accounts.trend')} allowProjectCompare={false} />
      <AccountPanel data={breakdowns} official={bundle.summary.official} timeline={bundle.timeseries.timeline} onConfirmAccountCount={onConfirmAccountCount} />
      <QuotaPanel pools={bundle.summary.quotaPools} cycles={bundle.summary.quotaCycles} />
      <MissingAccountEstimatePanel estimate={bundle.summary.missingAccountEstimate} />
    </>
  );
}
