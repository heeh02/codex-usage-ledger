import { useEffect, useState } from 'react';
import type { QuotaHistoryCursor, QuotaHistoryInterval } from '../../api/quotaHistory';
import { fetchQuotaIntervalUsage, mockQuotaIntervalUsage, type QuotaIntervalUsageResponse } from '../../api/quotaIntervalUsage';
import { LocalComposition } from '../../components/LocalComposition';
import { UsageBreakdownTable } from '../../components/UsageBreakdownTable';
import { dimensionLabel, formatPercent } from '../../lib';
import { useI18n } from '../../i18n';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { QuotaIntervalChart } from './QuotaIntervalChart';

export function QuotaIntervalUsage({ row, selection, demo, formatTimestamp, accountLabel }: { row: QuotaHistoryInterval; selection?: QuotaHistoryCursor; demo: boolean; formatTimestamp: (value: string | null) => string; accountLabel: string }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false); const [revision, setRevision] = useState(0);
  const [data, setData] = useState<QuotaIntervalUsageResponse | null>(null); const [busy, setBusy] = useState(false); const [failed, setFailed] = useState(false);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!open || !selection) return;
    const controller = new AbortController(); setBusy(true); setFailed(false);
    void runScopedRequest(controller.signal, () => demo ? Promise.resolve(mockQuotaIntervalUsage(row, selection)) : fetchQuotaIntervalUsage(row, selection, controller.signal), {
      success: value => { setPending(value.status === 'pending'); setData(old => value.status === 'pending' && old?.status === 'available' ? old : value); }, failure: () => setFailed(true), settled: () => setBusy(false),
    }); return () => controller.abort();
  }, [open, selection, row, demo, revision]);
  useEffect(() => {
    if (!open || !pending || busy || failed) return;
    const timer = window.setTimeout(() => setRevision(value => value + 1), 5000);
    return () => window.clearTimeout(timer);
  }, [open, pending, busy, failed]);
  const message = data?.status === 'source_overlap_review' ? t('interval.source_review') : data?.status === 'pending' ? t('interval.pending')
    : data?.status === 'no_safe_interval' ? t('history.no_safe_interval') : t('interval.no_evidence');
  return <section className="quota-interval-usage" aria-label={t('interval.title')}>
    <button type="button" aria-expanded={open} disabled={!selection} onClick={() => setOpen(value => !value)}>{t('interval.title')}</button>
    {!selection && <p>{t('interval.upgrade_needed')}</p>}
    {open && <>
      <p><strong>{accountLabel}</strong> · {row.limitName ?? row.limitId}</p>
      <p>{t('interval.scope')}</p>
      {busy && <p role="status">{t('interval.loading')}</p>}{failed && <p role="alert">{t('interval.failed')}</p>}
      {pending && data?.status === 'available' && <p role="status">{t('interval.pending')} {t('history.retaining_view')}</p>}
      {data && data.status !== 'available' && <p role="status">{message}</p>}
      {data && <QuotaIntervalChart data={data} formatTimestamp={formatTimestamp}/>}
      {data?.status === 'available' && data.usage && data.events !== null && <>
        <p>{t('quota.observation_interval')}: {formatTimestamp(data.start)} — {formatTimestamp(data.end)}</p>
        <p>{t('interval.observed')}: {formatTimestamp(data.observedAt)} · {t('interval.cache_hit')}: {formatPercent(data.usage.input ? data.usage.cached / data.usage.input : null)}</p>
        <LocalComposition usage={data.usage} eventCount={data.events}/>
        <UsageBreakdownTable rows={data.models} identityLabel={t('sessions.models_used')}/>
        <UsageBreakdownTable rows={data.projects} identityLabel={t('components.explorer.projects')} resolveLabel={group => group.id === null ? t('sessions.unknown_dimension') : dimensionLabel(group.id, group.label ?? group.id)}/>
      </>}
      <button type="button" disabled={busy} onClick={() => setRevision(value => value + 1)}>{t('interval.refresh')}</button>
    </>}
  </section>;
}
