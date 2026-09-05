import type { TurnEvidenceQuery } from './turnEvidence';
import type { TurnEvidenceResponse, TurnEvidenceRow } from './turn-evidence.generated';
import { mockRequestEvidence } from './requestEvidenceMock';
import { normalizedEvidenceSelection } from './requestEvidence';

export function mockTurnEvidence(query: TurnEvidenceQuery): TurnEvidenceResponse {
  const requests = mockRequestEvidence({ ...query, limit: 500 }).rows;
  const groups = new Map<string, TurnEvidenceRow>();
  for (const request of requests) {
    const id = request.turnId === null ? `request:${request.id}` : `turn:${request.turnId}`;
    let group = groups.get(id);
    if (!group) {
      group = { groupId: id, turnId: request.turnId, firstAt: request.at, lastAt: request.at,
        requestCount: 0, confirmedRequestCount: 0, confirmedUsage: null };
      groups.set(id, group);
    }
    group.requestCount++;
    group.lastAt = request.at;
    if (request.quality === 'confirmed') {
      group.confirmedRequestCount++;
      if (!group.confirmedUsage) group.confirmedUsage = { ...request.usage };
      else {
        for (const field of ['input', 'cached', 'cacheWrite', 'cacheWriteObservedInput', 'uncached', 'output', 'reasoning', 'total'] as const) {
          group.confirmedUsage[field] += request.usage[field];
        }
        group.confirmedUsage.cacheWriteCoverage = group.confirmedUsage.input
          ? group.confirmedUsage.cacheWriteObservedInput / group.confirmedUsage.input : 0;
      }
    }
  }
  const all = [...groups.values()], offset = query.offset ?? 0, limit = query.limit ?? 100;
  return { scope: 'thread_own_retained_turns', threadId: query.threadId, start: query.start,
    end: query.end, selectedAccount: normalizedEvidenceSelection(query.account),
    selectedModel: normalizedEvidenceSelection(query.model), historyComplete: false, backfillComplete: true,
    rows: all.slice(offset, offset + limit), nextOffset: offset + limit < all.length ? offset + limit : null };
}
