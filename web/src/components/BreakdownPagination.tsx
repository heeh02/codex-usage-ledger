import { useState } from 'react';
import { useI18n } from '../i18n';

const PAGE_SIZE = 20;

/** Presentation-only pagination; never changes the all-row metric denominator. */
export function useBreakdownPage(count: number, scopeKey: string) {
  const [selection, setSelection] = useState({ scopeKey, page: 0 });
  const pages = Math.max(1, Math.ceil(count / PAGE_SIZE));
  const page = selection.scopeKey === scopeKey ? Math.min(selection.page, pages - 1) : 0;
  return { page, pages, offset: page * PAGE_SIZE, end: Math.min((page + 1) * PAGE_SIZE, count),
    setPage: (next: number) => setSelection({ scopeKey, page: Math.max(0, Math.min(next, pages - 1)) }) };
}

export function BreakdownPagination({ count, page, pages, offset, end, setPage }: ReturnType<typeof useBreakdownPage> & { count: number }) {
  const { t } = useI18n();
  return <div className="breakdown-pagination">
    <span role="status">{t('breakdown.visible_rows', { start: count ? offset + 1 : 0, end, count })}</span>
    {pages > 1 && <div>
      <button type="button" disabled={page === 0} onClick={() => setPage(page - 1)}>{t('components.explorer.previous')}</button>
      <button type="button" disabled={page === pages - 1} onClick={() => setPage(page + 1)}>{t('breakdown.next')}</button>
    </div>}
  </div>;
}
