import { MockLedgerApi } from './mock';
import type {
  BreakdownsResponse,
  DashboardBundle,
  DashboardFilters,
  DataMode,
  ExplorerResponse,
  LedgerApi,
  QualityResponse,
  SummaryResponse,
  TimeseriesResponse,
} from './types';

function queryString(filters: DashboardFilters): string {
  const params = new URLSearchParams({
    account: filters.account,
    project: filters.project,
    model: filters.model,
    period: filters.period,
    metric: filters.metric,
  });
  if (filters.grain !== 'auto') params.set('grain', filters.grain);
  if (filters.session !== 'all') params.set('session', filters.session);
  return params.toString();
}

class HttpLedgerApi implements LedgerApi {
  readonly mode = 'http' as const;

  constructor(private readonly baseUrl: string) {}

  private async request<T>(path: string, filters: DashboardFilters, signal?: AbortSignal): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}?${queryString(filters)}`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
      signal,
    });

    if (!response.ok) {
      const detail = await response.text().catch(() => '');
      throw new Error(`${path} returned HTTP ${response.status}${detail ? `: ${detail.slice(0, 160)}` : ''}`);
    }

    return (await response.json()) as T;
  }

  getSummary(filters: DashboardFilters, signal?: AbortSignal): Promise<SummaryResponse> {
    return this.request('/v1/summary', filters, signal);
  }

  getTimeseries(filters: DashboardFilters, signal?: AbortSignal): Promise<TimeseriesResponse> {
    return this.request('/v1/timeseries', filters, signal);
  }

  getBreakdowns(filters: DashboardFilters, signal?: AbortSignal): Promise<BreakdownsResponse> {
    return this.request('/v1/breakdowns', filters, signal);
  }

  getQuality(filters: DashboardFilters, signal?: AbortSignal): Promise<QualityResponse> {
    return this.request('/v1/quality', filters, signal);
  }

  getExplorer(filters: DashboardFilters, signal?: AbortSignal): Promise<ExplorerResponse> {
    return this.request('/v1/explorer', filters, signal);
  }

  getBundle(filters: DashboardFilters, signal?: AbortSignal): Promise<DashboardBundle> {
    return this.request('/v1/bundle', filters, signal);
  }

  async setUserConfirmedAccountCount(count: number | null): Promise<void> {
    const response = await fetch(`${this.baseUrl}/v1/account-registry`, {
      method: 'POST',
      headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify({ userConfirmedAccountCount: count }),
    });
    if (!response.ok) {
      const detail = await response.text().catch(() => '');
      throw new Error(`account registry returned HTTP ${response.status}${detail ? `: ${detail.slice(0, 160)}` : ''}`);
    }
  }

  async refreshOfficial(): Promise<void> {
    const response = await fetch(`${this.baseUrl}/v1/official/refresh`, {
      method: 'POST',
      headers: { Accept: 'application/json' },
    });
    if (!response.ok) throw new Error(`official refresh returned HTTP ${response.status}`);
  }

  async refreshOfficialThread(sessionId: string): Promise<void> {
    const response = await fetch(`${this.baseUrl}/v1/official/thread/refresh?session=${encodeURIComponent(sessionId)}`, {
      method: 'POST',
      headers: { Accept: 'application/json' },
    });
    if (!response.ok) throw new Error(`official thread refresh returned HTTP ${response.status}`);
  }
}

export function configuredDataMode(): DataMode {
  return import.meta.env.VITE_LEDGER_DATA_MODE === 'mock' ? 'mock' : 'http';
}

export function createLedgerApi(): LedgerApi {
  if (configuredDataMode() === 'http') {
    const baseUrl = (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/, '');
    return new HttpLedgerApi(baseUrl);
  }
  return new MockLedgerApi();
}

export async function loadDashboardBundle(
  api: LedgerApi,
  filters: DashboardFilters,
  signal?: AbortSignal,
): Promise<DashboardBundle> {
  return api.getBundle(filters, signal);
}
