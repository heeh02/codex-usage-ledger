import { useEffect, useRef, useState } from 'react';
import { fetchQuotaHistory, type QuotaHistoryCursor, type QuotaHistoryResponse } from '../../api/quotaHistory';
import { mockQuotaHistory } from '../../api/quotaHistoryMock';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { useI18n } from '../../i18n';
import './quota-history.css';
import { QuotaIntervalUsage } from './QuotaIntervalUsage';

interface Props { account: string; demo: boolean; timezone: string; accountName: (id: string) => string }
export function QuotaHistoryPanel({ account, demo, timezone, accountName }: Props) {
  const { language, t } = useI18n();
  const [request, setRequest] = useState<{ kind: 'first' | 'next'; cursor?: QuotaHistoryCursor }>({ kind: 'first' });
  const [state, setState] = useState<{ page: QuotaHistoryResponse | null; previous: QuotaHistoryResponse[] }>({ page: null, previous: [] });
  const [busy, setBusy] = useState(true);
  const [failed, setFailed] = useState(false);
  const [indexPending, setIndexPending] = useState(false);
  const firstRow = useRef<HTMLElement>(null);
  const scrollRequested = useRef(false);
  useEffect(() => {
    if (scrollRequested.current && state.page) {
      scrollRequested.current = false;
      firstRow.current?.scrollIntoView({ block: 'center' });
      firstRow.current?.focus({ preventScroll: true });
    }
  }, [state.page]);
  useEffect(() => {
    const controller = new AbortController(); setBusy(true); setFailed(false);
    const query = { account, cursor: request.cursor, limit: 20 };
    void runScopedRequest(controller.signal, () => demo ? Promise.resolve(mockQuotaHistory(query)) : fetchQuotaHistory(query, controller.signal), {
      success: page => {
        setIndexPending(!page.indexReady);
        setState(old => !page.indexReady && old.page?.indexReady ? old : ({ page, previous: request.kind === 'next' && old.page ? [...old.previous, old.page] : [] }));
      },
      failure: () => setFailed(true), settled: () => setBusy(false),
    });
    return () => controller.abort();
  }, [account, demo, request]);
  useEffect(() => {
    if (!indexPending || busy || failed) return;
    const timer = window.setTimeout(() => setRequest({ kind: 'first' }), 5000);
    return () => window.clearTimeout(timer);
  }, [indexPending, busy, failed]);
  const date = (value: string | null) => {
    if (value === null) return '—';
    try { return new Intl.DateTimeFormat(language === 'en' ? 'en-US' : 'zh-CN', { timeZone: timezone, year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', timeZoneName: 'short' }).format(new Date(value)); }
    catch { return value; }
  };
  const duration = (seconds: string | null) => {
    if (seconds === null) return '—';
    const amount = BigInt(seconds);
    return amount > 0n && amount % 86400n === 0n ? t('history.days', { value: String(amount / 86400n) })
      : amount > 0n && amount % 3600n === 0n ? t('history.hours', { value: String(amount / 3600n) })
      : t('history.seconds', { value: seconds });
  };
  const boundary = (kind: string) => kind === 'observed_decrease' ? t('quota.boundary_decrease')
    : kind === 'deadline_change' ? t('quota.boundary_deadline') : kind === 'window_change' ? t('quota.boundary_window')
    : kind === 'conflicting_timestamp' ? t('quota.boundary_conflict') : kind === 'first_observation' ? t('quota.boundary_first') : t('quota.boundary_unknown');
  const percent = (value: number | null) => value === null ? '—' : `${value.toLocaleString(language, { maximumFractionDigits: 2 })}%`;
  const { page, previous } = state;
  return <section className="panel quota-history-panel" aria-label={t('history.title')}>
    <header><div><h2>{t('history.title')}</h2><p>{t('history.scope')}</p></div>
      <button type="button" disabled={busy} onClick={() => { scrollRequested.current = true; setRequest({ kind: 'first' }); }}>{t('history.refresh')}</button></header>
    {demo && <p className="quota-history-note">{t('history.demo')}</p>}
    {page && <p className="quota-history-note">{t('history.view_time')}: {date(page.view.asOf)} · {t('history.page_number', { value: previous.length + 1 })}</p>}
    {busy && <p role="status">{t('history.loading')}</p>}
    {failed && <p role="alert">{t('history.failed')}</p>}
    {indexPending && <p role="status">{t('history.preparing')}{page?.indexReady && ` ${t('history.retaining_view')}`}</p>}
    {page?.indexReady && !page.intervals.length && <p>{t('history.empty')}</p>}
    <div className="quota-history-list">{page?.intervals.map((row, index) => <article ref={index === 0 ? firstRow : undefined} tabIndex={-1} className="quota-history-item" key={row.id} data-history-id={row.id} data-account-id={row.accountId}>
      <header><div><strong>{row.limitName ?? row.limitId}</strong><span>{accountName(row.accountId)} · {duration(row.windowSeconds)}</span></div><span>{boundary(row.boundaryKind)}</span></header>
      <dl>
        <div><dt>{t('history.first_seen')}</dt><dd>{date(row.firstObservedAt)}</dd></div>
        <div><dt>{t('history.last_seen')}</dt><dd>{date(row.lastObservedAt)}</dd></div>
        <div><dt>{t('history.used_change')}</dt><dd>{percent(row.firstUsedPercent)} → {percent(row.lastUsedPercent)}</dd></div>
        <div><dt>{t('history.samples')}</dt><dd>{row.sampleCount.toLocaleString(language)}</dd></div>
      </dl>
      <details><summary>{t('history.details')}</summary>
        <dl><div><dt>{t('history.reported_reset')}</dt><dd>{date(row.reportedReset)}</dd></div>
          <div><dt>{t('history.nominal_start')}</dt><dd>{date(row.nominalStart)}</dd></div>
          <div><dt>{t('quota.boundary_observations')}</dt><dd>{row.boundaryAfter === null ? '—' : `${date(row.boundaryAfter)} — ${date(row.firstObservedAt)}`}</dd></div>
          <div><dt>{t('quota.observation_interval')}</dt><dd>{row.tokenSampleStart === null ? t('history.no_safe_interval') : `${date(row.tokenSampleStart)} — ${date(row.tokenSampleEnd)}`}</dd></div></dl>
      </details>
      <QuotaIntervalUsage key={`${row.id}:${page?.view.revision}:${page?.view.asOf}`} row={row} selection={page?.selections?.[index]} demo={demo} formatTimestamp={date} accountLabel={accountName(row.accountId)}/>
    </article>)}</div>
    <footer><button type="button" disabled={busy || !previous.length} onClick={() => { scrollRequested.current = true; setFailed(false); setState(old => ({ page: old.previous.at(-1)!, previous: old.previous.slice(0, -1) })); }}>{t('history.previous')}</button>
      <button type="button" disabled={busy || indexPending || !page?.indexReady || !page.next} onClick={() => { if (page?.next) { scrollRequested.current = true; setRequest({ kind: 'next', cursor: page.next }); } }}>{t('history.next')}</button>
      {page?.indexReady && page.next === null && page.intervals.length > 0 && <span>{t('history.end')}</span>}</footer>
  </section>;
}
