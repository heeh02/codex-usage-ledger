import { expect, it } from 'vitest';
import { parentSessionFilters, type SessionTrailEntry } from './sessionTrail';
import type { DashboardFilters } from '../api/types';

it('restores parent navigation without undoing current account/model/time filters', () => {
  const current: DashboardFilters = { account: 'account-b', project: 'project', model: 'model-b', period: 'month', metric: 'total', grain: 'day', session: 'child', nodeOffset: 0 };
  const parent: SessionTrailEntry<{ scope: 'own' }> = { id: 'parent', title: 'Parent', view: { scope: 'own' }, nodes: { nodeOffset: 50, nodeLimit: 25, nodeSearch: 'worker' }, scrollTop: 320 };
  expect(parentSessionFilters(current, parent)).toEqual({ ...current, session: 'parent', nodeOffset: 50, nodeLimit: 25, nodeSearch: 'worker' });
  expect(parent.view.scope).toBe('own');
});
