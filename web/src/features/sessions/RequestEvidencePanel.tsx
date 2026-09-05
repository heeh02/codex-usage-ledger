import { useEffect, useState } from 'react';
import { fetchRequestEvidence } from '../../api/requestEvidence';
import type { RequestEvidenceCursor, RequestEvidenceResponse } from '../../api/request-evidence.generated';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { useI18n } from '../../i18n';
import './request-evidence.css';

export function RequestEvidencePanel({ threadId, start, end, enabled }: {
  threadId: string; start: string; end: string; enabled: boolean;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [cursor, setCursor] = useState<RequestEvidenceCursor | null>(null);
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
      () => fetchRequestEvidence({ threadId, start, end, after: cursor }, controller.signal), {
        success: setPage,
        failure: reason => setError(reason instanceof Error ? reason.message : String(reason)),
        settled: () => setBusy(false),
      });
    return () => controller.abort();
  }, [threadId, start, end, cursor, open, enabled, retry]);
  return <section className="request-evidence-panel">
    <button type="button" aria-expanded={open} onClick={() => setOpen(value => !value)}>{t('requests.title')}</button>
    {open && <>
      <p>{t('requests.scope')}</p>
      {!enabled ? <p>{t('requests.unavailable_scope')}</p> : <>
        {busy && <p role="status">{t('requests.loading')}</p>}
        {error && <p role="alert">{error} <button type="button" onClick={() => setRetry(value => value + 1)}>{t('requests.retry')}</button></p>}
        {page && <><div className="request-evidence-scroll"><table>
          <thead><tr>{(['time', 'model', 'input', 'read', 'write', 'output', 'quality'] as const).map(key => <th key={key}>{t(`requests.${key}`)}</th>)}</tr></thead>
          <tbody>{page.rows.map(row => <tr key={row.id}>
            <td><time dateTime={row.at}>{row.at}</time></td><td>{row.model ?? '—'}</td>
            <td>{row.usage.uncached.toLocaleString()}</td><td>{row.usage.cached.toLocaleString()}</td>
            <td>{row.usage.cacheWriteCoverage > 0 ? row.usage.cacheWrite.toLocaleString() : '—'}</td>
            <td>{row.usage.output.toLocaleString()}</td><td>{t(`requests.${row.quality}`)}</td>
          </tr>)}</tbody>
        </table></div>
        {!page.rows.length && <p>{t('requests.empty')}</p>}
        <button type="button" disabled={busy || !cursor} onClick={() => setCursor(null)}>{t('requests.first')}</button>
        <button type="button" disabled={busy || !page.next} onClick={() => setCursor(page.next)}>{t('requests.next')}</button></>}
      </>}
    </>}
  </section>;
}
