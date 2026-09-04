import type {
  BreakdownsResponse,
  DashboardBundle,
  DataMode,
  ExplorerResponse,
  QualityResponse,
  SummaryResponse,
  TimeGrain,
  TimeseriesResponse,
} from './wire.generated';

export * from './wire.generated';

export type MetricKey = 'total' | 'input' | 'cached' | 'cacheWrite' | 'uncached' | 'output' | 'reasoning' | 'requests';
export type GrainKey = 'auto' | TimeGrain;
export type BreakdownDimension = 'account' | 'project' | 'model';

export interface DashboardFilters {
  account: string;
  project: string;
  model: string;
  period: import('./wire.generated').PeriodKey;
  session: string;
  metric: MetricKey;
  grain: GrainKey;
}

export interface LedgerApi {
  mode: DataMode;
  getSummary(filters: DashboardFilters, signal?: AbortSignal): Promise<SummaryResponse>;
  getTimeseries(filters: DashboardFilters, signal?: AbortSignal): Promise<TimeseriesResponse>;
  getBreakdowns(filters: DashboardFilters, signal?: AbortSignal): Promise<BreakdownsResponse>;
  getQuality(filters: DashboardFilters, signal?: AbortSignal): Promise<QualityResponse>;
  getExplorer(filters: DashboardFilters, signal?: AbortSignal): Promise<ExplorerResponse>;
  getBundle(filters: DashboardFilters, signal?: AbortSignal): Promise<DashboardBundle>;
  setUserConfirmedAccountCount(count: number | null): Promise<void>;
  refreshOfficial(): Promise<void>;
  refreshOfficialThread(sessionId: string): Promise<void>;
}
