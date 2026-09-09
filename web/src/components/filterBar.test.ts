import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { FilterBar } from './Ui';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import type { FilterCatalog } from '../api/types';

it('keeps visible refresh text stable while exposing busy state in both languages', () => {
  const catalog: FilterCatalog = { accounts: [], projects: [], models: [], periods: [] };
  for (const messages of [enMessages, zhCNMessages]) {
    const render = (refreshing: boolean) => renderToStaticMarkup(createElement(I18nContext.Provider,
      { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } },
      createElement(FilterBar, { catalog, value: { account: 'all', project: 'all', model: 'all', session: 'all', period: 'month', metric: 'total', grain: 'auto' },
        page: 'overview', refreshing, onChange: () => {}, onRefresh: () => {} }),
    ));
    const idle = render(false);
    const busy = render(true);
    for (const html of [idle, busy]) expect(html).toContain(`<span>${messages['components.ui.refresh']}</span>`);
    expect(idle).toContain('aria-busy="false"');
    expect(busy).toContain('aria-busy="true"');
    expect(busy).toContain(messages['components.ui.refreshing_local_usage']);
    expect(busy).toContain('disabled=""');
  }
});
