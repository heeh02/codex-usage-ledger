import { describe, expect, it } from 'vitest';
import type { TokenUsage } from './api/types';
import { metricValue, tokenComposition } from './lib';

const usage: TokenUsage = {
  input: 1_000,
  cached: 700,
  cacheWrite: 100,
  cacheWriteObservedInput: 1_000,
  cacheWriteCoverage: 1,
  uncached: 200,
  output: 80,
  reasoning: 30,
  total: 1_080,
};

describe('token display contract', () => {
  it('keeps the four displayed buckets mutually exclusive', () => {
    const displayed = tokenComposition(usage).reduce((sum, bucket) => sum + bucket.value, 0);
    expect(displayed).toBe(usage.total);
  });

  it('treats reasoning as output detail rather than another total bucket', () => {
    expect(metricValue(usage, 'reasoning')).toBe(30);
    expect(metricValue(usage, 'total')).toBe(1_080);
    expect(usage.input + usage.output).toBe(usage.total);
  });

  it('keeps request count separate from token metrics', () => {
    expect(metricValue(usage, 'requests', 17)).toBe(17);
  });
});
