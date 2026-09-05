import type { MissingAccountEstimate } from '../api/types';
import { compactNumber } from '../lib';
import { useI18n } from '../i18n';

export function MissingAccountEstimatePanel({ estimate }: { estimate: MissingAccountEstimate }) {
  const { t } = useI18n();
  if (!estimate.applicable) return null;
  return <section className="panel missing-estimate-panel">
    <header className="panel-heading"><h2>{t('diagnostics.account_difference')}</h2></header>
    <p>{t('diagnostics.difference_explanation')}</p>
    <strong>{estimate.alignedAccountDays > 0 ? compactNumber(estimate.rawResidualTokens) : '—'}</strong>
    <p>{estimate.coverageStart ?? '—'} — {estimate.coverageThrough ?? '—'} · {estimate.alignedAccountDays} {t('components.missing-account-estimate-panel.comparable_account_days')}</p>
  </section>;
}
