import type { TokenUsage } from '../api/types';
import { exactNumber, formatTokenMillions, formatPercent } from '../lib';
import { useI18n } from '../i18n';

export function LocalComposition({ usage, eventCount }: { usage: TokenUsage; eventCount: number }) {
  const { t } = useI18n();
  if (eventCount === 0) return <section className="local-composition"><p>{t('usage.no_confirmed_records')}</p></section>;
  const rows = [
    { label: t(usage.cacheWriteCoverage >= 0.999 ? 'components.ui.input_uncached' : 'components.explorer.input_unsplit'), value: usage.uncached, color: 'var(--orange)' },
    { label: t('components.explorer.cache_read'), value: usage.cached, color: 'var(--accent)' },
    { label: t('components.explorer.cache_write'), value: usage.cacheWriteCoverage > 0 ? usage.cacheWrite : null, color: 'var(--purple)' },
    { label: t('components.explorer.output'), value: usage.output, color: 'var(--blue)' },
  ];
  return <section className="local-composition">
    <p title={exactNumber(usage.total)}>{t('app.local_attribution')} · {formatTokenMillions(usage.total)} tokens</p>
    <div className="local-composition-meter" aria-hidden="true">{rows.map(row => <span key={row.label} style={{ width: `${usage.total ? (row.value ?? 0) / usage.total * 100 : 0}%`, background: row.color }} />)}</div>
    <dl className="local-composition-values">{rows.map(row => <div key={row.label}><dt>{row.label}</dt><dd title={row.value === null ? undefined : exactNumber(row.value)}>{formatTokenMillions(row.value)}</dd></div>)}</dl>
    <p>{t('components.explorer.write_field_coverage')} {formatPercent(usage.cacheWriteCoverage)} · {t('components.explorer.reasoning_inside_output')} <span title={exactNumber(usage.reasoning)}>{formatTokenMillions(usage.reasoning)}</span></p>
  </section>;
}
