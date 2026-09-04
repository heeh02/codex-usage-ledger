import type { DataQuality, MetricKey, PeriodKey, PeriodWindow, TokenUsage } from './api/types';

export type UiLanguage = 'zh-CN' | 'en';
let uiLanguage: UiLanguage = 'zh-CN';

export function setUiLanguage(language: UiLanguage): void {
  uiLanguage = language;
}

export function currentUiLanguage(): UiLanguage {
  return uiLanguage;
}

function locale(): string {
  return uiLanguage === 'zh-CN' ? 'zh-CN' : 'en-US';
}

export function compactNumber(value: number): string {
  return new Intl.NumberFormat(locale(), { notation: 'compact', maximumFractionDigits: 1 }).format(value);
}

export function exactNumber(value: number): string {
  return new Intl.NumberFormat(locale()).format(Math.round(value));
}

export function formatPercent(value: number): string {
  return new Intl.NumberFormat(locale(), { style: 'percent', maximumFractionDigits: 1 }).format(value);
}

export function formatDateTime(value: string | null): string {
  if (!value) return uiLanguage === 'zh-CN' ? '暂无可信观测' : 'No trusted observation';
  return new Intl.DateTimeFormat(locale(), {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

export function formatPeriodRange(period: PeriodWindow): string {
  const start = new Date(period.start);
  const end = new Date(period.end);
  if (!Number.isFinite(start.getTime()) || !Number.isFinite(end.getTime())) return uiLanguage === 'zh-CN' ? '时间范围未知' : 'Unknown time range';
  const formatter = new Intl.DateTimeFormat(locale(), {
    timeZone: period.timezone,
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  });
  return `${formatter.format(start)}—${formatter.format(end)}`;
}

export function shortDate(value: string): string {
  const isDayBucket = /^\d{4}-\d{2}-\d{2}$/.test(value);
  const parsed = new Date(isDayBucket ? `${value}T12:00:00Z` : value);
  if (!Number.isFinite(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(locale(), isDayBucket
    ? { month: 'numeric', day: 'numeric' }
    : { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(parsed);
}

export function relativeReset(value: string | null): string {
  if (!value) return uiLanguage === 'zh-CN' ? '重置时间未知' : 'Reset time unknown';
  const delta = new Date(value).getTime() - Date.now();
  if (delta <= 0) return uiLanguage === 'zh-CN' ? '等待下一次额度观测' : 'Waiting for the next quota observation';
  const hours = Math.ceil(delta / 3_600_000);
  if (hours < 24) return uiLanguage === 'zh-CN' ? `约 ${hours} 小时后重置` : `Resets in about ${hours}h`;
  const days = Math.ceil(hours / 24);
  return uiLanguage === 'zh-CN' ? `约 ${days} 天后重置` : `Resets in about ${days}d`;
}

export function qualityLabel(value: DataQuality): string {
  if (value === 'confirmed') return uiLanguage === 'zh-CN' ? '已直接确认' : 'Confirmed';
  if (value === 'quarantined') return uiLanguage === 'zh-CN' ? '已隔离' : 'Quarantined';
  return uiLanguage === 'zh-CN' ? '暂时未知' : 'Unknown';
}

export function tokenComposition(usage: TokenUsage): Array<{ key: keyof TokenUsage; label: string; value: number }> {
  return [
    { key: 'uncached', label: uiLanguage === 'zh-CN' ? '输入（非缓存）' : 'Input (uncached)', value: usage.uncached },
    { key: 'cached', label: uiLanguage === 'zh-CN' ? '缓存读取' : 'Cache read', value: usage.cached },
    { key: 'cacheWrite', label: uiLanguage === 'zh-CN' ? '缓存写入' : 'Cache write', value: usage.cacheWrite },
    { key: 'output', label: uiLanguage === 'zh-CN' ? '输出' : 'Output', value: usage.output },
  ];
}

export function metricValue(usage: TokenUsage, metric: MetricKey, events = 0): number {
  if (metric === 'requests') return events;
  return usage[metric];
}

export function metricLabel(metric: MetricKey): string {
  const chinese = {
    total: 'Token 总量',
    input: '输入（含缓存）',
    cached: '缓存读取',
    cacheWrite: '缓存写入',
    uncached: '输入（非缓存）',
    output: '输出',
    reasoning: 'Reasoning',
    requests: 'Sampling 请求',
  }[metric];
  const english = {
    total: 'Total tokens',
    input: 'Input (incl. cache)',
    cached: 'Cache read',
    cacheWrite: 'Cache write',
    uncached: 'Input (uncached)',
    output: 'Output',
    reasoning: 'Reasoning',
    requests: 'Sampling requests',
  }[metric];
  return uiLanguage === 'zh-CN' ? chinese : english;
}

export function periodLabel(period: PeriodKey): string {
  const labels: Record<PeriodKey, [string, string]> = {
    today: ['今日', 'Today'],
    week: ['本周', 'This week'],
    rolling7: ['近7天', 'Last 7 days'],
    month: ['本月', 'This month'],
    rolling30: ['近30天', 'Last 30 days'],
    weeks12: ['12周', '12 weeks'],
    months12: ['12月', '12 months'],
    lifetime: ['至今', 'Lifetime'],
  };
  return labels[period][uiLanguage === 'zh-CN' ? 0 : 1];
}

export function dimensionLabel(id: string, fallback: string): string {
  if (id === '__standalone_conversations__') return uiLanguage === 'zh-CN' ? '独立对话' : 'Standalone chats';
  if (id === 'unassigned') return uiLanguage === 'zh-CN' ? '未匹配记录' : 'Unmatched records';
  return fallback;
}
