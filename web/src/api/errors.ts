export type LedgerErrorCode = 'insufficient_time_precision' | 'invalid_query' | 'request_failed' | 'snapshot_unavailable';

export class LedgerRequestError extends Error {
  constructor(readonly status: number, readonly code: LedgerErrorCode) {
    super(`Ledger request returned HTTP ${status}`);
    this.name = 'LedgerRequestError';
  }
}

/** Never display raw response bodies, SQL errors, paths or HTML in the UI. */
export async function ledgerResponseError(response: Pick<Response, 'status' | 'json'>): Promise<LedgerRequestError> {
  const body: unknown = await response.json().catch(() => null);
  const code = body && typeof body === 'object' && !Array.isArray(body) && 'code' in body ? body.code : null;
  const known: LedgerErrorCode = code === 'snapshot_unavailable' && response.status === 503 ? code : code === 'insufficient_time_precision' && response.status === 422
    ? code : code === 'invalid_query' && response.status === 400 ? code : 'request_failed';
  return new LedgerRequestError(response.status, known);
}
