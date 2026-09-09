import { expect, it } from 'vitest';
import { mockRequestEvidence } from '../../api/requestEvidenceMock';
import { requestTokenDisplay } from './requestValue';

it('keeps unknown placeholders distinct from observed zero', () => {
  const row = mockRequestEvidence({ threadId: 'demo', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' }).rows[0];
  expect(requestTokenDisplay({ ...row, quality: 'unknown' }, 'output')).toBe('—');
  expect(requestTokenDisplay({ ...row, quality: 'unknown' }, 'total')).toBe('—');
  expect(requestTokenDisplay(row, 'total')).toBe('<0.001 M');
  expect(requestTokenDisplay(row, 'total', 'exact')).toBe('120');
  expect(requestTokenDisplay(row, 'reasoning')).toBe('<0.001 M');
  expect(requestTokenDisplay(row, 'reasoning', 'exact')).toBe('5');
  expect(requestTokenDisplay({ ...row, quality: 'unknown' }, 'total', 'exact')).toBe('—');
  expect(requestTokenDisplay({ ...row, usage: { ...row.usage, output: 0 } }, 'output')).toBe('0 M');
  expect(requestTokenDisplay({ ...row, usage: { ...row.usage, cacheWrite: 0, cacheWriteCoverage: 0 } }, 'cacheWrite')).toBe('—');
  expect(requestTokenDisplay({ ...row, usage: { ...row.usage, cacheWrite: 10, cacheWriteCoverage: 0 } }, 'cacheWrite', 'exact')).toBe('10');
});
