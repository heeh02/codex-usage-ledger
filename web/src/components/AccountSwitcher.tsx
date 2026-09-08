import { useId, useRef } from 'react';
import type { DimensionOption, OfficialAccountRow } from '../api/types';
import { useI18n } from '../i18n';
import { formatDateTime } from '../lib';
import './account-switcher.css';

export function accountScopeLabel(id: string, options: DimensionOption[], rows: OfficialAccountRow[], allLabel: string): string {
  if (id === 'all') return allLabel;
  return rows.find(row => row.id === id)?.label
    ?? options.find(option => option.id === id)?.label.replace(/^(当前账号|已校准账号|历史账号)\s*·\s*/, '')
    ?? (id.length > 12 ? `${id.slice(0,8)}…${id.slice(-4)}` : id);
}

export function AccountSwitcher({ options, rows, selected, pending, onSelect, onAccounts }: {
  options: DimensionOption[];
  rows: OfficialAccountRow[];
  selected: string;
  pending: boolean;
  onSelect: (id: string) => void;
  onAccounts: () => void;
}) {
  const { t } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const heading = useId();
  const label = (id: string) => accountScopeLabel(id, options, rows, t('components.ui.all_accounts'));
  const active = rows.filter(row => row.active);
  const close = () => { dialog.current?.close(); trigger.current?.focus(); };
  return <div className="account-switcher">
    <button ref={trigger} className="account-switcher-trigger" data-account={selected} type="button"
      aria-haspopup="dialog" aria-label={`${t('account-switcher.viewing')} ${label(selected)}`}
      onClick={() => dialog.current?.showModal()}>
      <span className="account-switcher-avatar" aria-hidden="true">{selected === 'all' ? '◎' : label(selected).slice(0, 1)}</span>
      <span><small>{t('account-switcher.viewing')}</small><strong>{label(selected)}</strong></span><span aria-hidden="true">⌃</span>
    </button>
    <dialog ref={dialog} className="account-switcher-dialog" aria-labelledby={heading}
      onClose={() => trigger.current?.focus()} onClick={event => { if (event.target === dialog.current) close(); }}>
      <div className="account-switcher-body">
        <header><h2 id={heading}>{t('account-switcher.title')}</h2><button type="button" onClick={close} aria-label={t('account-switcher.close')}>×</button></header>
        <p>{t('account-switcher.read_only')}</p>
        <label>{t('components.ui.account')}<select aria-label={t('components.ui.account')} value={selected} disabled={pending || options.length === 0}
          onChange={event => { onSelect(event.target.value); close(); }}>
          {!options.some(option => option.id === selected) && <option value={selected}>{label(selected)}</option>}
          {options.map(option => <option key={option.id} value={option.id}>{label(option.id)}</option>)}
        </select></label>
        {pending && <p role="status">{t('account-switcher.applying')}</p>}
        <section className="account-switcher-login"><strong>{t('account-switcher.observed_login')}</strong>
          <span>{active.length === 1 ? label(active[0].id) : t('account-switcher.login_unknown')}</span>
        </section>
        <div className="account-switcher-records">{rows.map(row => <article key={row.id}>
          <strong>{label(row.id)}</strong><span>{row.planType ?? '—'}{row.active ? ` · ${t('account-switcher.observed_login')}` : ''}</span>
          <small>{t('account-switcher.official_observed')} {row.observedAt && Number.isFinite(Date.parse(row.observedAt)) ? formatDateTime(row.observedAt) : '—'}</small>
        </article>)}</div>
        <button type="button" className="account-switcher-manage" onClick={() => { close(); onAccounts(); }}>{t('app.accounts_quota')}</button>
      </div>
    </dialog>
  </div>;
}
