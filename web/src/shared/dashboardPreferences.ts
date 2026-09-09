import type { DashboardFilters } from '../api/types';
import { boundedText, integerBetween, oneOf, readSessionObject } from './sessionPreferences';

const periods = ['today', 'week', 'month', 'rolling7', 'rolling30', 'weeks12', 'months12', 'year', 'lifetime', 'custom'] as const satisfies readonly DashboardFilters['period'][];
const identifier = (value: unknown) => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const validDate = (value?: string) => !!value && /^\d{4}-\d{2}-\d{2}$/.test(value)
  && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 10) === value;

export function restoreDashboardFilters(fallback: DashboardFilters): DashboardFilters {
  const filters = readSessionObject('ledger.filters', fallback, {
    account: identifier, project: identifier, model: identifier, session: identifier,
    period: oneOf(...periods),
    metric: oneOf('total', 'input', 'cached', 'cacheWrite', 'uncached', 'output', 'reasoning', 'requests'),
    grain: oneOf('auto', 'hour', 'day', 'week', 'month'),
    startDate: boundedText, endDate: boundedText,
    nodeSearch: boundedText, sessionSearch: boundedText,
    nodeOffset: integerBetween(0, Number.MAX_SAFE_INTEGER), sessionOffset: integerBetween(0, Number.MAX_SAFE_INTEGER),
    nodeLimit: integerBetween(1, 1000), sessionLimit: integerBetween(1, 100),
    sessionSort: oneOf('tokens', 'output', 'requests', 'recent'),
  });
  if (filters.period === 'custom' && (!validDate(filters.startDate) || !validDate(filters.endDate) || filters.startDate! > filters.endDate!)) {
    filters.period = fallback.period;
    delete filters.startDate;
    delete filters.endDate;
  }
  return filters;
}
