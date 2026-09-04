import type { MetricKey, QualityIssue, QualityResponse, QualityStateSummary, SourceHealth } from '../api/types';
import { compactNumber, exactNumber, formatDateTime, metricLabel, metricValue, qualityLabel } from '../lib';
import { EmptyState, Panel } from './Ui';
import { useI18n } from '../i18n';

function formatBytes(value: number): string {
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GB`;
  if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(1)} MB`;
  return `${Math.round(value / 1024)} KB`;
}

function QualityStateCard({ item, metric }: { item: QualityStateSummary; metric: MetricKey }) {
  const { t } = useI18n();
  const value = metric === 'requests'
    ? exactNumber(item.eventCount)
    : item.tokenCount === null
      ? '—'
      : compactNumber(metricValue(item.usage, metric, item.eventCount));
  return (
    <article className={`quality-state-card quality-${item.state}`}>
      <div className="quality-state-heading">
        <span>{item.state === 'confirmed' ? t('components.quality-panel.valid_attribution') : qualityLabel(item.state)}</span>
        <i aria-hidden="true" />
      </div>
      <strong>{value}</strong>
      <small>{exactNumber(item.eventCount)} {t('components.quality-panel.requests')}{item.tokenCount === null && metric !== 'requests' ? ` · ${t('components.quality-panel.token_count_unknown')}` : ''}</small>
      <p>{item.state === 'confirmed' ? t('components.quality-panel.only_the_more_complete_record_source_is') : item.state === 'quarantined' ? t('components.quality-panel.replay_or_source_conflicts_were_detected_quarantined') : t('components.quality-panel.safely_matched_token_details_are_unavailable_and')}</p>
    </article>
  );
}

function IssueRow({ issue }: { issue: QualityIssue }) {
  const { t } = useI18n();
  return (
    <article className={`issue-row severity-${issue.severity}`}>
      <div className="issue-marker" aria-hidden="true">!</div>
      <div>
        <div className="issue-title-row">
          <strong>{issue.title}</strong>
          <span>{qualityLabel(issue.state)}</span>
        </div>
        <p>{issue.detail}</p>
        <small>
          {exactNumber(issue.eventCount)} {t('components.quality-panel.requests')} · {issue.tokenCount === null ? t('components.quality-panel.token_count_unknown') : `${compactNumber(issue.tokenCount)} tokens`} · {t('components.quality-panel.latest')} {formatDateTime(issue.lastSeen)}
        </small>
      </div>
    </article>
  );
}

function SourceRow({ source }: { source: SourceHealth }) {
  const { t } = useI18n();
  return (
    <article className="source-row">
      <span className={`source-status source-${source.status}`} aria-label={source.status === 'fresh' ? t('components.quality-panel.fresh') : source.status === 'delayed' ? t('components.quality-panel.delayed') : t('components.quality-panel.offline')} />
      <div>
        <strong>{source.label}</strong>
        <small>{source.machineLabel}</small>
      </div>
      <div>
        <strong>{source.status === 'fresh' ? t('components.quality-panel.fresh') : source.status === 'delayed' ? t('components.quality-panel.delayed') : t('components.quality-panel.offline')}</strong>
        <small>{source.lagSeconds < 60 ? `${t('components.quality-panel.lag')} ${source.lagSeconds}${t('components.quality-panel.s')}` : `${t('components.quality-panel.lag')} ${Math.round(source.lagSeconds / 60)}${t('components.quality-panel.m')}`}</small>
      </div>
    </article>
  );
}

export function QualityPanel({ data, metric }: { data: QualityResponse; metric: MetricKey }) {
  const { t } = useI18n();
  const reconstruction = data.reconstruction;
  const reconstructionProgress = reconstruction.bytesTotal > 0
    ? Math.min(reconstruction.bytesProcessed / reconstruction.bytesTotal, 1)
    : 1;
  return (
    <section className="quality-layout">
      <Panel
        title={t('components.quality-panel.data_confidence')}
        eyebrow={t('components.quality-panel.local_evidence_ledger')}
        meta={<span className="trusted-policy">{metricLabel(metric)} · {t('components.quality-panel.selected_valid_source')}</span>}
        className="quality-panel"
      >
        <p className="quality-policy">{t('components.quality-panel.only_one_record_source_is_selected_per')}</p>
        <div className="quality-state-grid">
          {data.states.map((item) => <QualityStateCard key={item.state} item={item} metric={metric} />)}
        </div>
        <section className="reconstruction-summary" aria-labelledby="reconstruction-summary-heading">
          <header>
            <div><strong id="reconstruction-summary-heading">{t('components.quality-panel.history_reconstruction')}</strong><span>{t('components.quality-panel.select_either_reconstructed_or_live_records_per')}</span></div>
            <b>{Math.round(reconstructionProgress * 100)}%</b>
          </header>
          <div className="reconstruction-progress" role="progressbar" aria-label={t('components.quality-panel.history_reconstruction_progress_by_bytes_read')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(reconstructionProgress * 100)}><i style={{ width: `${reconstructionProgress * 100}%` }} /></div>
          <div className="reconstruction-stats">
            <div><span>{t('components.quality-panel.reconstructed')}</span><strong>{exactNumber(reconstruction.reconstructedSources)}</strong></div>
            <div><span>{t('components.quality-panel.pending_active')}</span><strong>{exactNumber(reconstruction.pendingSources + reconstruction.reconstructingSources)}</strong></div>
            <div><span>{t('components.quality-panel.unrecoverable')}</span><strong>{exactNumber(reconstruction.unrecoverableSources)}</strong></div>
            <div><span>{t('components.quality-panel.lifetime_selected_attribution')}</span><strong>{compactNumber(reconstruction.selectedTokens)}</strong></div>
          </div>
          <small>{t('components.quality-panel.measured_by_bytes_read')}: {formatBytes(reconstruction.bytesProcessed)} / {formatBytes(reconstruction.bytesTotal)}; {t('components.quality-panel.lifetime_selected_attribution_covers_all_history_and')}</small>
        </section>
        <div className="issue-list">
          <h3>{t('components.quality-panel.needs_attention')}</h3>
          {data.issues.length ? data.issues.map((issue) => <IssueRow key={issue.id} issue={issue} />) : <EmptyState text={t('components.quality-panel.no_active_data_quality_issues')} />}
        </div>
      </Panel>

      <Panel title={t('components.quality-panel.collection_sources')} eyebrow={t('components.quality-panel.data_freshness')} meta={formatDateTime(data.generatedAt)} className="sources-panel">
        <div className="source-list">
          {data.sources.map((source) => <SourceRow key={source.sourceId} source={source} />)}
        </div>
        <div className="method-note">
          <strong>{t('components.quality-panel.status_definitions')}</strong>
          <p>{t('components.quality-panel.valid_attribution_is_deduplicated_pending_unknown_and')}</p>
        </div>
      </Panel>
    </section>
  );
}
