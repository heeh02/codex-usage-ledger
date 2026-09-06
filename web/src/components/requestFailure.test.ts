import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { ErrorState } from './Ui';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';

it('offers a localized range recovery rather than implying missing history is temporary', () => {
  for (const messages of [zhCNMessages, enMessages]) {
    const html = renderToStaticMarkup(createElement(I18nContext.Provider,
      { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } },
      createElement(ErrorState, { message: messages['app.insufficient_time_precision'],
        title: messages['app.time_precision_unavailable_title'], actionLabel: messages['app.show_today_usage'], onRetry: () => {} }),
    ));
    expect(html).toContain('role="alert"');
    expect(html).toContain(messages['app.time_precision_unavailable_title']);
    expect(html).not.toContain(messages['components.ui.dashboard_data_is_temporarily_unavailable']);
    expect(html).not.toContain('HTTP 422');
  }
});
