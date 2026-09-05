import { afterEach, expect, it, vi } from 'vitest';
import { MockLedgerApi } from './mock';

afterEach(() => vi.unstubAllGlobals());
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
