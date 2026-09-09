import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { MockLedgerApi } from '../api/mock';
import type { TimeseriesResponse } from '../api/types';
import { bucketDateRange, comparisonKey, contiguous, timeDomain, timeRatio, trendSeries } from './time';

let data: TimeseriesResponse;
beforeAll(async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  data = await new MockLedgerApi().getTimeseries({ account: 'all', project: 'all', model: 'all', session: 'all', period: 'month', metric: 'total', grain: 'auto' });
});
afterAll(() => vi.unstubAllGlobals());

describe('calendar chart projection', () => {
  it('opens the complete selected calendar bucket', () => {
    expect(bucketDateRange('2024-02-01', 'month')).toEqual({ startDate: '2024-02-01', endDate: '2024-02-29' });
    expect(bucketDateRange('2026-08-31', 'week')).toEqual({ startDate: '2026-08-31', endDate: '2026-09-06' });
    expect(bucketDateRange('2026-09-05T12:00', 'hour')).toEqual({ startDate: '2026-09-05', endDate: '2026-09-05' });
  });
  it('places sparse series on the same actual date axis', () => {
    const domain = timeDomain(['2026-01-01', '2026-01-02', '2026-01-11'], 'day');
    expect(timeRatio('2026-01-02', domain)).toBeCloseTo(0.1);
    expect(timeRatio('2026-01-11', domain)).toBe(1);
    expect(contiguous([{ date: '2026-01-01' }, { date: '2026-01-02' }, { date: '2026-01-11' }], 'day', () => true).map(s => s.length)).toEqual([2, 1]);
  });

  it('does not shift a sparse prior month by row index or overflow February', () => {
    const period = { ...data.period, key: 'month' as const, start: '2026-01-31T16:00:00Z', comparisonStart: '2025-12-31T16:00:00Z', comparisonAvailable: true, timezone: 'Asia/Shanghai' };
    expect(comparisonKey('2026-01-03', period, 'day')).toBe('2026-02-03');
    expect(comparisonKey('2026-01-31', period, 'day')).toBeNull();
  });

  it('leaves room for missing leading dates in the requested interval', () => {
    const period = { ...data.period, start: '2026-01-01T00:00:00Z', end: '2026-01-11T12:00:00Z', timezone: 'UTC' };
    const domain = timeDomain(['2026-01-06', '2026-01-11'], 'day', period);
    expect(timeRatio('2026-01-06', domain)).toBe(0.5);
  });

  it('uses the official day grain rather than calling a day total hourly', () => {
    const sample = structuredClone(data);
    sample.grain = 'hour';
    sample.official.primaryScope = true;
    sample.official.granularity = 'day';
    sample.official.reconciledPoints = [];
    sample.official.points = [{ date: '2026-01-01', tokens: 100 }];
    const chart = trendSeries(sample, 'total');
    expect(chart.grain).toBe('day');
    expect(chart.points[0].local).toBeNull();
  });

  it('does not call project data official or turn absent comparisons into zero', () => {
    const sample = structuredClone(data);
    sample.official.primaryScope = false;
    sample.comparisonPoints = [];
    const chart = trendSeries(sample, 'total');
    expect(chart.account).toBe(false);
    expect(chart.points.every(p => p.previous === null)).toBe(true);
    expect(chart.points.map(p => p.value)).toEqual(sample.points.map(p => p.confirmed.total));
  });

  it('does not turn an unknown-only bucket into confirmed zero', () => {
    const sample = structuredClone(data);
    sample.official.primaryScope = false;
    sample.points = [sample.points[0]];
    sample.points[0].confirmedEvents = 0;
    sample.points[0].unknownEvents = 1;
    expect(trendSeries(sample, 'total').points[0].value).toBeNull();
  });
});
