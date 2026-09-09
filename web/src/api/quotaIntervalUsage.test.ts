import { expect, it } from 'vitest';
import { mockQuotaHistory } from './quotaHistoryMock';
import { mockQuotaIntervalUsage, validateQuotaIntervalUsage } from './quotaIntervalUsage';

it('requires fixed interval/account scope and conserved model/project components', () => {
  const page = mockQuotaHistory({ account: 'all' }); const row = page.intervals[0]; const selection = page.selections![0];
  const result = mockQuotaIntervalUsage(row, selection);
  expect(result.usage?.total).toBe(12_000_000);
  for (const invalid of [
    { ...result, accountId: 'other' }, { ...result, start: null }, { ...result, poolAttribution: true },
    { ...result, events: 3 }, { ...result, models: [] },
    { ...result, projects: [{ ...result.projects[0], usage: { ...result.projects[0].usage, cached: 0 } }] },
    { ...result, status: 'source_overlap_review' },
  ]) expect(() => validateQuotaIntervalUsage(invalid, row, selection)).toThrow();
  const blocked = { ...result, status: 'source_overlap_review', usage: null, events: null, models: [], projects: [] };
  expect(validateQuotaIntervalUsage(blocked, row, selection).usage).toBeNull();
});
