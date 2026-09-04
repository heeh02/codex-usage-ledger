import type {
  BreakdownDimension,
  BreakdownRow,
  BreakdownsResponse,
  DashboardFilters,
  DashboardBundle,
  DataQuality,
  AttributionCoverage,
  ExplorerResponse,
  FilterCatalog,
  LedgerApi,
  MissingAccountEstimate,
  OfficialUsageView,
  PeriodKey,
  PeriodWindow,
  QualityIssue,
  QualityResponse,
  QualityStateSummary,
  QualityUsage,
  QuotaPool,
  SourceHealth,
  SummaryResponse,
  TimelineEvent,
  TimeseriesPoint,
  TimeseriesResponse,
  TokenUsage,
} from './types';

const DAY_MS = 86_400_000;
const ALL = 'all';
const DEMO_DAYS = 60;

const ACCOUNTS = [
  { id: 'acct-personal', label: 'Personal · Pro', description: 'Verified local profile' },
  { id: 'acct-research', label: 'Research workspace', description: 'Verified team workspace' },
  { id: 'acct-unknown', label: 'Unknown historical', description: 'No reliable historical mapping' },
];

const PROJECTS = [
  { id: 'proj-atlas', label: 'Project Atlas', description: 'Developer tools' },
  { id: 'proj-beacon', label: 'Project Beacon', description: 'Local services' },
  { id: 'proj-orbit', label: 'Project Orbit', description: 'Data visualization' },
];

const MODELS = [
  { id: 'gpt-5.6-sol', label: 'gpt-5.6-sol', description: 'Frontier agentic coding' },
  { id: 'gpt-5.6-terra', label: 'gpt-5.6-terra', description: 'Balanced agentic coding' },
  { id: 'gpt-5.6-luna', label: 'gpt-5.6-luna', description: 'Fast agentic coding' },
];

const PERIODS = [
  { id: 'today' as const, label: '今日' },
  { id: 'week' as const, label: '本周' },
  { id: 'rolling7' as const, label: '近7天' },
  { id: 'month' as const, label: '本月' },
  { id: 'rolling30' as const, label: '近30天' },
  { id: 'weeks12' as const, label: '12周' },
  { id: 'months12' as const, label: '12月' },
  { id: 'lifetime' as const, label: 'Lifetime' },
];

const FILTER_CATALOG: FilterCatalog = {
  accounts: [{ id: ALL, label: '全部账号 · 已捕获 2/2' }, ...ACCOUNTS],
  projects: [{ id: ALL, label: '全部项目' }, ...PROJECTS],
  models: [{ id: ALL, label: '全部模型' }, ...MODELS],
  periods: PERIODS,
};

const MOCK_SESSIONS = [
  {
    id: 'session-atlas-audit',
    projectId: 'proj-atlas',
    title: 'Audit parser boundaries and fixtures',
    model: 'gpt-5.6-sol',
    agents: [
      { id: 'agent-dataset', parentId: 'session-atlas-audit', title: 'fixture audit', model: 'gpt-5.6-terra', nickname: 'Turing', depth: 1 },
      { id: 'agent-runtime', parentId: 'session-atlas-audit', title: 'runtime review', model: 'gpt-5.6-sol', nickname: 'Curie', depth: 1 },
      { id: 'agent-receipt', parentId: 'agent-runtime', title: 'receipt crosscheck', model: 'gpt-5.6-luna', nickname: 'Bohr', depth: 2 },
    ],
  },
  {
    id: 'session-atlas-export',
    projectId: 'proj-atlas',
    title: 'Prepare release evidence',
    model: 'gpt-5.6-terra',
    agents: [
      { id: 'agent-package', parentId: 'session-atlas-export', title: 'package verification', model: 'gpt-5.6-luna', nickname: 'Noether', depth: 1 },
    ],
  },
  {
    id: 'session-beacon-release',
    projectId: 'proj-beacon',
    title: 'Review desktop lifecycle',
    model: 'gpt-5.6-sol',
    agents: [
      { id: 'agent-lifecycle', parentId: 'session-beacon-release', title: 'lifecycle audit', model: 'gpt-5.6-terra', nickname: 'Lovelace', depth: 1 },
      { id: 'agent-package-beacon', parentId: 'session-beacon-release', title: 'package evidence', model: 'gpt-5.6-luna', nickname: 'Gauss', depth: 1 },
    ],
  },
  {
    id: 'session-beacon-api',
    projectId: 'proj-beacon',
    title: 'Diagnose local API fallback',
    model: 'gpt-5.6-terra',
    agents: [],
  },
  {
    id: 'session-orbit-dashboard',
    projectId: 'proj-orbit',
    title: 'Polish dashboard accessibility',
    model: 'gpt-5.6-sol',
    agents: [
      { id: 'agent-chart', parentId: 'session-orbit-dashboard', title: 'chart behavior', model: 'gpt-5.6-sol', nickname: 'Feynman', depth: 1 },
      { id: 'agent-ui', parentId: 'session-orbit-dashboard', title: 'desktop UI polish', model: 'gpt-5.6-terra', nickname: 'Hypatia', depth: 1 },
      { id: 'agent-privacy', parentId: 'session-orbit-dashboard', title: 'privacy boundary', model: 'gpt-5.6-luna', nickname: 'Rawls', depth: 1 },
    ],
  },
] as const;

interface MockFact {
  date: string;
  accountId: string;
  projectId: string;
  modelId: string;
  quality: DataQuality;
  events: number;
  usage: TokenUsage;
}

function startOfUtcDay(value = new Date()): Date {
  return new Date(Date.UTC(value.getUTCFullYear(), value.getUTCMonth(), value.getUTCDate()));
}

function isoDate(value: Date): string {
  return value.toISOString().slice(0, 10);
}

function dateFromOffset(anchor: Date, daysAgo: number): string {
  return isoDate(new Date(anchor.getTime() - daysAgo * DAY_MS));
}

function makeUsage(input: number, cached: number, output: number, reasoning: number, cacheWrite = 0, cacheWriteObservedInput = input): TokenUsage {
  const safeCached = Math.min(input, cached);
  const safeCacheWrite = Math.min(Math.max(input - safeCached, 0), cacheWrite);
  return {
    input,
    cached: safeCached,
    cacheWrite: safeCacheWrite,
    cacheWriteObservedInput: Math.min(input, cacheWriteObservedInput),
    cacheWriteCoverage: input ? Math.min(input, cacheWriteObservedInput) / input : 0,
    uncached: Math.max(0, input - safeCached - safeCacheWrite),
    output,
    reasoning: Math.min(output, reasoning),
    total: input + output,
  };
}

function emptyUsage(): TokenUsage {
  return makeUsage(0, 0, 0, 0);
}

function addUsage(target: TokenUsage, usage: TokenUsage): void {
  target.input += usage.input;
  target.cached += usage.cached;
  target.cacheWrite += usage.cacheWrite;
  target.cacheWriteObservedInput += usage.cacheWriteObservedInput;
  target.cacheWriteCoverage = target.input ? target.cacheWriteObservedInput / target.input : 0;
  target.uncached += usage.uncached;
  target.output += usage.output;
  target.reasoning += usage.reasoning;
  target.total += usage.total;
}

function emptyQualityUsage(): QualityUsage {
  return {
    confirmed: emptyUsage(),
    quarantined: emptyUsage(),
    unknown: emptyUsage(),
  };
}

function aggregateByQuality(facts: MockFact[]): QualityUsage {
  const result = emptyQualityUsage();
  for (const fact of facts) addUsage(result[fact.quality], fact.usage);
  return result;
}

function buildFacts(anchor: Date): MockFact[] {
  const facts: MockFact[] = [];

  for (let daysAgo = DEMO_DAYS - 1; daysAgo >= 0; daysAgo -= 1) {
    const dayIndex = DEMO_DAYS - 1 - daysAgo;
    const date = dateFromOffset(anchor, daysAgo);

    ACCOUNTS.slice(0, 2).forEach((account, accountIndex) => {
      const project = PROJECTS[(dayIndex + accountIndex) % PROJECTS.length];
      const model = MODELS[(dayIndex * 2 + accountIndex) % MODELS.length];
      const wave = 0.78 + ((dayIndex * 7 + accountIndex * 3) % 11) / 14;
      const base = Math.round((1_900_000 + accountIndex * 760_000) * wave);
      const cached = Math.round(base * (0.61 + ((dayIndex + accountIndex) % 5) * 0.045));
      const output = Math.round(base * (0.075 + (dayIndex % 4) * 0.009));
      const reasoning = Math.round(output * (0.24 + accountIndex * 0.08));

      facts.push({
        date,
        accountId: account.id,
        projectId: project.id,
        modelId: model.id,
        quality: 'confirmed',
        events: 8 + ((dayIndex + accountIndex) % 7),
        usage: makeUsage(base, cached, output, reasoning),
      });

      if ((dayIndex + accountIndex * 3) % 13 === 0) {
        facts.push({
          date,
          accountId: account.id,
          projectId: project.id,
          modelId: model.id,
          quality: 'quarantined',
          events: 2 + (dayIndex % 3),
          usage: makeUsage(
            Math.round(base * 0.31),
            Math.round(cached * 0.34),
            Math.round(output * 0.28),
            Math.round(reasoning * 0.24),
          ),
        });
      }
    });

    if (dayIndex % 17 === 4 || dayIndex % 19 === 8) {
      const project = PROJECTS[dayIndex % PROJECTS.length];
      const input = 310_000 + (dayIndex % 5) * 47_000;
      facts.push({
        date,
        accountId: 'acct-unknown',
        projectId: project.id,
        modelId: MODELS[dayIndex % MODELS.length].id,
        quality: 'unknown',
        events: 1 + (dayIndex % 2),
        usage: makeUsage(input, Math.round(input * 0.72), Math.round(input * 0.08), Math.round(input * 0.017)),
      });
    }
  }

  return facts;
}

function periodDays(period: PeriodKey): number {
  if (period === 'today') return 1;
  if (period === 'week' || period === 'rolling7') return 7;
  if (period === 'month' || period === 'rolling30') return 30;
  return DEMO_DAYS;
}

function periodWindow(anchor: Date, period: PeriodKey): PeriodWindow {
  const days = Math.min(periodDays(period), DEMO_DAYS);
  const option = PERIODS.find((item) => item.id === period) ?? PERIODS[2];
  const start = dateFromOffset(anchor, days - 1);
  const end = isoDate(anchor);
  const windowKind = period === 'rolling7' || period === 'rolling30'
    ? 'rolling'
    : period === 'lifetime'
      ? 'lifetime'
      : 'calendar';
  return {
    key: period,
    label: option.label,
    start,
    end,
    timezone: 'Asia/Shanghai',
    windowKind,
    crossesMonth: start.slice(0, 7) !== end.slice(0, 7),
    crossesYear: start.slice(0, 4) !== end.slice(0, 4),
  };
}

function filterFacts(facts: MockFact[], filters: DashboardFilters, anchor: Date): MockFact[] {
  const window = periodWindow(anchor, filters.period);
  return facts.filter((fact) => {
    if (fact.date < window.start || fact.date > window.end) return false;
    if (filters.account !== ALL && fact.accountId !== filters.account) return false;
    if (filters.project !== ALL && fact.projectId !== filters.project) return false;
    if (filters.model !== ALL && fact.modelId !== filters.model) return false;
    return true;
  });
}

function labelFor(dimension: BreakdownDimension, id: string): { label: string; description?: string } {
  const options = dimension === 'account' ? ACCOUNTS : dimension === 'project' ? PROJECTS : MODELS;
  return options.find((option) => option.id === id) ?? { label: id };
}

function buildBreakdown(facts: MockFact[], dimension: BreakdownDimension): BreakdownRow[] {
  const keyName = `${dimension}Id` as 'accountId' | 'projectId' | 'modelId';
  const groups = new Map<string, MockFact[]>();
  for (const fact of facts) {
    const key = fact[keyName];
    groups.set(key, [...(groups.get(key) ?? []), fact]);
  }

  const totalConfirmed = aggregateByQuality(facts).confirmed.total;
  return [...groups.entries()]
    .map(([id, rows]) => {
      const usage = aggregateByQuality(rows);
      const label = labelFor(dimension, id);
      return {
        id,
        label: label.label,
        description: label.description,
        usage,
        confirmedEvents: rows
          .filter((row) => row.quality === 'confirmed')
          .reduce((sum, row) => sum + row.events, 0),
        shareOfConfirmed: totalConfirmed ? usage.confirmed.total / totalConfirmed : 0,
      };
    })
    .sort((a, b) => b.usage.confirmed.total - a.usage.confirmed.total);
}

function buildTimeseries(facts: MockFact[]): TimeseriesPoint[] {
  const byDate = new Map<string, MockFact[]>();
  for (const fact of facts) byDate.set(fact.date, [...(byDate.get(fact.date) ?? []), fact]);
  return [...byDate.entries()]
    .map(([date, rows]) => ({
      date,
      ...aggregateByQuality(rows),
      confirmedEvents: rows.filter((row) => row.quality === 'confirmed').reduce((sum, row) => sum + row.events, 0),
      quarantinedEvents: rows.filter((row) => row.quality === 'quarantined').reduce((sum, row) => sum + row.events, 0),
      unknownEvents: rows.filter((row) => row.quality === 'unknown').reduce((sum, row) => sum + row.events, 0),
    }))
    .sort((a, b) => a.date.localeCompare(b.date));
}

function mockOfficial(points: TimeseriesPoint[], total: number): OfficialUsageView {
  const officialPoints = points.map((point) => ({ date: point.date, tokens: Math.round(point.confirmed.total * 1.4) }));
  return {
    source: 'codex_account_usage_read',
    primaryScope: true,
    authoritativeForAccountTotal: true,
    accountCoverageComplete: true,
    identityScopeComplete: true,
    knownAccountCount: 1,
    observedAccountCount: 1,
    userConfirmedAccountCount: 1,
    unobservedAccountCount: 0,
    verifiedAccountCount: 1,
    missingOfficialAccountCount: 0,
    provisionalIdentityCount: 0,
    provisionalLocalTokens: 0,
    totalIsLowerBound: false,
    displayTotalTokens: Math.round(total * 1.4),
    displayTotalKind: 'official',
    displayIsLowerBound: false,
    localTailTokens: 0,
    missingAccountLocalTokens: 0,
    localComplementTokens: 0,
    localTailStart: null,
    totalTokens: Math.round(total * 1.4),
    bucketTotalTokens: officialPoints.reduce((sum, point) => sum + point.tokens, 0),
    lifetimeTokens: 61_052_184_141,
    peakDailyTokens: 4_124_570_551,
    previousTotalTokens: null,
    deltaTokens: null,
    deltaPercent: null,
    displayPreviousTotalTokens: null,
    previousDisplayIsLowerBound: false,
    displayDeltaTokens: null,
    displayDeltaPercent: null,
    points: officialPoints,
    comparisonPoints: [],
    accountCount: 1,
    observedAt: new Date().toISOString(),
    coverageStart: officialPoints[0]?.date ?? null,
    coverageThrough: officialPoints.at(-1)?.date ?? null,
    commonCoverageStart: officialPoints[0]?.date ?? null,
    commonCoverageThrough: officialPoints.at(-1)?.date ?? null,
    latestCoverageThrough: officialPoints.at(-1)?.date ?? null,
    accountCoverage: [{
      accountId: 'acct-personal',
      coverageStart: officialPoints[0]?.date ?? null,
      coverageThrough: officialPoints.at(-1)?.date ?? null,
      officialAvailable: true,
    }],
    reconciledPoints: officialPoints.map((point) => ({
      date: point.date,
      value: point.tokens,
      officialTokens: point.tokens,
      localTailTokens: 0,
      localOnlyTokens: 0,
      status: 'exact_official',
      coveredAccounts: 1,
      knownAccounts: 1,
    })),
    reconciledComparisonPoints: [],
    coverageComplete: true,
    coverageRatio: 1,
    periodExact: true,
    backendIncludesToday: true,
    granularity: 'day',
    lastError: null,
  };
}

function mockMissingAccountEstimate(): MissingAccountEstimate {
  return {
    definitionId: 'missing_accounts_residual_v1',
    status: 'insufficient_coverage',
    applicable: false,
    isEstimate: true,
    isConservativeFloor: true,
    canSplitByMissingAccount: false,
    combinedUnobservedAccountCount: 0,
    capturedAccountCount: 1,
    coverageStart: null,
    coverageThrough: null,
    alignedAccountDays: 0,
    excessAccountDays: 0,
    excludedAccountDays: 0,
    rawResidualTokens: 0,
    allocationRoundingDelta: 0,
    componentInvariantMismatchTokens: 0,
    localAssignedOnAlignedDays: 0,
    officialOnAlignedDays: 0,
    knownLocalCappedTokens: 0,
    selectedUsage: emptyUsage(),
    totalUsage: emptyUsage(),
    byDay: [],
    byProject: [],
    byModel: [],
    sourceAccountExcess: [],
    method: '逐账号逐日比较；仅分配 max(本地归因 - 同账号官方日桶, 0)',
  };
}

function mockAttributionCoverage(localTokens: number, accountTokens: number): AttributionCoverage {
  const unassigned = Math.round(localTokens * 0.07);
  const standalone = Math.round(localTokens * 0.05);
  return {
    definitionId: 'project_attribution_coverage_v1',
    accountTotalTokens: accountTokens,
    officialBaseTokens: accountTokens,
    localComplementTokens: 0,
    localAttributedTokens: localTokens,
    selectedLocalTokens: localTokens,
    namedProjectTokens: localTokens - unassigned - standalone,
    unassignedTokens: unassigned,
    standaloneConversationTokens: standalone,
    standaloneConversations: {
      current: 2,
      historical: 0,
      indexed: 2,
      withLocalEvidence: 2,
    },
    unattributedTokens: 0,
    coverageRatio: localTokens ? 1 : null,
    officialWindowStart: null,
    officialWindowThrough: null,
    localWindowStart: null,
    localWindowThrough: null,
    officialDayCount: 0,
    localEvidenceDayCount: 0,
    localOnOfficialDays: localTokens,
    gapBuckets: [
      { id: 'official_before_local_evidence', label: '官方早于本机证据', tokens: 0, detail: '演示数据' },
      { id: 'official_days_without_local_evidence', label: '无本机采样证据的官方日期', tokens: Math.max(accountTokens - localTokens, 0), detail: '演示数据' },
      { id: 'overlap_and_unbucketed_gap', label: '其余净差', tokens: 0, detail: '演示数据' },
    ],
    canAllocateGapToProjects: false,
    scope: 'official_account_total_vs_this_machine_attribution',
  };
}

function quotaStatus(usedPercent: number | null): QuotaPool['status'] {
  if (usedPercent == null) return 'unknown';
  if (usedPercent >= 90) return 'critical';
  if (usedPercent >= 72) return 'warning';
  return 'healthy';
}

function buildQuotaPools(anchor: Date, filters: DashboardFilters): QuotaPool[] {
  const now = new Date(anchor.getTime() + 10 * 60 * 60 * 1000).toISOString();
  const rows: Array<Omit<QuotaPool, 'status'>> = [
    {
      id: 'personal-primary',
      accountId: 'acct-personal',
      accountLabel: 'Personal · Pro',
      limitId: 'codex-primary',
      label: 'Weekly agent pool',
      usedPercent: 68,
      windowMinutes: 10_080,
      resetsAt: new Date(anchor.getTime() + 4 * DAY_MS + 9 * 60 * 60 * 1000).toISOString(),
      observedAt: now,
      stale: false,
      detail: 'Primary Codex allowance',
    },
    {
      id: 'personal-secondary',
      accountId: 'acct-personal',
      accountLabel: 'Personal · Pro',
      limitId: 'codex-secondary',
      label: 'Burst pool',
      usedPercent: 34,
      windowMinutes: 300,
      resetsAt: new Date(anchor.getTime() + 3 * 60 * 60 * 1000).toISOString(),
      observedAt: now,
      stale: false,
      detail: 'Short-window capacity',
    },
    {
      id: 'research-primary',
      accountId: 'acct-research',
      accountLabel: 'Research workspace',
      limitId: 'codex-primary',
      label: 'Workspace weekly pool',
      usedPercent: 82,
      windowMinutes: 10_080,
      resetsAt: new Date(anchor.getTime() + 2 * DAY_MS + 7 * 60 * 60 * 1000).toISOString(),
      observedAt: new Date(anchor.getTime() + 8 * 60 * 60 * 1000).toISOString(),
      stale: true,
      detail: 'Last observation is older than the freshness target',
    },
  ];

  return rows
    .filter((row) => filters.account === ALL || row.accountId === filters.account)
    .map((row) => ({ ...row, status: quotaStatus(row.usedPercent) }));
}

function buildTimeline(anchor: Date, filters: DashboardFilters): TimelineEvent[] {
  const rows: TimelineEvent[] = [
    {
      id: 'switch-1',
      at: new Date(anchor.getTime() - 23 * DAY_MS + 11 * 60 * 60 * 1000).toISOString(),
      kind: 'account_switch',
      accountId: 'acct-research',
      title: 'Switched to Research workspace',
      detail: 'Verified from a new account epoch; historical rows were not rewritten.',
      confidence: 'verified',
    },
    {
      id: 'reset-1',
      at: new Date(anchor.getTime() - 18 * DAY_MS + 7 * 60 * 60 * 1000).toISOString(),
      kind: 'quota_reset',
      accountId: 'acct-research',
      title: 'Workspace weekly pool reset',
      detail: 'Observed usage returned to the start of a new 10,080-minute window.',
      confidence: 'verified',
    },
    {
      id: 'switch-2',
      at: new Date(anchor.getTime() - 9 * DAY_MS + 16 * 60 * 60 * 1000).toISOString(),
      kind: 'account_switch',
      accountId: 'acct-personal',
      title: 'Switched to Personal · Pro',
      detail: 'Account fingerprint changed at this point in the local source.',
      confidence: 'verified',
    },
    {
      id: 'reset-2',
      at: new Date(anchor.getTime() - 4 * DAY_MS + 5 * 60 * 60 * 1000).toISOString(),
      kind: 'quota_reset',
      accountId: 'acct-personal',
      title: 'Burst pool reset',
      detail: 'Short-window quota reset observed from a clean token event.',
      confidence: 'verified',
    },
  ];
  const window = periodWindow(anchor, filters.period);
  return rows.filter((row) => {
    const date = row.at.slice(0, 10);
    if (date < window.start || date > window.end) return false;
    return filters.account === ALL || row.accountId === filters.account;
  });
}

function delayed(signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(resolve, 90);
    signal?.addEventListener(
      'abort',
      () => {
        window.clearTimeout(timer);
        reject(new DOMException('Request aborted', 'AbortError'));
      },
      { once: true },
    );
  });
}

function scaledUsage(usage: TokenUsage, ratio: number): TokenUsage {
  return makeUsage(
    Math.round(usage.input * ratio),
    Math.round(usage.cached * ratio),
    Math.round(usage.output * ratio),
    Math.round(usage.reasoning * ratio),
    Math.round(usage.cacheWrite * ratio),
    Math.round(usage.cacheWriteObservedInput * ratio),
  );
}

function explorerFor(facts: MockFact[], filters: DashboardFilters, anchor: Date): ExplorerResponse {
  const aggregate = (next: DashboardFilters) => aggregateByQuality(filterFacts(facts, next, anchor)).confirmed;
  const lifetime = aggregate({ ...filters, period: 'lifetime', project: filters.project });
  const week = aggregate({ ...filters, period: 'week', project: filters.project });
  const today = aggregate({ ...filters, period: 'today', project: filters.project });
  const selectedPeriod = aggregate(filters);
  const allSessions = MOCK_SESSIONS.filter((session) => filters.project === ALL || session.projectId === filters.project);
  const now = new Date(anchor.getTime() + 10 * 60 * 60 * 1000);

  const projects = PROJECTS.map((project) => {
    const periodFacts = filterFacts(facts, { ...filters, project: project.id }, anchor);
    const periodUsage = aggregateByQuality(periodFacts).confirmed;
    const periodEvents = periodFacts.filter((fact) => fact.quality === 'confirmed').reduce((sum, fact) => sum + fact.events, 0);
    const lifetimeUsage = aggregate({ ...filters, project: project.id, period: 'lifetime' });
    const weekUsage = aggregate({ ...filters, project: project.id, period: 'week' });
    const monthUsage = aggregate({ ...filters, project: project.id, period: 'month' });
    const todayUsage = aggregate({ ...filters, project: project.id, period: 'today' });
    const sessions = MOCK_SESSIONS.filter((session) => session.projectId === project.id);
    return {
      id: project.id,
      label: project.label,
      kind: 'project' as const,
      sessionCount: sessions.length,
      subagentCount: sessions.reduce((sum, session) => sum + session.agents.length, 0),
      orphanSubagentCount: 0,
      historicalSessionCount: 0,
      historicalSubagentCount: 0,
      periodUsage,
      periodEvents,
      previousPeriodUsage: scaledUsage(periodUsage, 0.72 + PROJECTS.findIndex((item) => item.id === project.id) * 0.08),
      previousPeriodEvents: Math.round(periodEvents * 0.8),
      recent15Usage: scaledUsage(todayUsage, 0.03),
      recent15Events: Math.round(periodEvents * 0.01),
      activeSessionCount: sessions.length ? 1 : 0,
      sparkline: [0.5, 0.8, 0.65, 1, 0.9, 1.2].map((ratio) => Math.round(periodUsage.total * ratio / 6)),
      lifetimeUsage,
      weekUsage,
      weekPreviousUsage: scaledUsage(weekUsage, 0.78),
      monthUsage,
      monthPreviousUsage: scaledUsage(monthUsage, 0.74),
      todayUsage,
      lastActiveAt: new Date(now.getTime() - PROJECTS.findIndex((item) => item.id === project.id) * 42 * 60_000).toISOString(),
    };
  }).sort((left, right) => right.periodUsage.total - left.periodUsage.total);

  const sessions = allSessions.map((session, index) => {
    const projectUsage = projects.find((project) => project.id === session.projectId)?.periodUsage ?? emptyUsage();
    const treeUsage = scaledUsage(projectUsage, index % 2 === 0 ? 0.58 : 0.37);
    const ownUsage = scaledUsage(treeUsage, session.agents.length ? 0.44 : 1);
    const updatedAt = new Date(now.getTime() - index * 37 * 60_000).toISOString();
    return {
      id: session.id,
      title: session.title,
      model: session.model,
      createdAt: new Date(now.getTime() - (index + 2) * DAY_MS).toISOString(),
      updatedAt,
      archived: false,
      presentInCodex: true,
      hasUserEvent: true,
      subagentCount: session.agents.length,
      ownUsage,
      treeUsage,
      eventCount: 36 + index * 11,
      active: index === 0,
      kind: 'session' as const,
    };
  });

  const selected = filters.session === ALL ? undefined : MOCK_SESSIONS.find((session) => session.id === filters.session);
  const selectedRow = sessions.find((session) => session.id === filters.session);
  const selectedSession = selected && selectedRow ? (() => {
    const agentRatio = selected.agents.length ? 0.56 / selected.agents.length : 0;
    const agentUsage = selected.agents.map((agent) => scaledUsage(selectedRow.treeUsage, agentRatio));
    const nodes = [
      {
        id: selected.id,
        parentId: null,
        projectId: selected.projectId,
        projectName: PROJECTS.find((project) => project.id === selected.projectId)?.label ?? null,
        title: selected.title,
        model: selected.model,
        agentNickname: null,
        agentRole: null,
        agentPath: null,
        depth: 0,
        relativeDepth: 0,
        createdAt: selectedRow.createdAt,
        updatedAt: selectedRow.updatedAt,
        archived: false,
        presentInCodex: true,
        sourceKind: 'state_5',
        ownUsage: selectedRow.ownUsage,
        subtreeUsage: selectedRow.treeUsage,
        eventCount: selectedRow.eventCount,
        subtreeEventCount: selectedRow.eventCount + selected.agents.reduce((sum, _agent, index) => sum + 8 + index * 4, 0),
      },
      ...selected.agents.map((agent, index) => ({
        id: agent.id,
        parentId: agent.parentId,
        projectId: selected.projectId,
        projectName: PROJECTS.find((project) => project.id === selected.projectId)?.label ?? null,
        title: agent.title,
        model: agent.model,
        agentNickname: agent.nickname,
        agentRole: agent.depth === 1 ? 'explorer' : 'reviewer',
        agentPath: `/root/${agent.title.replaceAll(' ', '_')}`,
        depth: agent.depth,
        relativeDepth: agent.depth,
        createdAt: new Date(new Date(selectedRow.createdAt).getTime() + (index + 1) * 8 * 60_000).toISOString(),
        updatedAt: new Date(new Date(selectedRow.updatedAt).getTime() - index * 4 * 60_000).toISOString(),
        archived: false,
        presentInCodex: true,
        sourceKind: 'state_5',
        ownUsage: agentUsage[index],
        subtreeUsage: agentUsage[index],
        eventCount: 8 + index * 4,
        subtreeEventCount: 8 + index * 4,
      })),
    ];
    return {
      id: selected.id,
      title: selected.title,
      projectId: selected.projectId,
      projectName: PROJECTS.find((project) => project.id === selected.projectId)?.label ?? null,
      model: selected.model,
      createdAt: selectedRow.createdAt,
      updatedAt: selectedRow.updatedAt,
      presentInCodex: true,
      ownUsage: selectedRow.ownUsage,
      treeUsage: selectedRow.treeUsage,
      subagentCount: selected.agents.length,
      samplingTimeline: buildTimeseries(filterFacts(facts, filters, anchor)).map((point) => ({
        bucket: point.date,
        events: point.confirmedEvents,
        usage: point.confirmed,
      })),
      ownSamplingTimeline: buildTimeseries(filterFacts(facts, filters, anchor)).map((point) => ({
        bucket: point.date,
        events: Math.max(1, Math.round(point.confirmedEvents * (selectedRow.ownUsage.total / Math.max(selectedRow.treeUsage.total, 1)))),
        usage: scaledUsage(point.confirmed, selectedRow.ownUsage.total / Math.max(selectedRow.treeUsage.total, 1)),
      })),
      samplingGrain: 'day' as const,
      officialThreadUsage: null,
      nodes,
      truncated: false,
    };
  })() : null;

  return {
    generatedAt: new Date().toISOString(),
    period: filters.period,
    rankingWindows: {
      week: periodWindow(anchor, 'week'),
      month: periodWindow(anchor, 'month'),
      lifetime: periodWindow(anchor, 'lifetime'),
    },
    stats: {
      projectCount: PROJECTS.length,
      sessionCount: allSessions.length,
      subagentCount: allSessions.reduce((sum, session) => sum + session.agents.length, 0),
      orphanSubagentCount: 0,
      historicalSessionCount: 0,
      historicalSubagentCount: 0,
      standaloneConversations: {
        current: 0,
        historical: 0,
        indexed: 0,
        withLocalEvidence: 0,
        lifetimeUsage: emptyUsage(),
        selectedPeriodUsage: emptyUsage(),
      },
      lifetime,
      week,
      today,
      selectedPeriod,
      localRecent15Minutes: scaledUsage(today, 0.025),
      localRecent15Events: Math.round(12 * 0.025),
      official: {
        todayTokens: Math.round(today.total * 1.4),
        weekTokens: Math.round(week.total * 1.4),
        monthTokens: 12_800_000_000,
        selectedPeriodTokens: Math.round(selectedPeriod.total * 1.4),
        lifetimeTokens: 61_052_184_141,
        peakDailyTokens: 4_124_570_551,
        coverageThrough: isoDate(now),
        observedAt: now.toISOString(),
        backendIncludesToday: true,
        accountCoverageComplete: true,
        knownAccountCount: 1,
        missingOfficialAccountCount: 0,
        totalIsLowerBound: false,
      },
      activeSessions: today.total ? 2 : 0,
      latestConfirmedAt: now.toISOString(),
    },
    projects,
    sessions,
    selectedSession,
  };
}

export class MockLedgerApi implements LedgerApi {
  readonly mode = 'mock' as const;
  private readonly anchor = startOfUtcDay();
  private readonly facts = buildFacts(this.anchor);

  async refreshOfficial(): Promise<void> {
    await delayed();
  }

  async setUserConfirmedAccountCount(): Promise<void> {
    await delayed();
  }

  async refreshOfficialThread(): Promise<void> {
    await delayed();
  }

  async getSummary(filters: DashboardFilters, signal?: AbortSignal): Promise<SummaryResponse> {
    await delayed(signal);
    const facts = filterFacts(this.facts, filters, this.anchor);
    const usage = aggregateByQuality(facts);
    const period = periodWindow(this.anchor, filters.period);
    const official = mockOfficial(buildTimeseries(facts), usage.confirmed.total);
    const confirmedEvents = facts
      .filter((fact) => fact.quality === 'confirmed')
      .reduce((sum, fact) => sum + fact.events, 0);
    const confirmedDates = facts.filter((fact) => fact.quality === 'confirmed').map((fact) => fact.date);

    return {
      generatedAt: new Date().toISOString(),
      mode: 'mock',
      period,
      filters: FILTER_CATALOG,
      usage,
      official,
      attributionCoverage: mockAttributionCoverage(usage.confirmed.total, official.displayTotalTokens ?? 0),
      missingAccountEstimate: mockMissingAccountEstimate(),
      metrics: {
        accountTotal: {
          value: official.displayTotalTokens,
          source: 'official',
          status: 'exact',
          windowStart: period.start,
          windowEnd: period.end,
          timezone: period.timezone,
          accountScope: filters.account,
          machineScope: 'all_devices',
          coverage: {
            complete: true,
            ratio: 1,
            knownAccountCount: official.knownAccountCount,
            missingOfficialAccountCount: 0,
          },
          definitionId: 'account_total_v1',
        },
        localAttributedTotal: {
          value: usage.confirmed.total,
          source: 'local',
          status: 'local_sample',
          windowStart: period.start,
          windowEnd: period.end,
          timezone: period.timezone,
          accountScope: filters.account,
          machineScope: 'this_machine',
          coverage: {
            complete: true,
            ratio: 1,
            knownAccountCount: official.knownAccountCount,
            missingOfficialAccountCount: 0,
          },
          definitionId: 'local_attributed_total_v1',
        },
      },
      confirmedEvents,
      cacheRate: usage.confirmed.input ? usage.confirmed.cached / usage.confirmed.input : 0,
      latestConfirmedAt: confirmedDates.length ? `${confirmedDates.sort().at(-1)}T10:00:00.000Z` : null,
      quotaPools: buildQuotaPools(this.anchor, filters),
      quotaCycles: buildQuotaPools(this.anchor, filters).map((pool) => ({
        id: `${pool.id}:cycle`,
        accountId: pool.accountId,
        accountLabel: pool.accountLabel,
        limitId: pool.limitId,
        label: pool.label,
        role: 'primary',
        windowKind: pool.windowMinutes === 10_080 ? 'weekly' : 'short',
        windowMinutes: pool.windowMinutes,
        cycleStart: pool.resetsAt && pool.windowMinutes ? new Date(Date.parse(pool.resetsAt) - pool.windowMinutes * 60_000).toISOString() : null,
        cycleEnd: pool.resetsAt,
        firstObservedAt: new Date(this.anchor.getTime() - 2 * DAY_MS).toISOString(),
        lastObservedAt: pool.observedAt,
        firstUsedPercent: pool.usedPercent == null ? null : Math.max(0, pool.usedPercent - 18),
        usedPercent: pool.usedPercent,
        usedDeltaPercent: pool.usedPercent == null ? null : 18,
        sampleCount: 24,
        localObservationStart: new Date(this.anchor.getTime() - 2 * DAY_MS).toISOString(),
        localCoverageRatio: 0.72,
        localUsage: usage.confirmed,
        localEvents: confirmedEvents,
        empiricalTokensPerUsedPercent: pool.usedPercent == null ? null : usage.confirmed.total / 18,
        empiricalRatioIsConversion: false,
      })),
      comparison: {
        usage: emptyUsage(),
        deltaTokens: usage.confirmed.total,
        deltaPercent: null,
        available: false,
        previousEvents: 0,
      },
      averagePerDay: usage.confirmed.total / Math.max(periodDays(filters.period), 1),
      matchRate: confirmedEvents ? confirmedEvents / Math.max(confirmedEvents + 2, 1) : 1,
      unmatchedEvents: facts.filter((fact) => fact.quality === 'unknown').reduce((sum, fact) => sum + fact.events, 0),
      reconciliation: {
        comparable: false,
        officialTotalTokens: Math.round(usage.confirmed.total * 1.4),
        localAttributedTokens: usage.confirmed.total,
        attributionGapTokens: null,
        localCoverageComplete: true,
        reason: 'mock attribution scope differs',
      },
    };
  }

  async getTimeseries(filters: DashboardFilters, signal?: AbortSignal): Promise<TimeseriesResponse> {
    await delayed(signal);
    const facts = filterFacts(this.facts, filters, this.anchor);
    const points = buildTimeseries(facts);
    return {
      generatedAt: new Date().toISOString(),
      period: periodWindow(this.anchor, filters.period),
      grain: 'day',
      points,
      comparisonPoints: [],
      projectSeries: PROJECTS.map((project) => {
        const projectFacts = facts.filter((fact) => fact.projectId === project.id);
        return {
          id: project.id,
          label: project.label,
          totalTokens: aggregateByQuality(projectFacts).confirmed.total,
          points: buildTimeseries(projectFacts).map((point) => ({ date: point.date, confirmed: point.confirmed, confirmedEvents: point.confirmedEvents })),
        };
      }).filter((series) => series.totalTokens > 0),
      official: mockOfficial(points, points.reduce((sum, point) => sum + point.confirmed.total, 0)),
      timeline: buildTimeline(this.anchor, filters),
    };
  }

  async getBreakdowns(filters: DashboardFilters, signal?: AbortSignal): Promise<BreakdownsResponse> {
    await delayed(signal);
    const facts = filterFacts(this.facts, filters, this.anchor);
    return {
      generatedAt: new Date().toISOString(),
      period: periodWindow(this.anchor, filters.period),
      account: buildBreakdown(facts, 'account'),
      project: buildBreakdown(facts, 'project'),
      model: buildBreakdown(facts, 'model'),
      officialAccounts: ACCOUNTS.slice(0, 2).map((account, index) => ({
        id: account.id,
        label: account.label,
        active: index === 0,
        officialAvailable: true,
        planType: index === 0 ? 'pro' : 'plus',
        todayTokens: 420_000_000 - index * 80_000_000,
        todayIsLowerBound: false,
        weekTokens: 3_100_000_000 - index * 600_000_000,
        weekIsLowerBound: false,
        monthTokens: 12_800_000_000 - index * 2_000_000_000,
        monthIsLowerBound: false,
        lifetimeTokens: 31_000_000_000 - index * 4_000_000_000,
        lifetimeIsLowerBound: false,
        coverageStart: '2026-05-07',
        coverageThrough: isoDate(this.anchor),
        observedAt: new Date().toISOString(),
        authEpochCount: 2 + index,
        firstSeenAt: '2026-05-07T00:00:00Z',
        lastSeenAt: new Date().toISOString(),
      })),
    };
  }

  async getQuality(filters: DashboardFilters, signal?: AbortSignal): Promise<QualityResponse> {
    await delayed(signal);
    const facts = filterFacts(this.facts, filters, this.anchor);
    const usage = aggregateByQuality(facts);
    const stateSummaries: QualityStateSummary[] = [
      {
        state: 'confirmed',
        eventCount: facts.filter((fact) => fact.quality === 'confirmed').reduce((sum, fact) => sum + fact.events, 0),
        tokenCount: usage.confirmed.total,
        usage: usage.confirmed,
        description: 'Included in the trusted total after replay and counter checks.',
      },
      {
        state: 'quarantined',
        eventCount: facts.filter((fact) => fact.quality === 'quarantined').reduce((sum, fact) => sum + fact.events, 0),
        tokenCount: usage.quarantined.total,
        usage: usage.quarantined,
        description: 'Excluded: evidence indicates replay, regression, or ambiguous lineage.',
      },
      {
        state: 'unknown',
        eventCount: facts.filter((fact) => fact.quality === 'unknown').reduce((sum, fact) => sum + fact.events, 0),
        tokenCount: null,
        usage: usage.unknown,
        description: 'Excluded until attribution or schema evidence becomes sufficient.',
      },
    ];
    const window = periodWindow(this.anchor, filters.period);
    const issues: QualityIssue[] = [
      {
        id: 'foreign-session-meta',
        state: 'quarantined',
        severity: 'critical',
        title: 'Foreign session metadata replay detected',
        detail: 'A child rollout replayed parent session metadata before token samples. The replay batch is excluded.',
        eventCount: stateSummaries[1].eventCount,
        tokenCount: stateSummaries[1].tokenCount,
        firstSeen: `${window.start}T00:00:00.000Z`,
        lastSeen: `${window.end}T08:48:19.000Z`,
      },
      {
        id: 'historical-account-unknown',
        state: 'unknown',
        severity: 'warning',
        title: 'Historical account attribution unavailable',
        detail: 'The current auth snapshot cannot reliably identify older usage. These rows stay unassigned.',
        eventCount: stateSummaries[2].eventCount,
        tokenCount: stateSummaries[2].tokenCount,
        firstSeen: `${window.start}T00:00:00.000Z`,
        lastSeen: `${window.end}T02:17:00.000Z`,
      },
    ];
    const sources: SourceHealth[] = [
      {
        sourceId: 'mac-local',
        label: 'Local Codex home',
        machineLabel: 'Mac workstation',
        status: 'fresh',
        lastObservedAt: new Date(this.anchor.getTime() + 10 * 60 * 60 * 1000).toISOString(),
        lagSeconds: 18,
      },
      {
        sourceId: 'research-profile',
        label: 'Research profile',
        machineLabel: 'Mac workstation',
        status: 'delayed',
        lastObservedAt: new Date(this.anchor.getTime() + 8 * 60 * 60 * 1000).toISOString(),
        lagSeconds: 7_200,
      },
    ];

    return {
      generatedAt: new Date().toISOString(),
      trustedPolicy: 'Only confirmed event deltas contribute to trusted usage. Quarantined and unknown remain visible.',
      states: stateSummaries,
      issues: issues.filter((issue) => issue.eventCount > 0),
      sources: sources.filter((source) => filters.account !== 'acct-unknown' || source.sourceId === 'mac-local'),
      reconstruction: {
        pendingSources: 12,
        reconstructingSources: 4,
        reconstructedSources: 128,
        unrecoverableSources: 2,
        bytesProcessed: 8_400_000_000,
        bytesTotal: 12_000_000_000,
        selectedTokens: Math.round(usage.confirmed.total * 0.42),
      },
    };
  }

  async getExplorer(filters: DashboardFilters, signal?: AbortSignal): Promise<ExplorerResponse> {
    await delayed(signal);
    return explorerFor(this.facts, filters, this.anchor);
  }

  async getBundle(filters: DashboardFilters, signal?: AbortSignal): Promise<DashboardBundle> {
    const [summary, timeseries, breakdowns, quality, explorer] = await Promise.all([
      this.getSummary(filters, signal),
      this.getTimeseries(filters, signal),
      this.getBreakdowns(filters, signal),
      this.getQuality(filters, signal),
      this.getExplorer(filters, signal),
    ]);
    return {
      summary,
      timeseries,
      breakdowns,
      quality,
      explorer,
      collection: {
        mode: 'mock',
        phase: 'live',
        itemsTotal: 60,
        itemsCompleted: 60,
        bytesRead: 0,
        eventsInserted: summary.confirmedEvents,
        message: null,
        updatedAt: new Date().toISOString(),
        rollupItemsTotal: summary.confirmedEvents,
        rollupItemsCompleted: summary.confirmedEvents,
        rollupComplete: true,
        rawRetentionDays: 7,
      },
    };
  }
}
