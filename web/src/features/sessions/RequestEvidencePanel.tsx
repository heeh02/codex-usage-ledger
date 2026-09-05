import { useEffect, useState } from 'react';
import { fetchRequestEvidence } from '../../api/requestEvidence';
import type { RequestEvidenceResponse } from '../../api/request-evidence.generated';
import { advanceRequestPage, firstRequestPage, previousRequestPage } from './requestPaging';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { useI18n } from '../../i18n';
import './request-evidence.css';
import { mockRequestEvidence } from '../../api/requestEvidenceMock';

export function RequestEvidencePanel({ threadId, start, end, enabled, demo }: {
  threadId: string; start: string; end: string; enabled: boolean; demo: boolean;
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
    if (!open || !enabled) return;
    const controller = new AbortController();
    setBusy(true);
    setError('');
    void runScopedRequest(controller.signal,
      () => demo ? Promise.resolve(mockRequestEvidence({ threadId, start, end, after: cursor }))
        : fetchRequestEvidence({ threadId, start, end, after: cursor }, controller.signal), {
        success: setPage,
        failure: reason => setError(reason instanceof Error ? reason.message : String(reason)),
        settled: () => setBusy(false),
      });
    return () => controller.abort();
  }, [threadId, start, end, cursor, open, enabled, demo, retry]);
  return <section className="request-evidence-panel" aria-label={t('requests.title')}>
    <button type="button" aria-expanded={open} onClick={() => setOpen(value => !value)}>{t('requests.title')}</button>
    {open && <>
      <p>{t('requests.scope')}</p>
      {demo && <p>{t('requests.demo')}</p>}
      {!enabled ? <p>{t('requests.unavailable_scope')}</p> : <>
        {busy && <p role="status">{t('requests.loading')}</p>}
        {error && <p role="alert">{error} <button type="button" onClick={() => setRetry(value => value + 1)}>{t('requests.retry')}</button></p>}
        {page && <><p role="status">{t('requests.page_summary', {
          count: String(page.rows.length), start: page.rows[0]?.at ?? '—', end: page.rows.at(-1)?.at ?? '—',
        })}</p><div className="request-evidence-scroll"><table aria-label={t('requests.title')}>
          <thead><tr>{(['time', 'turn', 'model', 'input', 'read', 'write', 'output', 'quality'] as const).map(key => <th key={key}>{t(`requests.${key}`)}</th>)}</tr></thead>
          <tbody>{page.rows.map(row => <tr key={row.id}>
            <td><time dateTime={row.at}>{row.at}</time></td>
            <td title={row.turnId ?? undefined}>{row.turnId ?? t('requests.unknown_turn')}</td><td>{row.model ?? '—'}</td>
            <td>{row.usage.uncached.toLocaleString()}</td><td>{row.usage.cached.toLocaleString()}</td>
            <td>{row.usage.cacheWriteCoverage > 0 ? row.usage.cacheWrite.toLocaleString() : '—'}</td>
            <td>{row.usage.output.toLocaleString()}</td><td>{t(`requests.${row.quality}`)}</td>
          </tr>)}</tbody>
        </table></div>
        {!page.rows.length && <p>{t('requests.empty')}</p>}
        <button type="button" disabled={busy || !cursor} onClick={() => setPaging(firstRequestPage())}>{t('requests.first')}</button>
        <button type="button" disabled={busy || !paging.previous.length} onClick={() => setPaging(previousRequestPage)}>{t('requests.previous')}</button>
        <button type="button" disabled={busy || Boolean(error) || !page.next} onClick={() => setPaging(value => advanceRequestPage(value, page.next))}>{t('requests.next')}</button></>}
      </>}
    </>}
  </section>;
}
