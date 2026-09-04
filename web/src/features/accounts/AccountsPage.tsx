import type { DashboardBundle } from '../../api/types';
import { AccountPanel } from '../../components/AccountPanel';
import { MissingAccountEstimatePanel } from '../../components/MissingAccountEstimatePanel';
import { QuotaPanel } from '../../components/QuotaPanel';

interface AccountsPageProps {
  bundle: DashboardBundle;
  onConfirmAccountCount: (count: number) => Promise<void>;
}

export function AccountsPage({ bundle, onConfirmAccountCount }: AccountsPageProps) {
  return (
    <>
      <AccountPanel data={bundle.breakdowns} official={bundle.summary.official} timeline={bundle.timeseries.timeline} onConfirmAccountCount={onConfirmAccountCount} />
      <QuotaPanel pools={bundle.summary.quotaPools} cycles={bundle.summary.quotaCycles} />
      <MissingAccountEstimatePanel estimate={bundle.summary.missingAccountEstimate} />
    </>
  );
}
