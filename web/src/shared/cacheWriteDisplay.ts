import type { TokenUsage } from '../api/types';

/** A positive recorded amount must survive uncertain coverage metadata.
 * Zero without field coverage is unavailable; positive coverage permits an
 * observed zero. This says nothing about completeness of the entire period.
 */
export function hasCacheWriteAmount(usage: Pick<TokenUsage, 'cacheWrite' | 'cacheWriteCoverage'>): boolean {
  return usage.cacheWrite > 0 || usage.cacheWriteCoverage > 0;
}
