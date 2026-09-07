import type { ExplorerSessionDetail } from '../../api/types';
import { Panel, EmptyState } from '../../components/Ui';
import { UsageBreakdownTable } from '../../components/UsageBreakdownTable';
import { useI18n } from '../../i18n';

export function SessionDistributions({ detail, scope }: { detail: Pick<ExplorerSessionDetail, 'localDistributions'>; scope: 'own' | 'tree' }) {
  const { t } = useI18n();
  const data = detail.localDistributions?.[scope];
  return <Panel title={t('sessions.distributions')} eyebrow={t('app.local_attribution')}>
    <p>{t(scope === 'own' ? 'sessions.distributions_own' : 'sessions.distributions_tree')}</p>
    {(['models', 'accounts'] as const).map(dimension => <details key={dimension} open>
      <summary>{t(dimension === 'models' ? 'sessions.models_used' : 'sessions.accounts_used')}</summary>
      {!data?.[dimension] ? <EmptyState text={t('sessions.distributions_unavailable')} /> :
        <UsageBreakdownTable rows={data[dimension]!} identityLabel={t(dimension === 'models' ? 'sessions.models_used' : 'sessions.accounts_used')}/>}
    </details>)}
  </Panel>;
}
