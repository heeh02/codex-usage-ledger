import { expect, it } from 'vitest';
import { hasCacheWriteAmount } from './cacheWriteDisplay';

it('distinguishes unknown zero, observed zero, and positive amounts with uncertain coverage', () => {
  expect(hasCacheWriteAmount({ cacheWrite: 0, cacheWriteCoverage: 0 })).toBe(false);
  expect(hasCacheWriteAmount({ cacheWrite: 0, cacheWriteCoverage: .5 })).toBe(true);
  expect(hasCacheWriteAmount({ cacheWrite: 0, cacheWriteCoverage: 1 })).toBe(true);
  expect(hasCacheWriteAmount({ cacheWrite: 10, cacheWriteCoverage: 0 })).toBe(true);
});
