import type { MetricKey, PeriodWindow, TimeGrain, TimeseriesResponse } from '../api/types';
import { metricValue } from '../lib';

// Source bucket keys are local civil times, not timestamps in the viewer's
// browser timezone. Use a UTC ordinal solely for drawing that civil axis.
export function civilTime(key: string): number {
  return Date.parse(key.length === 10 ? `${key}T00:00:00Z` : `${key.replace(/Z$/, '')}Z`);
}

export function civilKey(timestamp: string, timezone: string): string {
  const date = new Date(timestamp);
  if (!Number.isFinite(date.getTime())) return timestamp;
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: timezone, year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23',
  }).formatToParts(date);
  const part = (type: string) => parts.find(p => p.type === type)?.value;
  return `${part('year')}-${part('month')}-${part('day')}T${part('hour')}:${part('minute')}:${part('second')}`;
}

export function nextBucket(key: string, grain: TimeGrain): number {
  const date = new Date(civilTime(key));
  if (grain === 'month') date.setUTCMonth(date.getUTCMonth() + 1);
  else if (grain === 'week') date.setUTCDate(date.getUTCDate() + 7);
  else if (grain === 'day') date.setUTCDate(date.getUTCDate() + 1);
  else date.setUTCHours(date.getUTCHours() + 1);
  return date.getTime();
}

export function timeDomain(keys: string[], grain: TimeGrain, period?: PeriodWindow): [number, number] {
  const times = keys.map(civilTime).filter(Number.isFinite);
  const floor = (value: number) => {
    const date = new Date(value);
    date.setUTCMinutes(0, 0, 0);
    if (grain !== 'hour') date.setUTCHours(0);
    if (grain === 'month') date.setUTCDate(1);
    if (grain === 'week') date.setUTCDate(date.getUTCDate() - (date.getUTCDay() + 6) % 7);
    return date.getTime();
  };
  if (period) {
    for (const timestamp of [period.start, period.end]) {
      const value = civilTime(civilKey(timestamp, period.timezone));
      if (Number.isFinite(value)) times.push(floor(value));
    }
  }
  if (!times.length) return [0, 1];
  const first = Math.min(...times), last = Math.max(...times);
  return [first, last === first ? nextBucket(new Date(first).toISOString().slice(0, 16), grain) : last];
}

export function timeRatio(key: string, domain: [number, number]): number {
  return (civilTime(key) - domain[0]) / Math.max(domain[1] - domain[0], 1);
}

export function contiguous<T extends { date: string }>(points: T[], grain: TimeGrain, valid: (point: T) => boolean): T[][] {
  const segments: T[][] = [];
  let current: T[] = [];
  for (const point of points) {
    if (!valid(point)) { current = []; continue; }
    if (!current.length || civilTime(point.date) > nextBucket(current.at(-1)!.date, grain)) {
      current = [];
      segments.push(current);
    }
    current.push(point);
  }
  return segments;
}

export function comparisonKey(key: string, period: PeriodWindow, grain: TimeGrain): string | null {
  if (!period.comparisonStart || period.comparisonAvailable === false) return null;
  const currentStart = civilTime(civilKey(period.start, period.timezone));
  const previousStart = civilTime(civilKey(period.comparisonStart, period.timezone));
  const date = new Date(civilTime(key));
  if (!Number.isFinite(date.getTime())) return null;
  if (period.key === 'month' || period.key === 'year' || grain === 'month') {
    const a = new Date(currentStart), b = new Date(previousStart);
    const monthOffset = (a.getUTCFullYear() - b.getUTCFullYear()) * 12 + a.getUTCMonth() - b.getUTCMonth();
    const day = date.getUTCDate();
    date.setUTCDate(1);
    date.setUTCMonth(date.getUTCMonth() + monthOffset);
    const expectedMonth = date.getUTCMonth();
    date.setUTCDate(day);
    if (date.getUTCMonth() !== expectedMonth) return null;
  } else {
    date.setTime(date.getTime() + currentStart - previousStart);
  }
  return date.toISOString().slice(0, grain === 'hour' ? 16 : 10);
}

export interface TrendPoint {
  date: string;
  value: number | null;
  previous: number | null;
  local: number | null;
}

export function bucketDateRange(date: string, grain: TimeGrain): { startDate: string; endDate: string } {
  const startDate = date.slice(0, 10);
  const endDate = grain === 'hour' ? startDate : new Date(nextBucket(date, grain) - 1).toISOString().slice(0, 10);
  return { startDate, endDate };
}

export function trendSeries(data: TimeseriesResponse, metric: MetricKey) {
  const localValue = (point: TimeseriesResponse['points'][number]) => point.confirmedEvents === 0
    && (point.unknownEvents > 0 || point.quarantinedEvents > 0)
    ? null : metricValue(point.confirmed, metric, point.confirmedEvents);
  const account = metric === 'total' && data.official.primaryScope
    && (data.official.reconciledPoints.length > 0 || data.official.points.length > 0);
  const grain = account ? data.official.granularity : data.grain;
  const rows = account
    ? data.official.reconciledPoints.length
      ? data.official.reconciledPoints.map(p => ({ date: p.date, value: p.status === 'unknown' ? null : p.value }))
      : data.official.points.map(p => ({ date: p.date, value: p.tokens }))
    : data.points.map(p => ({ date: p.date, value: localValue(p) }));
  const comparison = account
    ? data.official.reconciledComparisonPoints.length
      ? data.official.reconciledComparisonPoints.map(p => ({ date: p.date, value: p.status === 'unknown' ? null : p.value }))
      : data.official.comparisonPoints.map(p => ({ date: p.date, value: p.tokens }))
    : data.comparisonPoints.map(p => ({ date: p.date, value: metricValue(p.confirmed, metric, p.confirmedEvents) }));
  const previous = new Map(comparison.flatMap(p => {
    const key = comparisonKey(p.date, data.period, grain);
    return key ? [[key, p.value] as const] : [];
  }));
  const local = new Map(data.points.map(p => [p.date, localValue(p)]));
  const points: TrendPoint[] = rows.filter(p => Number.isFinite(civilTime(p.date)))
    .sort((a, b) => civilTime(a.date) - civilTime(b.date))
    .map(p => ({ ...p, previous: previous.get(p.date) ?? null,
      local: account && grain === data.grain ? local.get(p.date) ?? null : null }));
  return { points, grain, account, domain: timeDomain(points.map(p => p.date), grain, data.period) };
}
