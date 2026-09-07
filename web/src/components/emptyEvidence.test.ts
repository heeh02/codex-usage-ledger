import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, expect, it, vi } from 'vitest';
import { LocalUsagePulse } from './Explorer';
import { LocalComposition } from './LocalComposition';
import { DataStatusStrip } from './Ui';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import { MockLedgerApi } from '../api/mock';
import { formatPercent } from '../lib';

afterEach(() => vi.unstubAllGlobals());
it('distinguishes absent evidence from recorded zero tokens in both locales', async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  const api = new MockLedgerApi();
  const filters = { account: 'all', project: 'missing-fixture', model: 'all', session: 'all',
    period: 'lifetime' as const, metric: 'total' as const, grain: 'auto' as const };
  const summary = await api.getSummary(filters);
  const explorer = await api.getExplorer(filters);
  expect(summary.confirmedEvents).toBe(0);
  expect(summary.matchRate).toBeNull();
  expect(summary.cacheRate).toBeNull();
  expect(summary.metrics.localAttributedTotal.value).toBeNull();
  expect(formatPercent(null)).toBe('—');
  expect(formatPercent(0)).not.toBe('—');
  for (const messages of [enMessages, zhCNMessages]) {
    const render = (child: ReturnType<typeof createElement>) => renderToStaticMarkup(createElement(
      I18nContext.Provider, { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } }, child,
    ));
    const amounts = (html: string) => Array.from(html.matchAll(/<strong>(.*?)<\/strong>/g), match => match[1]);
    const empty = render(createElement(LocalUsagePulse, { explorer, summary, metric: 'total' }));
    expect(amounts(empty)).toEqual(['—', '—', '—', '0', '—']);
    expect(empty).toContain(messages['usage.no_confirmed_records']);
    const strip = render(createElement(DataStatusStrip, { summary, page: 'overview' }));
    expect(strip).not.toContain('is-good');
    expect(strip).not.toContain('100%');
    const noComposition = render(createElement(LocalComposition, { usage: summary.usage.confirmed, eventCount: 0 }));
    expect(noComposition).toContain(messages['usage.no_confirmed_records']);
    expect(noComposition).not.toContain('<dl');
    const zero = render(createElement(LocalUsagePulse, {
      summary: { ...summary, confirmedEvents: 1 },
      explorer: { ...explorer, stats: { ...explorer.stats, localRecent15Events: 1 } }, metric: 'total',
    }));
    expect(amounts(zero)).toEqual(['0 M', '0 M', '0 M', '1', '0 M']);
  }
});
