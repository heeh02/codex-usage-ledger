import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import type { QuotaCycle } from '../api/types';
import { QuotaPanel } from './QuotaPanel';

const cycle: QuotaCycle = {
  id: 'interval-a', accountId: 'account-a', accountLabel: 'Account A', limitId: 'pool-a', label: 'Pool A', role: 'primary', windowKind: 'weekly', windowMinutes: 10080,
  cycleStart: '2026-01-01T00:00:00Z', cycleEnd: '2026-01-08T00:00:00Z',
  firstObservedAt: '2026-01-03T00:00:00Z', lastObservedAt: '2026-01-04T00:00:00Z', firstUsedPercent: 20, usedPercent: 30, usedDeltaPercent: 10, sampleCount: 2,
  localObservationStart: '2026-01-03T00:00:00Z', localObservationEnd: '2026-01-04T00:00:00Z', localCoverageRatio: null,
  localUsage: { total: 0, input: 0, cached: 0, uncached: 0, cacheWrite: 0, cacheWriteObservedInput: 0, cacheWriteCoverage: 0, output: 0, reasoning: 0 }, localEvents: 0,
  empiricalTokensPerUsedPercent: null, empiricalRatioIsConversion: false, boundaryKind: 'observed_decrease', boundaryAfter: '2026-01-02T00:00:00Z', historyLimited: true,
};

it('labels bounded observations and distinguishes missing usage from observed zero in both languages', () => {
  for (const messages of [enMessages, zhCNMessages]) {
    const render = (localEvents: number) => renderToStaticMarkup(createElement(I18nContext.Provider,
      { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } },
      createElement(QuotaPanel, { pools: [], cycles: [{ ...cycle, localEvents }] })));
    const missing = render(0);
    expect(missing).toContain(messages['quota.history_preview_scope']);
    expect(missing).toContain(messages['quota.history_limited']);
    expect(missing).toContain(messages['quota.boundary_decrease']);
    expect(missing).toContain(messages['quota.boundary_observations']);
    expect(missing).not.toContain('0 M');
    expect(render(1)).toContain('0 M');
  }
});
