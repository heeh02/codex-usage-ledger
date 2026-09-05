import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import type { DashboardBundle, DashboardFilters } from './api/types';
import { MockLedgerApi } from './api/mock';
import { csvCell, exportUsageCsv, exportUsageJson, usageExportRows, USAGE_CSV_COLUMNS } from './export';

const filters: DashboardFilters = { account: 'all', project: 'all', model: 'all', session: 'all', period: 'week', metric: 'total', grain: 'auto' };
let bundle: DashboardBundle;
beforeAll(async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  bundle = await new MockLedgerApi().getBundle(filters);
});
afterAll(() => vi.unstubAllGlobals());

describe('scoped usage export', () => {
  it('exports versioned JSON rows without unrelated catalogs or raw bundle data', () => {
    const sample = structuredClone(bundle);
    sample.explorer.projects[0].label = 'DO-NOT-EXPORT-CATALOG-TITLE';
    const serialized = exportUsageJson(sample, { ...filters, project: 'proj-atlas' });
    const exported = JSON.parse(serialized);
    expect(exported.version).toBe(1);
    expect(exported.bundle).toBeUndefined();
    expect(serialized).not.toContain('DO-NOT-EXPORT-CATALOG-TITLE');
    expect(exported.rows.length).toBeGreaterThan(0);
    expect(exported.rows.every((row: { source: string }) => row.source === 'local')).toBe(true);
    expect(Object.keys(exported.rows[0])).toEqual([...USAGE_CSV_COLUMNS]);
  });

  it('keeps unavailable official components null in JSON instead of zero', () => {
    const sample = structuredClone(bundle);
    sample.summary.official.points = [{ date: '2026-01-01', tokens: 123 }];
    const exported = JSON.parse(exportUsageJson(sample, filters));
    const official = exported.rows.find((row: { source: string }) => row.source === 'official');
    expect(official.total).toBe(123);
    expect(official.input_total).toBeNull();
    expect(official.cache_write_observed).toBeNull();
    expect(official.output).toBeNull();
  });
  it('keeps local activity when no official dates are available', () => {
    const sample = structuredClone(bundle);
    sample.summary.official.points = [];
    const rows = usageExportRows(sample, filters);
    expect(rows.length).toBe(sample.timeseries.points.length);
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.reduce((sum, row) => sum + Number(row[3]), 0)).toBe(sample.timeseries.points.reduce((sum, point) => sum + point.confirmed.total, 0));
    expect(rows.every(row => row.length === USAGE_CSV_COLUMNS.length)).toBe(true);
  });

  it('retains official-only dates without inventing composition', () => {
    const sample = structuredClone(bundle);
    sample.summary.official.points = [{ date: '2026-01-01', tokens: 123 }];
    const official = usageExportRows(sample, filters).find(row => row[1] === 'official')!;
    expect(official[0]).toBe('2026-01-01');
    expect(official[3]).toBe(123);
    expect(official.slice(4, 12)).toEqual(Array(8).fill(''));
    expect(official.length).toBe(USAGE_CSV_COLUMNS.length);
  });

  it('does not export account totals under a project filter', () => {
    expect(usageExportRows(bundle, { ...filters, project: 'proj-atlas' }).every(row => row[1] === 'local')).toBe(true);
  });

  it('distinguishes unknown cache writes from observed zero', () => {
    const sample = structuredClone(bundle);
    sample.timeseries.points[0].confirmed.cacheWriteCoverage = 0;
    const rows = usageExportRows(sample, filters);
    const local = rows.find(row => row[1] === 'local' && row[0] === sample.timeseries.points[0].date)!;
    expect(local[6]).toBe('');
    expect(local.at(-1)).toBe('local_cache_write_partial');
    expect(exportUsageCsv(sample, filters)).toContain('cache_write_observed');
  });

  it('exports the selected conversation own timeline rather than project totals', async () => {
    const selected = { ...filters, session: 'session-atlas-audit' };
    const sample = await new MockLedgerApi().getBundle(selected);
    const detail = sample.explorer.selectedSession!;
    expect(detail).toBeTruthy();
    const rows = usageExportRows(sample, selected, 'own');
    expect(rows.length).toBe(detail.ownSamplingTimeline.length);
    expect(rows.reduce((sum, row) => sum + Number(row[3]), 0)).toBe(detail.ownSamplingTimeline.reduce((sum, point) => sum + point.usage.total, 0));
    expect(rows.every(row => row[1] === 'local')).toBe(true);
    expect(rows.every(row => row[16] === 'own')).toBe(true);
    const json = JSON.parse(exportUsageJson(sample, selected, 'own'));
    expect(json.rows.reduce((sum: number, row: { total: number }) => sum + row.total, 0))
      .toBe(detail.ownUsage.total);
    expect(usageExportRows(sample, { ...selected, session: 'different-session' })).toEqual([]);
    expect(JSON.parse(exportUsageJson(sample, { ...selected, session: 'different-session' })).rows).toEqual([]);
  });

  it('escapes strings and spreadsheet expressions', () => {
    expect(csvCell('a,"b"')).toBe('"a,""b"""');
    expect(csvCell('=1+1')).toBe('"\'=1+1"');
    expect(csvCell(123)).toBe('123');
  });
});
