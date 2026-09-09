import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { CollectionProgress } from './Ui';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import type { CollectionStatus } from '../api/types';

const status: CollectionStatus = {
  mode: 'daemon', phase: 'degraded', itemsTotal: 100, itemsCompleted: 100,
  bytesRead: 1024, eventsInserted: 4, message: 'private source error must not render',
  updatedAt: '2026-01-01T00:00:00Z', rollupItemsTotal: 100,
  rollupItemsCompleted: 100, rollupComplete: true, rawRetentionDays: 14,
};

it('explains retryable failures in both languages without false completion or raw error text', () => {
  for (const messages of [enMessages, zhCNMessages]) {
    const render = (phase: CollectionStatus['phase']) => renderToStaticMarkup(createElement(
      I18nContext.Provider, { value: { language: 'en', setLanguage: () => {}, t: key => messages[key] } },
      createElement(CollectionProgress, { status: { ...status, phase } }),
    ));
    const html = render('degraded');
    expect(html).toContain(messages['collection.degraded']);
    expect(html).toContain(messages['collection.retry_detail']);
    expect(html).toContain('role="status"');
    expect(html).not.toContain(status.message);
    expect(html).not.toContain('100%');
    expect(html).not.toContain('collection-progress-meter');
    expect(render('live')).toBe('');
  }
});

it('distinguishes identity review from a failure that automatic retry can resolve', () => {
  for (const messages of [enMessages, zhCNMessages]) {
    const html=renderToStaticMarkup(createElement(I18nContext.Provider,
      {value:{language:'en',setLanguage:()=>{},t:key=>messages[key]}},
      createElement(CollectionProgress,{status:{...status,message:'sampling,reconstruction_identity_review'}})));
    expect(html).toContain(messages['collection.identity_review']);
    expect(html).not.toContain(messages['collection.retry_detail']);
    expect(html).not.toContain('reconstruction_identity_review');
  }
});
