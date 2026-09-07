import { expect, it } from 'vitest';
import { accountScopeLabel } from './AccountSwitcher';

it('separates observed-login prefixes from the selected reporting label', () => {
  const options = [{ id: 'a', label: '当前账号 · Account A' }, { id: 'b', label: '历史账号 · Account B' }];
  expect(accountScopeLabel('a', options, [], 'All accounts')).toBe('Account A');
  expect(accountScopeLabel('b', options, [], 'All accounts')).toBe('Account B');
  expect(accountScopeLabel('all', options, [], '全部账号')).toBe('全部账号');
  expect(accountScopeLabel('missing', options, [], 'All accounts')).toBe('missing');
});
