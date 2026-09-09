import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { UsageBreakdownTable } from './UsageBreakdownTable';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import { LocalComposition } from './LocalComposition';

it('does not label unsplit input as uncached or absent writes as measured zero', () => {
  const usage = { input: 100, cached: 80, cacheWrite: 0, cacheWriteObservedInput: 0, cacheWriteCoverage: 0, uncached: 20, output: 20, reasoning: 5, total: 120 };
  for (const messages of [enMessages, zhCNMessages]) {
    const html = renderToStaticMarkup(createElement(I18nContext.Provider, { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } },
      createElement(UsageBreakdownTable, { identityLabel: 'Models', rows: [{ id: null, events: 1, usage }] })));
    expect(html).toContain(messages['components.explorer.input_unsplit']);
    expect(html).toContain(messages['sessions.unknown_dimension']);
    expect(html).toContain('<td>—</td>');
    expect(html).toContain('title="120"');
    expect(html).toContain('usage-breakdown-scroll');
  }
});

it('does not hide positive write amounts when coverage metadata is unknown', () => {
  const usage = { input: 10_000_000, cached: 4_000_000, cacheWrite: 1_000_000, cacheWriteObservedInput: 0,
    cacheWriteCoverage: 0, uncached: 5_000_000, output: 2_000_000, reasoning: 0, total: 12_000_000 };
  for (const messages of [enMessages, zhCNMessages]) {
    const render = (child: ReturnType<typeof createElement>) => renderToStaticMarkup(createElement(I18nContext.Provider,
      { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } }, child));
    const table = render(createElement(UsageBreakdownTable, { identityLabel: 'Models', rows: [{ id: 'model', events: 1, usage }] }));
    expect(table).toContain(`title="1,000,000">1 M (${messages['sessions.partial_split']})`);
    expect(table).toContain(messages['components.explorer.input_unsplit']);
    const composition = render(createElement(LocalComposition, { usage, eventCount: 1 }));
    expect(composition).toContain('title="1,000,000">1 M');
    const amounts = [...composition.matchAll(/<dd title="([0-9,]+)"/g)].map(match => Number(match[1].replaceAll(',', '')));
    expect(amounts.reduce((sum, value) => sum + value, 0)).toBe(usage.total);
  }
});

it('keeps absent confirmed usage distinct from recorded zero and shows partial write coverage', () => {
  const usage = { input: 100, cached: 80, cacheWrite: 5, cacheWriteObservedInput: 50, cacheWriteCoverage: .5, uncached: 15, output: 20, reasoning: 5, total: 120 };
  const html = renderToStaticMarkup(createElement(I18nContext.Provider, { value: { language: 'en', setLanguage: () => {}, t: key => enMessages[key] } },
    createElement(UsageBreakdownTable, { identityLabel: 'Models', rows: [
      { id: 'missing', available: false, events: 0, usage },
      { id: 'observed', events: 1, usage },
      { id: 'zero', events: 1, usage: { ...usage, input: 0, cached: 0, cacheWrite: 0, cacheWriteCoverage: 0, cacheWriteObservedInput: 0, uncached: 0, output: 0, reasoning: 0, total: 0 } },
    ] })));
  expect(html).toContain(enMessages['sessions.partial_split']);
  expect(html).toContain(enMessages['usage.no_confirmed_records']);
  expect(html).toContain('title="0">0 M');
  expect(html.indexOf('observed')).toBeLessThan(html.indexOf('missing'));
  const missing = html.slice(html.indexOf('missing'), html.indexOf('</tr>', html.indexOf('missing')));
  expect(missing).not.toContain('title="120"');
  expect(missing.match(/<td>—<\/td>/g)).toHaveLength(5);
});
