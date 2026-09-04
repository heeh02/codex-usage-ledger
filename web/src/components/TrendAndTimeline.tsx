import { useEffect, useRef, useState } from 'react';
import type { ExplorerResponse, MetricKey, TimeseriesResponse } from '../api/types';
import { compactNumber, dimensionLabel, formatDateTime, formatPercent, metricLabel, metricValue, shortDate } from '../lib';
import { civilTime, contiguous, nextBucket, timeDomain, timeRatio, trendSeries } from '../charts/time';
import { UsageTrendChart } from './UsageTrendChart';
import { useI18n } from '../i18n';
import { EmptyState, Panel } from './Ui';

interface ChartGeometry {
  width: number;
  height: number;
  padding: { top: number; right: number; bottom: number; left: number };
}

function useChartGeometry() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(640);
  useEffect(() => {
    if (!ref.current) return;
    const observer = new ResizeObserver(entries => setWidth(Math.max(280, Math.round(entries[0].contentRect.width))));
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);
  return { width, height: 260, padding: { top: 24, right: 18, bottom: 36, left: 66 }, ref };
}

function pointXAtRatio(ratio: number, geometry: ChartGeometry): number {
  return geometry.padding.left + ratio * (geometry.width - geometry.padding.left - geometry.padding.right);
}

function pointY(value: number, max: number, geometry: ChartGeometry): number {
  const innerHeight = geometry.height - geometry.padding.top - geometry.padding.bottom;
  return geometry.padding.top + innerHeight - (value / Math.max(max, 1)) * innerHeight;
}


function ChartAxes({ geometry, domain, max }: { geometry: ChartGeometry; domain: [number, number]; max: number }) {
  return <>
    {[0, 0.5, 1].map(ratio => <g key={ratio}>
      <line x1={geometry.padding.left} x2={geometry.width - geometry.padding.right} y1={pointY(max * ratio, max, geometry)} y2={pointY(max * ratio, max, geometry)} className="chart-gridline" />
      <text x={geometry.padding.left - 9} y={pointY(max * ratio, max, geometry) + 4} textAnchor="end" className="chart-axis-label">{compactNumber(max * ratio)}</text>
      <text x={pointXAtRatio(ratio, geometry)} y={geometry.height - 12} textAnchor={ratio === 0 ? 'start' : ratio === 1 ? 'end' : 'middle'} className="chart-date-label">{shortDate(new Date(domain[0] + (domain[1] - domain[0]) * ratio).toISOString().slice(0, 16))}</text>
    </g>)}
  </>;
}

function CompositionChart({ data }: { data: TimeseriesResponse }) {
  const { t } = useI18n();
  const geometry = useChartGeometry();
  const { width, height, padding } = geometry;
  const points = data.points;
  const domain = timeDomain(points.map(p => p.date), data.grain, data.period);
  if (points.length) domain[1] = Math.max(domain[1], ...points.map(p => nextBucket(p.date, data.grain)));
  const max = Math.max(...points.map((point) => point.confirmed.total), 1);
  const input = points.reduce((sum, point) => sum + point.confirmed.input, 0);
  const cacheWriteObservedInput = points.reduce((sum, point) => sum + point.confirmed.cacheWriteObservedInput, 0);
  const cacheWriteCoverage = input ? cacheWriteObservedInput / input : 0;
  return (
    <div className="composition-chart-wrap usage-time-chart" ref={geometry.ref}>
      {!points.length && <EmptyState text={t('components.trend-and-timeline.no_token_composition_can_be_drawn_within')} />}
      <svg className="trend-chart composition-area-chart" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={t('components.trend-and-timeline.input_cache_read_cache_write_and_output')}>
        <ChartAxes geometry={geometry} domain={domain} max={max} />
        {points.map(point => {
          const usage = point.confirmed;
          const barWidth = Math.max(2, Math.min(44, (nextBucket(point.date, data.grain) - civilTime(point.date)) / Math.max(domain[1] - domain[0], 1) * (width - padding.left - padding.right) * 0.8));
          let lower = 0;
          return <g key={point.date}><title>{point.date} · {usage.total.toLocaleString()} · {t('components.explorer.input')} {usage.uncached.toLocaleString()} · {t('components.explorer.cache_read')} {usage.cached.toLocaleString()} · {t('components.explorer.cache_write')} {usage.cacheWriteCoverage > 0 ? usage.cacheWrite.toLocaleString() : '—'} · {t('components.explorer.output')} {usage.output.toLocaleString()}</title>
            {([['uncached', usage.uncached], ['cached', usage.cached], ['cache-write', usage.cacheWrite], ['output', usage.output]] as const).map(([kind, value]) => {
              const upper = lower + value;
              const bottomY = pointY(lower, max, geometry);
              lower = upper;
              return <rect key={kind} x={pointXAtRatio(timeRatio(point.date, domain), geometry)} y={pointY(upper, max, geometry)} width={barWidth} height={bottomY - pointY(upper, max, geometry)} className={`stack-area stack-${kind}`} />;
            })}
          </g>;
        })}
      </svg>
      <div className="chart-footer"><div className="chart-legend"><span><i className="legend-uncached" />{t('components.explorer.input')}</span><span><i className="legend-cached" />{t('components.explorer.cache_read')}</span><span><i className="legend-cache-write" />{t('components.explorer.cache_write')}</span><span><i className="legend-output" />{t('components.explorer.output')}</span></div><span className="hover-hint">{t('components.trend-and-timeline.local_four_buckets')} · {t('components.explorer.write_field_coverage')} {formatPercent(cacheWriteCoverage)}</span></div>
    </div>
  );
}

const PROJECT_COLORS = ['#168f70', '#5886e8', '#8b64d3', '#d77a28', '#7b858c'];

function ProjectCompareChart({ data, metric }: { data: TimeseriesResponse; metric: MetricKey }) {
  const { t } = useI18n();
  const geometry = useChartGeometry();
  const { width, height, padding } = geometry;
  const ranked = [...data.projectSeries].sort((left, right) => {
    const leftTotal = left.points.reduce((sum, point) => sum + metricValue(point.confirmed, metric, point.confirmedEvents), 0);
    const rightTotal = right.points.reduce((sum, point) => sum + metricValue(point.confirmed, metric, point.confirmedEvents), 0);
    return rightTotal - leftTotal;
  });
  const [selected, setSelected] = useState<string[]>(() => ranked.slice(0, 5).map((series) => series.id));
  const seriesKey = ranked.map((series) => series.id).join('|');
  useEffect(() => setSelected(ranked.slice(0, 5).map((series) => series.id)), [seriesKey, metric]);
  const visible = ranked.filter((series) => selected.includes(series.id)).slice(0, 5);
  const domain = timeDomain(data.projectSeries.flatMap(series => series.points.map(point => point.date)), data.grain, data.period);
  const max = Math.max(...visible.flatMap((series) => series.points.map((point) => metricValue(point.confirmed, metric, point.confirmedEvents))), 1);
  const toggle = (id: string) => setSelected((current) => current.includes(id) ? current.filter((value) => value !== id) : current.length < 5 ? [...current, id] : current);
  return (
    <div className="project-compare-wrap usage-time-chart" ref={geometry.ref}>
      <div className="project-series-picker">{ranked.slice(0, 10).map((series) => <button className={selected.includes(series.id) ? 'is-selected' : ''} key={series.id} onClick={() => toggle(series.id)} type="button">{dimensionLabel(series.id, series.label)}</button>)}</div>
      {visible.length ? <svg className="trend-chart project-compare-chart" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={t('components.trend-and-timeline.project_usage_comparison')}>
        <ChartAxes geometry={geometry} domain={domain} max={max} />
        {visible.map((series, seriesIndex) => contiguous(series.points, data.grain, () => true).map((segment, index) => <g key={`${series.id}-${index}`}>
          <polyline points={segment.map(point => `${pointXAtRatio(timeRatio(point.date, domain), geometry)},${pointY(metricValue(point.confirmed, metric, point.confirmedEvents), max, geometry)}`).join(' ')} className="trend-line project-compare-line" style={{ stroke: PROJECT_COLORS[seriesIndex] }} />
          {segment.length === 1 && <circle cx={pointXAtRatio(timeRatio(segment[0].date, domain), geometry)} cy={pointY(metricValue(segment[0].confirmed, metric, segment[0].confirmedEvents), max, geometry)} r={3} fill={PROJECT_COLORS[seriesIndex]} />}
        </g>))}
      </svg> : <EmptyState text={t('components.trend-and-timeline.select_1_5_projects_to_compare')} />}
      <div className="chart-footer"><div className="chart-legend">{visible.map((series, index) => <span key={series.id}><i style={{ background: PROJECT_COLORS[index] }} />{dimensionLabel(series.id, series.label)}</span>)}</div><span className="hover-hint">{t('components.trend-and-timeline.up_to_5_projects')}</span></div>
    </div>
  );
}

function ProjectRanking({ explorer, metric, onOpenProject }: { explorer: ExplorerResponse; metric: MetricKey; onOpenProject: (projectId: string) => void }) {
  const { t } = useI18n();
  const value = (project: ExplorerResponse['projects'][number]) => metricValue(project.periodUsage, metric, project.periodEvents);
  const allProjects = [...explorer.projects];
  const rankedProjects = allProjects
    .filter((project) => value(project) > 0)
    .sort((left, right) => value(right) - value(left));
  const projects = rankedProjects.slice(0, 7);
  const otherProjects = rankedProjects.slice(7);
  const otherTotal = otherProjects.reduce((sum, project) => sum + value(project), 0);
  const max = projects[0] ? value(projects[0]) : 1;
  const localTotal = allProjects.reduce((sum, project) => sum + value(project), 0);
  const denominator = localTotal;
  const sparkline = (values: number[]) => {
    if (!values.length) return '';
    const maxValue = Math.max(...values, 1);
    return values.map((item, index) => `${(index / Math.max(values.length - 1, 1)) * 72},${18 - (item / maxValue) * 16}`).join(' ');
  };
  const deltaLabel = (project: ExplorerResponse['projects'][number]) => {
    const previous = metricValue(project.previousPeriodUsage, metric, project.previousPeriodEvents);
    if (previous <= 0) return t('components.explorer.no_comparable_coverage');
    const delta = (value(project) - previous) / previous;
    return `${delta >= 0 ? '+' : ''}${(delta * 100).toFixed(0)}%`;
  };
  return projects.length ? (
    <div className="project-ranking-list">
      {projects.map((project, index) => (
        <button key={project.id} type="button" onClick={() => onOpenProject(project.id)}>
          <span className="ranking-index">{index + 1}</span>
          <div>
            <div><strong>{dimensionLabel(project.id, project.label)}</strong><span>{compactNumber(value(project))}</span></div>
            <i><span style={{ width: `${(value(project) / max) * 100}%` }} /></i>
            <small>{denominator ? `${t('components.trend-and-timeline.local_project_sample')} ${((value(project) / denominator) * 100).toFixed(1)}% · ` : ''}{deltaLabel(project)} · {project.activeSessionCount} active / {project.sessionCount} sessions · {formatDateTime(project.lastActiveAt)}</small>
          </div>
          <svg viewBox="0 0 72 20" aria-label={`${dimensionLabel(project.id, project.label)} sparkline`}><polyline points={sparkline(project.sparkline)} /></svg>
        </button>
      ))}
      {otherTotal > 0 && (
        <div className="project-gap-row">
          <span className="ranking-index">…</span>
          <div><div><strong>{t('components.trend-and-timeline.other_projects')} · {otherProjects.length}</strong><span>{compactNumber(otherTotal)}</span></div><i><span style={{ width: `${(otherTotal / max) * 100}%` }} /></i><small>{t('components.trend-and-timeline.local_project_sample')} {denominator ? `${((otherTotal / denominator) * 100).toFixed(1)}%` : '—'} · {t('components.trend-and-timeline.included_in_denominator')}</small></div>
        </div>
      )}
    </div>
  ) : <EmptyState text={t('components.trend-and-timeline.no_samples_are_attributable_to_local_projects')} />;
}

export function UsageTrendPanel({ data, metric, className = '', title = '用量趋势', allowProjectCompare = true }: { data: TimeseriesResponse; metric: MetricKey; className?: string; title?: string; allowProjectCompare?: boolean }) {
  const { t } = useI18n();
  const displayTitle = title === '用量趋势' ? t('components.trend-and-timeline.usage_trend') : title;
  const officialScope = metric === 'total' && data.official.primaryScope && data.official.accountCount > 0;
  const [view, setView] = useState<'total' | 'composition' | 'projects'>('total');
  const chartGrain = view === 'total' ? trendSeries(data, metric).grain : data.grain;
  useEffect(() => {
    if (!allowProjectCompare && view === 'projects') setView('total');
  }, [allowProjectCompare, view]);
  return (
    <Panel
      title={displayTitle}
      eyebrow={officialScope ? t('components.trend-and-timeline.official_account_trend') : t('app.local_attribution')}
      meta={<div className="trend-panel-meta"><span>{chartGrain === 'hour' ? t('components.trend-and-timeline.hourly') : chartGrain === 'day' ? t('components.trend-and-timeline.daily') : chartGrain === 'week' ? t('components.trend-and-timeline.weekly') : t('components.trend-and-timeline.monthly')}</span><div className="trend-view-control"><button aria-pressed={view === 'total'} className={view === 'total' ? 'is-active' : ''} onClick={() => setView('total')} type="button">{t('components.trend-and-timeline.total')}</button><button aria-pressed={view === 'composition'} className={view === 'composition' ? 'is-active' : ''} onClick={() => setView('composition')} type="button">{t('components.trend-and-timeline.composition')}</button>{allowProjectCompare && <button aria-pressed={view === 'projects'} className={view === 'projects' ? 'is-active' : ''} onClick={() => setView('projects')} type="button">{t('components.trend-and-timeline.compare_projects')}</button>}</div></div>}
      className={`trend-panel ${className}`}
    >
      {view === 'total' && <UsageTrendChart data={data} metric={metric} />}
      {view === 'composition' && <CompositionChart data={data} />}
      {view === 'projects' && <ProjectCompareChart data={data} metric={metric} />}
    </Panel>
  );
}

export function TrendAndTimeline({
  data,
  explorer,
  metric,
  onOpenProject,
}: {
  data: TimeseriesResponse;
  explorer: ExplorerResponse;
  metric: MetricKey;
  onOpenProject: (projectId: string) => void;
}) {
  const { t } = useI18n();
  return (
    <section className="trend-timeline-grid">
      <UsageTrendPanel data={data} metric={metric} />
      <Panel title={t('components.trend-and-timeline.project_ranking')} eyebrow={t('app.local_attribution')} meta={<span className="definition-chip">{t('components.trend-and-timeline.local_attribution_sample')}</span>} className="timeline-panel">
        <ProjectRanking explorer={explorer} metric={metric} onOpenProject={onOpenProject} />
      </Panel>
    </section>
  );
}
