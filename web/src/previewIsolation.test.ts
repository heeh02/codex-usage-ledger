import { afterEach, expect, it, vi } from 'vitest';
import config from '../vite.config';

afterEach(() => vi.unstubAllEnvs());
const environment = { command: 'serve' as const, mode: 'test', isSsrBuild: false, isPreview: false };

it('does not forward synthetic preview API requests to the live ledger', async () => {
  vi.stubEnv('VITE_LEDGER_DATA_MODE', 'mock');
  const resolved = await config(environment);
  expect(resolved.server?.proxy).toBeUndefined();
  expect(resolved.server?.strictPort).toBe(true);
  expect(resolved.preview?.proxy).toEqual({});
});

it('retains loopback-only proxy for explicit HTTP development', async () => {
  vi.stubEnv('VITE_LEDGER_DATA_MODE', 'http');
  const resolved = await config(environment);
  expect(resolved.server?.host).toBe('127.0.0.1');
  expect(resolved.server?.proxy?.['/v1']).toBe('http://127.0.0.1:47127');
});
