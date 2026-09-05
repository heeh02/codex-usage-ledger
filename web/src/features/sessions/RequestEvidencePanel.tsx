import { useEffect, useState } from 'react';
import { fetchRequestEvidence } from '../../api/requestEvidence';
import type { RequestEvidenceResponse } from '../../api/request-evidence.generated';
import { advanceRequestPage, firstRequestPage, previousRequestPage } from './requestPaging';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { useI18n } from '../../i18n';
import './request-evidence.css';
import { mockRequestEvidence } from '../../api/requestEvidenceMock';
import { requestTokenDisplay } from './requestValue';

export function RequestEvidencePanel({ threadId, start, end, account, model, demo, revision }: {
  threadId: string; start: string; end: string; account: string; model: string; demo: boolean; revision: object;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [paging, setPaging] = useState(firstRequestPage);
  const cursor = paging.cursor;
  const [page, setPage] = useState<RequestEvidenceResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setBusy(true);
    setError('');
    void runScopedRequest(controller.signal,
      () => demo ? Promise.resolve(mockRequestEvidence({ threadId, start, end, account, model, after: cursor }))
        : fetchRequestEvidence({ threadId, start, end, account, model, after: cursor }, controller.signal), {
        success: setPage,
        failure: reason => setError(reason instanceof Error ? reason.message : String(reason)),
        settled: () => setBusy(false),
      });
    return () => controller.abort();
  }, [threadId, start, end, account, model, cursor, open, demo, retry, revision]);
  return <section className="request-evidence-panel" aria-label={t('requests.title')}>
    <button className="evidence-toggle" type="button" aria-expanded={open} onClick={() => setOpen(value => !value)}><span aria-hidden="true">{open ? '⌄' : '›'}</span>{t('requests.title')}</button>
    {open && <>
      <p>{t('requests.scope')}</p>
      {demo && <p>{t('requests.demo')}</p>}
      <>
      {busy && <p role="status">{t('requests.loading')}</p>}
      {page?.backfillComplete === false && <p role="status">{t('requests.backfill_pending')}</p>}
        {error && <p role="alert">{error} <button type="button" onClick={() => setRetry(value => value + 1)}>{t('requests.retry')}</button></p>}
        {page && <><p role="status">{t('requests.page_summary', {
          count: String(page.rows.length), start: page.rows[0]?.at ?? '—', end: page.rows.at(-1)?.at ?? '—',
        })}</p><div className="request-evidence-scroll" tabIndex={0} role="region" aria-label={t('requests.scroll_region')}><table aria-label={t('requests.title')}>
          <thead><tr>{(['time', 'turn', 'model', 'total', 'input', 'read', 'write', 'output', 'reasoning', 'quality'] as const).map(key => <th key={key}>{t(`requests.${key}`)}</th>)}</tr></thead>
          <tbody>{page.rows.map(row => <tr key={row.id}>
            <td><time dateTime={row.at}>{row.at}</time></td>
            <td title={row.turnId ?? undefined}>{row.turnId ?? t('requests.unknown_turn')}</td><td>{row.model ?? '—'}</td>
            <td><strong>{requestTokenDisplay(row, 'total')}</strong></td>
            <td>{requestTokenDisplay(row, 'uncached')}</td><td>{requestTokenDisplay(row, 'cached')}</td>
            <td>{requestTokenDisplay(row, 'cacheWrite')}</td>
            <td>{requestTokenDisplay(row, 'output')}</td><td>{requestTokenDisplay(row, 'reasoning')}</td><td>{t(`requests.${row.quality}`)}</td>
          </tr>)}</tbody>
        </table></div>
        {!page.rows.length && <p>{t('requests.empty')}</p>}
        <button type="button" disabled={busy || !cursor} onClick={() => setPaging(firstRequestPage())}>{t('requests.first')}</button>
        <button type="button" disabled={busy || !paging.previous.length} onClick={() => setPaging(previousRequestPage)}>{t('requests.previous')}</button>
        <button type="button" disabled={busy || Boolean(error) || !page.next} onClick={() => setPaging(value => advanceRequestPage(value, page.next))}>{t('requests.next')}</button></>}
      </>
    </>}
  </section>;
}
