import { expect, it } from 'vitest';
import { accountScopeLabel, accountPlanLabel } from './AccountSwitcher';

it('uses observed tier names without guessing a Pro multiplier', () => {
  expect(accountPlanLabel('pro')).toBe('Pro');
  expect(accountPlanLabel('pro_20x')).toBe('Pro 20×');
  expect(accountPlanLabel('pro_5x')).toBe('Pro 5×');
  expect(accountPlanLabel('plus')).toBe('Plus');
  expect(accountPlanLabel('free')).toBe('Free');
  expect(accountPlanLabel(null)).toBe('—');
});

it('separates observed-login prefixes from the selected reporting label', () => {
  const options = [{ id: 'a', label: '当前账号 · Account A' }, { id: 'b', label: '历史账号 · Account B' }];
  expect(accountScopeLabel('a', options, [], 'All accounts')).toBe('Account A');
  expect(accountScopeLabel('b', options, [], 'All accounts')).toBe('Account B');
  expect(accountScopeLabel('all', options, [], '全部账号')).toBe('全部账号');
  expect(accountScopeLabel('missing', options, [], 'All accounts')).toBe('missing');
  expect(accountScopeLabel('0123456789abcdef'.repeat(4), [], [], 'All accounts')).toBe('01234567…cdef');
});
