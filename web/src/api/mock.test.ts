import { afterEach, expect, it, vi } from 'vitest';
import { MockLedgerApi } from './mock';

afterEach(() => vi.unstubAllGlobals());
it('conserves project, root, node and timeline token dimensions in demo fixtures', async () => {
  vi.stubGlobal('window', { setTimeout, clearTimeout });
  const api = new MockLedgerApi();
  const filters = { account: 'all', project: 'all', model: 'all', session: 'all',
    period: 'lifetime' as const, grain: 'auto' as const, metric: 'total' as const };
  const catalog = await api.getExplorer(filters);
  const fields = ['input', 'cached', 'cacheWrite', 'uncached', 'output', 'reasoning',
    'total', 'cacheWriteObservedInput'] as const;
  for (const project of catalog.projects) {
    const result = await api.getExplorer({ ...filters, project: project.id });
    for (const field of fields) {
      expect(result.sessions.reduce((sum, row) => sum + row.treeUsage[field], 0))
        .toBe(project.periodUsage[field]);
    }
    for (const row of result.sessions) {
      const detail = (await api.getExplorer({ ...filters, project: project.id, session: row.id })).selectedSession!;
      for (const field of fields) {
        expect(detail.nodes.reduce((sum, node) => sum + node.ownUsage[field], 0)).toBe(detail.treeUsage[field]);
        expect(detail.samplingTimeline.reduce((sum, point) => sum + point.usage[field], 0)).toBe(detail.treeUsage[field]);
        expect(detail.ownSamplingTimeline.reduce((sum, point) => sum + point.usage[field], 0)).toBe(detail.ownUsage[field]);
      }
      expect(detail.samplingTimeline.reduce((sum, point) => sum + point.events, 0)).toBe(detail.treeEventCount);
      for (const node of detail.nodes) {
        for (const field of fields) {
          expect(node.ownUsage[field] + detail.nodes.filter(child => child.parentId === node.id)
            .reduce((sum, child) => sum + child.subtreeUsage[field], 0)).toBe(node.subtreeUsage[field]);
        }
      }
    }
  }
});
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
  expect(detail.ownSamplingTimeline.reduce((sum, point) => sum + point.usage.total, 0)).toBe(detail.ownUsage.total);
  expect(detail.samplingTimeline.length).toBeGreaterThan(1);
  expect(detail.nodes[0].subtreeUsage.total).toBe(detail.treeUsage.total);
});
