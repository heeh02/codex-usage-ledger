import { useEffect, useState } from 'react';
import type { ExplorerResponse, MetricKey, TimeseriesResponse } from '../api/types';
import { compactNumber, currentUiLanguage, dimensionLabel, formatDateTime, formatPercent, metricLabel, metricValue, shortDate } from '../lib';
import { useI18n } from '../i18n';
import { EmptyState, Panel } from './Ui';

interface ChartGeometry {
  width: number;
  height: number;
  padding: { top: number; right: number; bottom: number; left: number };
}

const WIDE_GEOMETRY: ChartGeometry = { width: 920, height: 300, padding: { top: 38, right: 22, bottom: 38, left: 58 } };
const COMPACT_GEOMETRY: ChartGeometry = { width: 640, height: 320, padding: { top: 42, right: 18, bottom: 42, left: 56 } };

function useChartGeometry(): ChartGeometry {
  const query = '(max-width: 900px)';
  const [compact, setCompact] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = (event: MediaQueryListEvent) => setCompact(event.matches);
    setCompact(media.matches);
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);
  return compact ? COMPACT_GEOMETRY : WIDE_GEOMETRY;
}

function pointX(index: number, count: number, geometry: ChartGeometry): number {
  return geometry.padding.left + (index / Math.max(count - 1, 1)) * (geometry.width - geometry.padding.left - geometry.padding.right);
}

function pointXAtRatio(ratio: number, geometry: ChartGeometry): number {
  return geometry.padding.left + ratio * (geometry.width - geometry.padding.left - geometry.padding.right);
}

function pointY(value: number, max: number, geometry: ChartGeometry): number {
  const innerHeight = geometry.height - geometry.padding.top - geometry.padding.bottom;
  return geometry.padding.top + innerHeight - (value / Math.max(max, 1)) * innerHeight;
}

interface ChartPoint {
  date: string;
  official: number;
  local: number;
  previous: number;
  xRatio: number;
  statusLabel?: string;
  sourceDetail?: string;
}

function seriesPath(points: ChartPoint[], field: 'official' | 'local' | 'previous', max: number, geometry: ChartGeometry): string {
  return points
    .map((point) => `${pointXAtRatio(point.xRatio, geometry).toFixed(1)},${pointY(point[field], max, geometry).toFixed(1)}`)
    .join(' ');
}

function mergeChartPoints(data: TimeseriesResponse, metric: MetricKey): ChartPoint[] {
  const zh = currentUiLanguage() === 'zh-CN';
  const localByDate = new Map(data.points.map((point) => [point.date, metricValue(point.confirmed, metric, point.confirmedEvents)]));
  if (metric === 'total' && data.official.primaryScope && data.official.reconciledPoints.length) {
    return data.official.reconciledPoints.map((point, index) => ({
      date: point.date,
      official: point.value,
      local: localByDate.get(point.date) ?? 0,
      previous: data.official.reconciledComparisonPoints[index]?.value ?? 0,
      xRatio: index / Math.max(data.official.reconciledPoints.length - 1, 1),
      statusLabel: point.status === 'exact_official'
        ? (zh ? '官方精确' : 'Official exact')
        : point.status === 'local_tail'
          ? (zh ? '官方 + 本机尾部下限' : 'Official + local tail floor')
          : point.status === 'local_only_account'
            ? (zh ? '已捕获账号缺官方档案 · 当前为下限' : 'Captured account lacks archive · lower bound')
            : (zh ? '覆盖未知' : 'Coverage unknown'),
      sourceDetail: zh
        ? `官方 ${compactNumber(point.officialTokens)} · 本机尾部 ${compactNumber(point.localTailTokens)} · 已捕获缺档账号 ${compactNumber(point.localOnlyTokens)} · 覆盖 ${point.coveredAccounts}/${point.knownAccounts}`
        : `Official ${compactNumber(point.officialTokens)} · local tail ${compactNumber(point.localTailTokens)} · captured without archive ${compactNumber(point.localOnlyTokens)} · coverage ${point.coveredAccounts}/${point.knownAccounts}`,
    }));
  }
  if (metric === 'total' && data.official.primaryScope && data.official.points.length) {
    const coverage = Math.max(0, Math.min(data.official.coverageRatio, 1));
    return data.official.points.map((point, index) => ({
      date: point.date,
      official: point.tokens,
      local: localByDate.get(point.date) ?? 0,
      previous: data.official.comparisonPoints[index]?.tokens ?? 0,
      xRatio: (index / Math.max(data.official.points.length - 1, 1)) * coverage,
    }));
  }
  return data.points.map((point, index) => ({
    date: point.date,
    official: metricValue(point.confirmed, metric, point.confirmedEvents),
    local: 0,
    previous: data.comparisonPoints[index]
      ? metricValue(data.comparisonPoints[index].confirmed, metric, data.comparisonPoints[index].confirmedEvents)
      : 0,
    xRatio: (data.period.coverageOffset ?? 0) + (index / Math.max(data.points.length - 1, 1)) * (data.period.coverageRatio ?? 1),
  }));
}

function TrendChart({ data, metric }: { data: TimeseriesResponse; metric: MetricKey }) {
  const { t } = useI18n();
  const geometry = useChartGeometry();
  const { width, height, padding } = geometry;
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  const points = mergeChartPoints(data, metric);
  if (!points.length) return <EmptyState text={t('components.trend-and-timeline.once_collection_starts_a_trusted_daily_token')} />;

  const max = Math.max(
    ...points.flatMap((point) => [point.official, point.local, point.previous]),
    1,
  );
  const accountTotal = metric === 'total' && data.official.primaryScope ? data.official.displayTotalTokens : null;
  const officialTotal = accountTotal ?? points.reduce((sum, point) => sum + point.official, 0);
  const averageDivisor = points.length;
  const average = accountTotal !== null
    ? accountTotal / Math.max(averageDivisor, 1)
    : points.reduce((sum, point) => sum + point.official, 0) / points.length;
  const averageLabel = data.grain === 'hour' ? t('components.trend-and-timeline.hourly_avg') : data.grain === 'week' ? t('components.explorer.weekly_avg') : data.grain === 'month' ? t('components.explorer.monthly_avg') : t('components.explorer.daily_avg');
  const peakPoint = points.reduce((peak, point) => point.official > peak.official ? point : peak, points[0]);
  const activeIndex = hoverIndex ?? points.length - 1;
  const active = points[activeIndex];
  const activeX = pointXAtRatio(active.xRatio, geometry);
  const activeY = pointY(active.official, max, geometry);
  const averageY = pointY(average, max, geometry);
  const labelIndexes = [...new Set([0, Math.floor((points.length - 1) / 2), points.length - 1])];
  const innerHeight = height - padding.top - padding.bottom;

  return (
    <div className="trend-chart-wrap">
      <div className="trend-kpis">
        <div><span>{metric === 'total' && accountTotal !== null ? (data.official.displayIsLowerBound ? t('components.trend-and-timeline.live_period_lower_bound') : t('components.trend-and-timeline.official_period_total')) : metricLabel(metric)}</span><strong>{data.official.displayIsLowerBound && accountTotal !== null ? '≥ ' : ''}{compactNumber(officialTotal)}</strong></div>
        <div><span>{averageLabel}</span><strong>{compactNumber(average)}</strong></div>
        <div><span>{t('components.trend-and-timeline.peak')} · {shortDate(peakPoint.date)}</span><strong>{compactNumber(peakPoint.official)}</strong></div>
      </div>

      <div className="chart-canvas">
        {metric === 'total' && data.official.primaryScope && !data.official.reconciledPoints.length && data.official.coverageRatio < 1 && (
          <div className="chart-coverage-gap" style={{ left: `${Math.max(0, data.official.coverageRatio) * 100}%` }}><span>{t('components.trend-and-timeline.no_official_coverage')}</span></div>
        )}
        {(!data.official.primaryScope || metric !== 'total') && (data.period.coverageOffset ?? 0) > 0 && (
          <div className="chart-coverage-gap is-leading" style={{ left: 0, right: `${(1 - (data.period.coverageOffset ?? 0)) * 100}%` }}><span>{t('components.trend-and-timeline.no_local_coverage')}</span></div>
        )}
        <div className="chart-readout">
          <span>{shortDate(active.date)} · {active.statusLabel ?? (metric === 'total' && accountTotal !== null ? t('components.token-overview.official_booked') : metricLabel(metric))}</span>
          <strong>{compactNumber(active.official)}</strong>
          <small>{active.sourceDetail ?? `${t('components.explorer.previous')} ${compactNumber(active.previous)}${metric === 'total' && accountTotal !== null ? ` · ${t('components.trend-and-timeline.local_attribution')} ${compactNumber(active.local)}` : ''}`}</small>
        </div>
        <svg
          className="trend-chart"
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="xMidYMid meet"
          role="img"
          aria-label={`${metricLabel(metric)} ${t('components.trend-and-timeline.trend_chart')}`}
          onPointerLeave={() => setHoverIndex(null)}
          onPointerMove={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            const x = ((event.clientX - bounds.left) / bounds.width) * width;
            const ratio = (x - padding.left) / (width - padding.left - padding.right);
            setHoverIndex(Math.max(0, Math.min(points.length - 1, Math.round(ratio * (points.length - 1)))));
          }}
        >
          <defs>
            <linearGradient id="confirmedArea" x1="0" y1="0" x2="0" y2="1">
              <stop className="area-stop-primary" offset="0%" />
              <stop className="area-stop-secondary" offset="100%" />
            </linearGradient>
          </defs>
          {[0, 0.25, 0.5, 0.75, 1].map((ratio) => {
            const y = padding.top + innerHeight * ratio;
            const value = max * (1 - ratio);
            return (
              <g key={ratio}>
                <line x1={padding.left} x2={width - padding.right} y1={y} y2={y} className="chart-gridline" />
                <text x={padding.left - 10} y={y + 4} textAnchor="end" className="chart-axis-label">
                  {compactNumber(value)}
                </text>
              </g>
            );
          })}
          <line x1={padding.left} x2={width - padding.right} y1={averageY} y2={averageY} className="average-line" />
          <text x={width - padding.right} y={averageY - 6} textAnchor="end" className="average-label">{averageLabel}</text>
          <polygon
            points={`${padding.left},${height - padding.bottom} ${seriesPath(points, 'official', max, geometry)} ${pointXAtRatio(points.at(-1)?.xRatio ?? 1, geometry)},${height - padding.bottom}`}
            fill="url(#confirmedArea)"
          />
          <polyline points={seriesPath(points, 'official', max, geometry)} className="trend-line confirmed-line" />
          <polyline points={seriesPath(points, 'previous', max, geometry)} className="trend-line comparison-line" />
          <polyline points={seriesPath(points, 'local', max, geometry)} className="trend-line quarantined-line" />
          <line x1={activeX} x2={activeX} y1={padding.top} y2={height - padding.bottom} className="chart-crosshair" />
          <circle cx={activeX} cy={activeY} r="8" className="active-point-halo" />
          <circle cx={activeX} cy={activeY} r="4" className="active-point" />
          {labelIndexes.map((index) => {
            const x = pointXAtRatio(points[index].xRatio, geometry);
            return (
              <text key={index} x={x} y={height - 12} textAnchor={index === 0 ? 'start' : index === points.length - 1 ? 'end' : 'middle'} className="chart-date-label">
                {shortDate(points[index].date)}
              </text>
            );
          })}
        </svg>
      </div>
      <div className="chart-footer">
        <div className="chart-legend" aria-label={t('components.trend-and-timeline.trend_legend')}>
          <span><i className="legend-confirmed" />{metric === 'total' && data.official.reconciledPoints.length ? t('components.trend-and-timeline.daily_account_reconciliation') : metric === 'total' && accountTotal !== null ? t('components.token-overview.official_booked') : metricLabel(metric)}</span>
          <span><i className="legend-comparison" />{t('components.trend-and-timeline.previous_period')}</span>
          {metric === 'total' && accountTotal !== null && <span><i className="legend-quarantined" />{t('components.trend-and-timeline.local_attributable')}</span>}
        </div>
        <span className="hover-hint">{t('components.trend-and-timeline.move_the_pointer_to_inspect_each_bucket')}</span>
      </div>
      <details className="chart-data-table">
        <summary>{t('components.trend-and-timeline.view_trend_data_table')}</summary>
        <div>
          <table>
            <thead><tr><th>{t('components.explorer.time')}</th><th>{t('components.trend-and-timeline.current')}</th><th>{t('components.explorer.previous')}</th><th>{t('app.local_attribution')}</th><th>{t('components.trend-and-timeline.status')}</th></tr></thead>
            <tbody>{points.map((point) => <tr key={point.date}><td>{point.date}</td><td>{point.official.toLocaleString('en-US')}</td><td>{point.previous.toLocaleString('en-US')}</td><td>{point.local.toLocaleString('en-US')}</td><td>{point.statusLabel ?? t('components.trend-and-timeline.local_sample')}</td></tr>)}</tbody>
          </table>
        </div>
      </details>
    </div>
  );
}

function stackedArea(points: TimeseriesResponse['points'], lower: (point: TimeseriesResponse['points'][number]) => number, upper: (point: TimeseriesResponse['points'][number]) => number, max: number, geometry: ChartGeometry): string {
  const top = points.map((point, index) => `${pointX(index, points.length, geometry)},${pointY(upper(point), max, geometry)}`);
  const bottom = [...points].reverse().map((point, reverseIndex) => {
    const index = points.length - 1 - reverseIndex;
    return `${pointX(index, points.length, geometry)},${pointY(lower(point), max, geometry)}`;
  });
  return [...top, ...bottom].join(' ');
}

function CompositionChart({ data }: { data: TimeseriesResponse }) {
  const { t } = useI18n();
  const geometry = useChartGeometry();
  const { width, height, padding } = geometry;
  const points = data.points;
  if (!points.length) return <EmptyState text={t('components.trend-and-timeline.no_token_composition_can_be_drawn_within')} />;
  const max = Math.max(...points.map((point) => point.confirmed.total), 1);
  const input = points.reduce((sum, point) => sum + point.confirmed.input, 0);
  const cacheWriteObservedInput = points.reduce((sum, point) => sum + point.confirmed.cacheWriteObservedInput, 0);
  const cacheWriteCoverage = input ? cacheWriteObservedInput / input : 0;
  return (
    <div className="composition-chart-wrap">
      <svg className="trend-chart composition-area-chart" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={t('components.trend-and-timeline.input_cache_read_cache_write_and_output')}>
        {[0, 0.5, 1].map((ratio) => <line key={ratio} x1={padding.left} x2={width - padding.right} y1={padding.top + (height - padding.top - padding.bottom) * ratio} y2={padding.top + (height - padding.top - padding.bottom) * ratio} className="chart-gridline" />)}
        <polygon className="stack-area stack-uncached" points={stackedArea(points, () => 0, (point) => point.confirmed.uncached, max, geometry)} />
        <polygon className="stack-area stack-cached" points={stackedArea(points, (point) => point.confirmed.uncached, (point) => point.confirmed.uncached + point.confirmed.cached, max, geometry)} />
        <polygon className="stack-area stack-cache-write" points={stackedArea(points, (point) => point.confirmed.uncached + point.confirmed.cached, (point) => point.confirmed.input, max, geometry)} />
        <polygon className="stack-area stack-output" points={stackedArea(points, (point) => point.confirmed.input, (point) => point.confirmed.total, max, geometry)} />
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
  const max = Math.max(...visible.flatMap((series) => series.points.map((point) => metricValue(point.confirmed, metric, point.confirmedEvents))), 1);
  const toggle = (id: string) => setSelected((current) => current.includes(id) ? current.filter((value) => value !== id) : current.length < 5 ? [...current, id] : current);
  return (
    <div className="project-compare-wrap">
      <div className="project-series-picker">{ranked.slice(0, 10).map((series) => <button className={selected.includes(series.id) ? 'is-selected' : ''} key={series.id} onClick={() => toggle(series.id)} type="button">{dimensionLabel(series.id, series.label)}</button>)}</div>
      {visible.length ? <svg className="trend-chart project-compare-chart" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={t('components.trend-and-timeline.project_usage_comparison')}>
        {[0, 0.5, 1].map((ratio) => <line key={ratio} x1={padding.left} x2={width - padding.right} y1={padding.top + (height - padding.top - padding.bottom) * ratio} y2={padding.top + (height - padding.top - padding.bottom) * ratio} className="chart-gridline" />)}
        {visible.map((series, seriesIndex) => <polyline key={series.id} points={series.points.map((point, index) => `${pointX(index, series.points.length, geometry)},${pointY(metricValue(point.confirmed, metric, point.confirmedEvents), max, geometry)}`).join(' ')} className="trend-line project-compare-line" style={{ stroke: PROJECT_COLORS[seriesIndex] }} />)}
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
  useEffect(() => {
    if (!allowProjectCompare && view === 'projects') setView('total');
  }, [allowProjectCompare, view]);
  return (
    <Panel
      title={displayTitle}
      eyebrow={officialScope ? t('components.trend-and-timeline.official_account_trend') : t('app.local_attribution')}
      meta={<div className="trend-panel-meta"><span>{data.grain === 'hour' ? t('components.trend-and-timeline.hourly') : data.grain === 'day' ? t('components.trend-and-timeline.daily') : data.grain === 'week' ? t('components.trend-and-timeline.weekly') : t('components.trend-and-timeline.monthly')}</span><div className="trend-view-control"><button aria-pressed={view === 'total'} className={view === 'total' ? 'is-active' : ''} onClick={() => setView('total')} type="button">{t('components.trend-and-timeline.total')}</button><button aria-pressed={view === 'composition'} className={view === 'composition' ? 'is-active' : ''} onClick={() => setView('composition')} type="button">{t('components.trend-and-timeline.composition')}</button>{allowProjectCompare && <button aria-pressed={view === 'projects'} className={view === 'projects' ? 'is-active' : ''} onClick={() => setView('projects')} type="button">{t('components.trend-and-timeline.compare_projects')}</button>}</div></div>}
      className={`trend-panel ${className}`}
    >
      {view === 'total' && <TrendChart data={data} metric={metric} />}
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
