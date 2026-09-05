import { expect, it } from 'vitest';
import { mockTurnEvidence } from './turnEvidenceMock';
import { mockRequestEvidence } from './requestEvidenceMock';

it('conserves the complete request fixture across turn pages', () => {
  const query = { threadId: 'demo', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' };
  const requests = mockRequestEvidence({ ...query, limit: 500 }).rows;
  const first = mockTurnEvidence({ ...query, limit: 50 });
  expect(first.nextOffset).toBe(50);
  const second = mockTurnEvidence({ ...query, limit: 50, offset: 50 });
  expect(second.nextOffset).toBeNull();
  const rows = [...first.rows, ...second.rows];
  expect(new Set(rows.map(row => row.groupId)).size).toBe(rows.length);
  expect(rows.reduce((sum, row) => sum + row.requestCount, 0)).toBe(requests.length);
  expect(rows.reduce((sum, row) => sum + (row.confirmedUsage?.total ?? 0), 0)).toBe(24600);
  expect(rows.filter(row => row.turnId === null).every(row => row.requestCount === 1)).toBe(true);
});
