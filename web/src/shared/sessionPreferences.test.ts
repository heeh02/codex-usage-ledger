import { expect, it, vi } from 'vitest';
import { integerBetween, oneOf, readSessionObject, writeSessionObjects } from './sessionPreferences';

const defaults = { account: 'all', period: 'week', scope: 'tree', selected: null as string | null, page: 0 };
const validators = { period: oneOf('week', 'month'), scope: oneOf('own', 'tree'), page: integerBetween(0, 100) };

it('keeps valid preferences but rejects bad types/enums and unknown fields', () => {
  const storage = () => ({ getItem: () => '{"account":42,"period":"month","scope":"bad","selected":{},"page":-1,"usage":900,"__proto__":{"polluted":true}}', setItem: vi.fn() });
  const restored = readSessionObject('settings', defaults, validators, storage);
  expect(restored).toEqual({ ...defaults, period: 'month' });
  expect(Object.keys(restored)).toEqual(Object.keys(defaults));
  expect(defaults.period).toBe('week');
});

it.each(['null', '[]', 'true', '"text"', '{'])('ignores invalid persisted object %s', encoded => {
  expect(readSessionObject('settings', defaults, validators, () => ({ getItem: () => encoded, setItem: vi.fn() }))).toEqual(defaults);
});

it('handles both a denied Storage getter and getItem/setItem errors', () => {
  const denied = () => { throw new Error('storage blocked'); };
  expect(readSessionObject('settings', defaults, validators, denied)).toEqual(defaults);
  expect(() => writeSessionObjects({ settings: defaults }, denied)).not.toThrow();
  const unavailable = () => ({ getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('quota'); } });
  expect(readSessionObject('settings', defaults, validators, unavailable)).toEqual(defaults);
  expect(() => writeSessionObjects({ settings: defaults }, unavailable)).not.toThrow();
});

it('restores explicitly allowed optional numeric fields', () => {
  const fallback: { page: number; limit?: number } = { page: 0 };
  expect(readSessionObject('settings', fallback, { limit: integerBetween(1, 100) }, () => ({ getItem: () => '{"limit":100}', setItem: vi.fn() }))).toEqual({ page: 0, limit: 100 });
});
