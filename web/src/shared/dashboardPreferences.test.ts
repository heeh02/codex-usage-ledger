import { afterEach, expect, it, vi } from 'vitest';
import type { DashboardFilters } from '../api/types';
import { restoreDashboardFilters } from './dashboardPreferences';

const defaults: DashboardFilters = { account: 'all', project: 'all', model: 'all', session: 'all', period: 'rolling30', metric: 'total', grain: 'auto' };
const load = (stored: unknown) => {
  vi.stubGlobal('window', { sessionStorage: { getItem: () => JSON.stringify(stored) } });
  return restoreDashboardFilters(defaults);
};
afterEach(() => vi.unstubAllGlobals());

it.each<DashboardFilters['period']>(['today', 'week', 'rolling7', 'month', 'rolling30', 'weeks12', 'months12', 'year', 'lifetime'])('preserves supported period %s', period => {
  expect(load({ period }).period).toBe(period);
});
it('keeps valid custom dates and drops corrupt window settings', () => {
  expect(load({ period: 'custom', startDate: '2024-02-29', endDate: '2024-03-01' }).period).toBe('custom');
  for (const dates of [{}, { startDate: '2026-02-30', endDate: '2026-03-01' }, { startDate: '2026-03-01', endDate: '2026-02-28' }]) {
    const restored = load({ period: 'custom', ...dates });
    expect(restored.period).toBe('rolling30');
    expect(restored.startDate).toBeUndefined();
    expect(restored.endDate).toBeUndefined();
  }
});
it('rejects incompatible fields while retaining valid navigation preferences', () => {
  const result = load({ project: 'synthetic-project', account: '', session: null, model: [], period: 'bad', sessionOffset: -1, nodeLimit: 5000, grain: 'year', metric: 42, nodeSearch: 'x'.repeat(257), usage: { total: 999 } });
  expect(result).toEqual({ ...defaults, project: 'synthetic-project' });
  const longIdentifier = 'synthetic-'.repeat(40);
  expect(load({ project: longIdentifier }).project).toBe(longIdentifier);
});
