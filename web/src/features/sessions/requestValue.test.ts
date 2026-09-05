import { expect, it } from 'vitest';
import { mockRequestEvidence } from '../../api/requestEvidenceMock';
import { requestTokenDisplay } from './requestValue';

it('keeps unknown placeholders distinct from observed zero', () => {
  const row = mockRequestEvidence({ threadId: 'demo', start: '2026-08-01T00:00:00Z', end: '2026-08-02T00:00:00Z' }).rows[0];
  expect(requestTokenDisplay({ ...row, quality: 'unknown' }, 'output')).toBe('—');
  expect(requestTokenDisplay({ ...row, usage: { ...row.usage, output: 0 } }, 'output')).toBe('0');
  expect(requestTokenDisplay({ ...row, usage: { ...row.usage, cacheWriteCoverage: 0 } }, 'cacheWrite')).toBe('—');
});
