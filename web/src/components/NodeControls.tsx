import { useEffect, useState } from 'react';
import type { DashboardFilters, ExplorerSessionDetail } from '../api/types';
import { useI18n } from '../i18n';

export function NodeControls({ detail, filters, onChange }: { detail: ExplorerSessionDetail; filters: DashboardFilters; onChange: (filters: DashboardFilters) => void }) {
  const { t } = useI18n();
  const [search, setSearch] = useState(filters.nodeSearch ?? '');
  useEffect(() => setSearch(filters.nodeSearch ?? ''), [filters.nodeSearch]);
  const page = detail.nodePage;
  const offset = page?.offset ?? 0, limit = page?.limit ?? 200;
  return <form className="conversation-controls" onSubmit={event => { event.preventDefault(); onChange({ ...filters, nodeSearch: search, nodeOffset: 0 }); }}>
    <input value={search} aria-label={t('nodes.search_all')} placeholder={t('nodes.search_all')} onChange={event => setSearch(event.target.value)} />
    <button type="submit">{t('chats.search_action')}</button>
    <span>{t('chats.page_count', { start: detail.nodes.length ? offset + 1 : 0, end: offset + detail.nodes.length, total: page?.total ?? detail.nodes.length })}</span>
    <button type="button" disabled={offset === 0} onClick={() => onChange({ ...filters, nodeOffset: Math.max(0, offset - limit) })}>{t('chats.previous')}</button>
    <button type="button" disabled={!page?.hasMore} onClick={() => onChange({ ...filters, nodeOffset: offset + limit })}>{t('chats.next')}</button>
  </form>;
}
