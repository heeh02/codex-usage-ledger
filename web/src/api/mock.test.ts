import { afterEach, expect, it, vi } from 'vitest';
import { MockLedgerApi } from './mock';

afterEach(() => vi.unstubAllGlobals());
it('keeps totals and counts stable across node pages and search', async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  const api = new MockLedgerApi();
  const filters = { account: 'all', project: 'proj-atlas', model: 'all', session: 'session-atlas-audit', period: 'lifetime' as const, grain: 'auto' as const, metric: 'total' as const, nodeLimit: 1 };
  const first = (await api.getExplorer(filters)).selectedSession!;
  const second = (await api.getExplorer({ ...filters, nodeOffset: 1 })).selectedSession!;
  expect(first.nodes).toHaveLength(1);
  expect(second.nodes).toHaveLength(1);
  expect(second.nodes[0].id).not.toBe(first.nodes[0].id);
  expect(second.treeUsage.total).toBe(first.treeUsage.total);
  expect(second.treeEventCount).toBe(first.treeEventCount);
  expect(second.ownEventCount).toBe(first.ownEventCount);
  const found = (await api.getExplorer({ ...filters, nodeSearch: 'receipt' })).selectedSession!;
  expect(found.nodePage?.total).toBe(1);
  expect(found.nodes[0].id).toBe('agent-receipt');
  expect(found.treeUsage.total).toBe(first.treeUsage.total);
});
it('loads a child with its own conserved descendant timeline', async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  const result = await new MockLedgerApi().getExplorer({ account: 'all', project: 'proj-atlas', model: 'all', session: 'agent-runtime', period: 'lifetime', grain: 'auto', metric: 'total' });
  const detail = result.selectedSession!;
  expect(detail.id).toBe('agent-runtime');
  expect(detail.nodes.map(node => node.id)).toEqual(['agent-runtime', 'agent-receipt']);
  expect(detail.nodes.reduce((sum, node) => sum + node.ownUsage.total, 0)).toBe(detail.treeUsage.total);
  expect(detail.samplingTimeline.reduce((sum, point) => sum + point.usage.total, 0)).toBe(detail.treeUsage.total);
  expect(detail.ownSamplingTimeline[0].usage.total).toBe(detail.ownUsage.total);
  expect(detail.nodes[0].subtreeUsage.total).toBe(detail.treeUsage.total);
});
