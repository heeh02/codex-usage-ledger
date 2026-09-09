import { expect, it } from 'vitest';
import { mockRequestEvidence } from './requestEvidenceMock';

it('provides three conserved pages with explicit and unknown turns', () => {
  const query = { threadId: 'demo-thread', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' };
  const first = mockRequestEvidence(query);
  const second = mockRequestEvidence({ ...query, after: first.next });
  const third = mockRequestEvidence({ ...query, after: second.next });
  expect([first.rows.length, second.rows.length, third.rows.length]).toEqual([100, 100, 5]);
  const rows = [...first.rows, ...second.rows, ...third.rows];
  expect(new Set(rows.map(row => row.id)).size).toBe(205);
  expect(rows.reduce((sum, row) => sum + row.usage.total, 0)).toBe(24600);
  expect(first.rows[0].turnId).toBeNull();
  expect(first.rows[1].turnId).toBe(first.rows[2].turnId);
  expect(third.next).toBeNull();
});
