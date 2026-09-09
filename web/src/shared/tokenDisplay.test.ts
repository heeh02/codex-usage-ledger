import { afterEach, describe, expect, it } from 'vitest';
import { compactNumber, formatMetricAmount, formatSignedTokenMillions, formatTokenMillions, metricAxisGutter, setUiLanguage } from '../lib';

afterEach(() => setUiLanguage('zh-CN'));

describe('million-token display', () => {
  it('reserves space for all M axis labels without shrinking their type', () => {
    for (const max of [1, 1_000_000, 1_000_000_000_000, Number.MAX_SAFE_INTEGER]) {
      for (const ratio of [0, 0.25, 0.5, 0.75, 1]) {
        expect(metricAxisGutter(max, 'total')).toBeGreaterThanOrEqual(formatMetricAmount(max * ratio, 'total').length * 8 + 12);
      }
    }
  });
  it('keeps signed diagnostic differences separate from nonnegative usage', () => {
    expect(formatSignedTokenMillions(-12_000_000)).toBe('−12 M');
    expect(formatSignedTokenMillions(-1)).toBe('>−0.001 M');
    expect(formatSignedTokenMillions(0)).toBe('0 M');
    expect(formatSignedTokenMillions(null)).toBe('—');
    expect(formatTokenMillions(-12_000_000)).toBe('—');
  });
  it.each(['zh-CN', 'en'] as const)('keeps M units in %s without changing non-token counts', language => {
    setUiLanguage(language);
    expect(formatTokenMillions(12_345_678)).toBe('12.35 M');
    expect(formatTokenMillions(1_000_000_000)).toBe('1,000 M');
    expect(formatTokenMillions(12_345)).toBe('0.012 M');
    expect(formatTokenMillions(0)).toBe('0 M');
    expect(formatMetricAmount(17, 'requests')).toBe(compactNumber(17));
    expect(formatMetricAmount(1_000_000, 'cached')).toBe('1 M');
  });

  it('never rounds positive observations to measured zero or null to zero', () => {
    for (const value of [1, 50, 999]) expect(formatTokenMillions(value)).toBe('<0.001 M');
    for (const value of [null, undefined, NaN, Infinity, -Infinity, -1]) {
      expect(formatTokenMillions(value)).toBe('—');
    }
    expect(formatMetricAmount(null, 'requests')).toBe('—');
    expect(formatTokenMillions(1_000)).toBe('0.001 M');
  });
});
