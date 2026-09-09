import { expect, it, vi } from 'vitest';
import { runScopedRequest } from './requestLifecycle';

it('ignores late success and settlement from an obsolete scope', async () => {
  let resolve!: (value: string) => void;
  const pending = new Promise<string>(done => { resolve = done; });
  const controller = new AbortController();
  const callbacks = { success: vi.fn(), failure: vi.fn(), settled: vi.fn() };
  const request = runScopedRequest(controller.signal, () => pending, callbacks);
  controller.abort();
  resolve('old account');
  await request;
  expect(callbacks.success).not.toHaveBeenCalled();
  expect(callbacks.failure).not.toHaveBeenCalled();
  expect(callbacks.settled).not.toHaveBeenCalled();
});

it('does not let an obsolete error replace current status', async () => {
  let reject!: (error: Error) => void;
  const pending = new Promise<string>((_, fail) => { reject = fail; });
  const controller = new AbortController();
  const callbacks = { success: vi.fn(), failure: vi.fn(), settled: vi.fn() };
  const request = runScopedRequest(controller.signal, () => pending, callbacks);
  controller.abort();
  reject(new Error('stale network error'));
  await request;
  expect(callbacks.failure).not.toHaveBeenCalled();
  expect(callbacks.settled).not.toHaveBeenCalled();
});

it('applies current responses and preserves the last snapshot on failure', async () => {
  const controller = new AbortController();
  let snapshot = 'previous';
  const callbacks = { success: (value: string) => { snapshot = value; }, failure: vi.fn(), settled: vi.fn() };
  await runScopedRequest(controller.signal, async () => 'current', callbacks);
  expect(snapshot).toBe('current');
  await runScopedRequest(controller.signal, async () => { throw new Error('offline'); }, callbacks);
  expect(snapshot).toBe('current');
  expect(callbacks.failure).toHaveBeenCalledOnce();
  expect(callbacks.settled).toHaveBeenCalledTimes(2);
});
