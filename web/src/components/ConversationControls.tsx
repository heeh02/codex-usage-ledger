import { useEffect, useState } from 'react';
import type { DashboardFilters, ExplorerResponse } from '../api/types';
import { useI18n } from '../i18n';

export function ConversationControls({ explorer, filters, onChange }: {
  explorer: ExplorerResponse;
  filters: DashboardFilters;
  onChange: (filters: DashboardFilters) => void;
}) {
  const { t } = useI18n();
  const [search, setSearch] = useState(filters.sessionSearch ?? '');
  useEffect(() => setSearch(filters.sessionSearch ?? ''), [filters.sessionSearch]);
  const page = explorer.sessionPage;
  const offset = page?.offset ?? 0, limit = page?.limit ?? 30;
  return <form className="conversation-controls" onSubmit={event => {
    event.preventDefault();
    onChange({ ...filters, sessionSearch: search, sessionOffset: 0 });
  }}>
    <input aria-label={t('chats.search')} placeholder={t('chats.search')} value={search} onChange={event => setSearch(event.target.value)} />
    <button type="submit">{t('chats.search_action')}</button>
    <select aria-label={t('chats.sort')} value={filters.sessionSort ?? 'tokens'} onChange={event => onChange({ ...filters, sessionSort: event.target.value as DashboardFilters['sessionSort'], sessionOffset: 0 })}>
      <option value="tokens">{t('components.explorer.token_usage')}</option>
      <option value="output">{t('components.explorer.output')}</option>
      <option value="requests">{t('components.ui.requests')}</option>
      <option value="recent">{t('components.explorer.recent_activity')}</option>
    </select>
    <span>{t('chats.page_count', { start: explorer.sessions.length ? offset + 1 : 0, end: offset + explorer.sessions.length, total: page?.total ?? explorer.sessions.length })}</span>
    <button type="button" disabled={offset === 0} onClick={() => onChange({ ...filters, sessionOffset: Math.max(0, offset - limit) })}>{t('chats.previous')}</button>
    <button type="button" disabled={!page?.hasMore} onClick={() => onChange({ ...filters, sessionOffset: offset + limit })}>{t('chats.next')}</button>
  </form>;
}
