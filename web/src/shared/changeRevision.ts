/** An invalid event must not advance the observed revision or interrupt the UI. */
export function parseChangeRevision(data: string): string | null {
  try {
    const payload: unknown = JSON.parse(data);
    if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return null;
    const revision = (payload as { revision?: unknown }).revision;
    if (typeof revision === 'string') return revision.trim() ? revision : null;
    if (typeof revision === 'number' && Number.isSafeInteger(revision) && revision >= 0) return String(revision);
    return null;
  } catch {
    return null;
  }
}
