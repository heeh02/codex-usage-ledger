import { LedgerRequestError } from '../api/errors';
import type { Translate } from '../i18n';

export function requestFailureMessage(reason: unknown, t: Translate): string {
  if (reason instanceof LedgerRequestError) {
    if (reason.code === 'insufficient_time_precision') return t('app.insufficient_time_precision');
    if (reason.code === 'invalid_query') return t('app.invalid_usage_query');
    return t('app.usage_request_http_failed', { status: reason.status });
  }
  return t('app.usage_request_connection_failed');
}
