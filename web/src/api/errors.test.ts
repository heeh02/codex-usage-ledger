import { afterEach, expect, it, vi } from 'vitest';
import { LedgerRequestError, ledgerResponseError } from './errors';
import { createLedgerApi } from './client';

afterEach(() => { vi.unstubAllGlobals(); vi.unstubAllEnvs(); });

it('propagates typed failures and cancellation through the real HTTP client', async () => {
  vi.stubEnv('VITE_LEDGER_DATA_MODE', 'http');
  const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({ code: 'insufficient_time_precision', error: 'private path' }), { status: 422 }));
  vi.stubGlobal('fetch', fetch);
  const controller = new AbortController();
  await expect(createLedgerApi().getBundle({ account:'all', project:'all', model:'all', session:'all', period:'month', metric:'total', grain:'auto' }, controller.signal))
    .rejects.toMatchObject({ name: 'LedgerRequestError', code:'insufficient_time_precision', status:422 });
  expect(fetch.mock.calls[0][1].signal).toBe(controller.signal);
});

it('recognizes precision errors by stable code and status, not prose', async () => {
  const error = await ledgerResponseError({ status: 422, json: async () => ({ code: 'insufficient_time_precision', error: 'private/source/path' }) });
  expect(error).toBeInstanceOf(LedgerRequestError);
  expect(error.code).toBe('insufficient_time_precision');
  expect(error.message).not.toContain('private');
  expect((await ledgerResponseError({ status: 500, json: async () => ({ code: 'insufficient_time_precision' }) })).code).toBe('request_failed');
});

it('handles legacy, malformed and unknown error bodies without exposing them', async () => {
  for (const body of [null, [], 'HTML', { error: 'private detail' }, { code: 'future_code' }]) {
    expect((await ledgerResponseError({ status: 500, json: async () => body })).code).toBe('request_failed');
  }
  expect((await ledgerResponseError({ status: 503, json: async () => { throw new Error('invalid JSON'); } })).status).toBe(503);
  expect((await ledgerResponseError({ status: 400, json: async () => ({ code: 'invalid_query' }) })).code).toBe('invalid_query');
});
