import type { ExplorerSessionDetail } from '../../api/types';
import { Panel, EmptyState } from '../../components/Ui';
import { useI18n } from '../../i18n';
import { compactNumber } from '../../lib';
import './request-evidence.css';

export function SessionDistributions({ detail, scope }: { detail: Pick<ExplorerSessionDetail, 'localDistributions'>; scope: 'own' | 'tree' }) {
  const { t }=useI18n();
  const data=detail.localDistributions?.[scope];
  return <Panel title={t('sessions.distributions')} eyebrow={t('app.local_attribution')}>
    <p>{t(scope==='own' ? 'sessions.distributions_own' : 'sessions.distributions_tree')}</p>
    {(['models','accounts'] as const).map(dimension => <details key={dimension} open>
      <summary>{t(dimension==='models' ? 'sessions.models_used' : 'sessions.accounts_used')}</summary>
      {!data?.[dimension] ? <EmptyState text={t('sessions.distributions_unavailable')} /> :
        <div className="request-evidence-scroll" tabIndex={0} role="region" aria-label={t(dimension==='models' ? 'sessions.models_used' : 'sessions.accounts_used')}>
          <table><thead><tr>
            <th>{t(dimension==='models' ? 'sessions.models_used' : 'sessions.accounts_used')}</th>
            {(['total','uncached','cached','cacheWrite','output','events'] as const).map(key=><th key={key}>{t(`sessions.mix_${key}`)}</th>)}
          </tr></thead><tbody>{data[dimension]!.map(row=><tr key={JSON.stringify(row.id)}>
            <td>{row.id===null ? t('sessions.unknown_dimension') : row.label}</td>
            {(['total','uncached','cached','cacheWrite','output'] as const).map(key=><td key={key} title={key==='cacheWrite'&&row.usage.cacheWriteCoverage===0 ? undefined : row.usage[key].toLocaleString()}>
              {key==='cacheWrite' && row.usage.cacheWriteCoverage===0 ? '—' : compactNumber(row.usage[key])}
              {key==='cacheWrite' && row.usage.cacheWriteCoverage>0 && row.usage.cacheWriteCoverage<1 ? ` (${t('sessions.partial_split')})` : ''}
            </td>)}<td>{row.events.toLocaleString()}</td>
          </tr>)}</tbody></table>
        </div>}
    </details>)}
  </Panel>;
}
