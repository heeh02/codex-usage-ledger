import { describe, expect, it } from 'vitest';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ModelUsageLabel } from './ModelUsageLabel';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';

const render = (props: Parameters<typeof ModelUsageLabel>[0]) => renderToStaticMarkup(createElement(I18nContext.Provider,
  { value: { language: 'en', setLanguage: () => {}, t: (key, parameters) => enMessages[key].replace(/\{(\w+)\}/g, (_, name: string) => String(parameters?.[name] ?? '')) } },
  createElement(ModelUsageLabel, props)));

describe('model usage labels', () => {
  it('uses scoped models rather than catalog metadata when available', () => {
    const html = render({ models: ['model-a', 'model-b'], catalogModel: 'catalog-only' });
    expect(html).toContain('2');
    expect(html).toContain('model-a · model-b');
    expect(html).not.toContain('catalog-only');
  });
  it('distinguishes absent evidence from an empty scoped result', () => {
    expect(render({ catalogModel: 'catalog-only' })).toContain('catalog-only');
    expect(render({ models: [], catalogModel: 'catalog-only' })).not.toContain('catalog-only');
  });
});
