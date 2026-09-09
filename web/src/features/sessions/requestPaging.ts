import type { RequestEvidenceCursor } from '../../api/request-evidence.generated';

export interface RequestPaging {
  cursor: RequestEvidenceCursor | null;
  previous: Array<RequestEvidenceCursor | null>;
}
export const firstRequestPage = (): RequestPaging => ({ cursor: null, previous: [] });
export function advanceRequestPage(state: RequestPaging, next: RequestEvidenceCursor | null): RequestPaging {
  if (!next) return state;
  return { cursor: next, previous: [...state.previous, state.cursor] };
}
export function previousRequestPage(state: RequestPaging): RequestPaging {
  if (!state.previous.length) return state;
  return { cursor: state.previous.at(-1) ?? null, previous: state.previous.slice(0, -1) };
}
