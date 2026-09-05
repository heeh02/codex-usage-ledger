import { useEffect, useState } from 'react';
import { fetchTurnEvidence } from '../../api/turnEvidence';
import { mockTurnEvidence } from '../../api/turnEvidenceMock';
import type { TurnEvidenceResponse } from '../../api/turn-evidence.generated';
import { runScopedRequest } from '../../shared/requestLifecycle';
import { useI18n } from '../../i18n';
import './request-evidence.css';

export function TurnEvidencePanel({ threadId, start, end, account, model, demo }: {
  threadId: string; start: string; end: string; account: string; model: string; demo: boolean;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false), [offset, setOffset] = useState(0);
  const [page, setPage] = useState<TurnEvidenceResponse | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState(''), [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setBusy(true); setError('');
    const query = { threadId, start, end, account, model, offset, limit: 50 };
    void runScopedRequest(controller.signal, () => demo ? Promise.resolve(mockTurnEvidence(query))
      : fetchTurnEvidence(query, controller.signal), {
      success: setPage, failure: reason => setError(String(reason)), settled: () => setBusy(false),
    });
    return () => controller.abort();
  }, [open, threadId, start, end, account, model, offset, demo, retry]);
  return <section className="request-evidence-panel" aria-label={t('turns.title')}>
    <button className="evidence-toggle" type="button" aria-expanded={open} onClick={() => setOpen(value => !value)}><span aria-hidden="true">{open ? '⌄' : '›'}</span>{t('turns.title')}</button>
    {open && <><p>{t('turns.scope')}</p>{demo && <p>{t('requests.demo')}</p>}
      {busy && <p role="status">{t('requests.loading')}</p>}
      {error && <p role="alert">{error} <button type="button" onClick={() => setRetry(value => value + 1)}>{t('requests.retry')}</button></p>}
      {page && <><div className="request-evidence-scroll" tabIndex={0} role="region" aria-label={t('turns.title')}>
        <table><thead><tr>{(['turn', 'time', 'count', 'total', 'input', 'read', 'write', 'output'] as const).map(key =>
          <th key={key}>{t(`turns.${key}`)}</th>)}</tr></thead>
          <tbody>{page.rows.map(row => <tr key={row.groupId}>
            <td>{row.turnId ?? t('requests.unknown_turn')}</td><td>{row.firstAt}</td>
            <td>{row.confirmedRequestCount}/{row.requestCount}</td>
            {(['total', 'uncached', 'cached', 'cacheWrite', 'output'] as const).map(field => <td key={field}>
              {row.confirmedUsage && (field !== 'cacheWrite' || row.confirmedUsage.cacheWriteCoverage > 0)
                ? row.confirmedUsage[field].toLocaleString() : '—'}
            </td>)}
          </tr>)}</tbody></table>
      </div>
      {!page.rows.length && <p>{t('requests.empty')}</p>}
      <button type="button" disabled={busy || offset === 0} onClick={() => setOffset(Math.max(0, offset - 50))}>{t('requests.previous')}</button>
      <button type="button" disabled={busy || Boolean(error) || page.nextOffset === null} onClick={() => setOffset(page.nextOffset ?? offset)}>{t('requests.next')}</button></>}
    </>}
  </section>;
}
