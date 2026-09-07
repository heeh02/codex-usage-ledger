import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { UsageBreakdownTable } from './UsageBreakdownTable';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';

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
