import { useId, useRef } from 'react';
import type { DimensionOption, OfficialAccountRow } from '../api/types';
import { useI18n } from '../i18n';
import './account-switcher.css';

export function accountScopeLabel(id: string, options: DimensionOption[], rows: OfficialAccountRow[], allLabel: string): string {
  if (id === 'all') return allLabel;
  return rows.find(row => row.id === id)?.label
    ?? options.find(option => option.id === id)?.label.replace(/^(当前账号|已校准账号|历史账号)\s*·\s*/, '')
    ?? (id.length > 12 ? `${id.slice(0,8)}…${id.slice(-4)}` : id);
}

export function accountPlanLabel(plan: string | null | undefined): string {
  const value = (plan ?? '').toLowerCase().replace(/[-_ ]/g, '');
  if (value.includes('pro') && value.includes('20')) return 'Pro 20×';
  if (value.includes('pro') && value.includes('5')) return 'Pro 5×';
  if (value.includes('pro')) return 'Pro';
  if (value === 'plus') return 'Plus';
  if (value === 'free') return 'Free';
  return plan || '—';
}

export function AccountSwitcher({ options, rows, selected, pending, onSelect }: {
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
  const tierOrder = ['Pro 20×', 'Pro 5×', 'Pro', 'Plus', 'Free'];
  const rank = (id: string) => {
    if (id === 'all') return -1;
    const index = tierOrder.indexOf(accountPlanLabel(rows.find(row => row.id === id)?.planType));
    return index < 0 ? tierOrder.length : index;
  };
  const ids = Array.from(new Set(['all', ...options.map(option => option.id), ...rows.map(row => row.id)]))
    .sort((a, b) => rank(a) - rank(b));
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
        <p>{t('account-switcher.scope_only')}</p>
        <div className="account-switcher-options" role="group" aria-label={t('components.ui.account')}>
          {ids.map(id => {
            const row = rows.find(row => row.id === id);
            return <button type="button" key={id} className="account-switcher-option" data-account={id}
              aria-pressed={selected === id} onClick={() => { onSelect(id); close(); }}>
              <span className="account-switcher-avatar" aria-hidden="true">{id === 'all' ? '◎' : accountPlanLabel(row?.planType).slice(0, 1)}</span>
              <span className="account-switcher-option-name"><strong>{label(id)}</strong>
                {row?.active && <small>{t('account-switcher.observed_login')}</small>}</span>
              {id !== 'all' && <span className="account-switcher-plan">{accountPlanLabel(row?.planType)}</span>}
              <span className="account-switcher-check" aria-hidden="true">{selected === id ? '✓' : ''}</span>
            </button>;
          })}
        </div>
        {pending && <p role="status">{t('account-switcher.applying')}</p>}
      </div>
    </dialog>
  </div>;
}
