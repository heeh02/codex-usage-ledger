import { describe, expect, it } from 'vitest';
import { enMessages } from './en';
import { zhCNMessages } from './zh-CN';

describe('locale catalogs', () => {
  it('have exact key parity', () => {
    expect(Object.keys(enMessages).sort()).toEqual(Object.keys(zhCNMessages).sort());
  });

  it('do not contain empty translations', () => {
    for (const catalog of [zhCNMessages, enMessages]) {
      expect(Object.values(catalog).every((value) => value.trim().length > 0)).toBe(true);
    }
  });
});
