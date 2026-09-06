import { expect, it } from 'vitest';
import { LedgerRequestError } from '../api/errors';
import type { Translate } from '../i18n';
import { enMessages } from '../locales/en';
import { zhCNMessages } from '../locales/zh-CN';
import { requestFailureMessage } from './requestFailureMessage';
import { runScopedRequest } from './requestLifecycle';

it('renders the same failure in either locale without storing translated text', () => {
  const error = new LedgerRequestError(422, 'insufficient_time_precision');
  for (const messages of [zhCNMessages, enMessages]) {
    const t: Translate = (key, parameters) => messages[key].replace('{status}', String(parameters?.status ?? ''));
    expect(requestFailureMessage(error, t)).toBe(messages['app.insufficient_time_precision']);
    expect(requestFailureMessage(new LedgerRequestError(400, 'invalid_query'), t)).toBe(messages['app.invalid_usage_query']);
    expect(requestFailureMessage(new LedgerRequestError(500, 'request_failed'), t)).toContain('500');
    expect(requestFailureMessage(new Error('private path'), t)).toBe(messages['app.usage_request_connection_failed']);
  }
});

it('keeps both accepted filters and values when a precision request fails', async () => {
  const accepted = { filters: { period: 'week', account: 'synthetic-a' }, tokens: 120 };
  let snapshot = accepted;
  let failure: unknown;
  await runScopedRequest(new AbortController().signal, async () => { throw new LedgerRequestError(422, 'insufficient_time_precision'); }, {
    success: value => { snapshot = value; }, failure: reason => { failure = reason; }, settled: () => {},
  });
  expect(snapshot).toBe(accepted);
  expect(failure).toBeInstanceOf(LedgerRequestError);
});
