use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

string_enum!(DataQuality {
    Confirmed,
    Quarantined,
    Unknown
});
string_enum!(QuotaPoolStatus {
    Healthy,
    Warning,
    Critical,
    Unknown
});
string_enum!(MetricSource {
    Official,
    Local,
    Reconciled
});
string_enum!(MetricStatus {
    Exact,
    LowerBound,
    LocalSample,
    Unknown
});
string_enum!(QualitySeverity {
    Info,
    Warning,
    Critical
});
string_enum!(SourceStatus {
    Fresh,
    Delayed,
    Offline
});
string_enum!(TimelineEventKind {
    AccountSwitch,
    QuotaReset,
    QuotaResetScheduled
});
string_enum!(AttributionConfidence {
    Verified,
    Inferred,
    Unknown
});
string_enum!(CollectionPhase {
    Idle,
    Optimizing,
    Compacting,
    Backfill,
    Syncing,
    Live
});
string_enum!(ExplorerProjectKind {
    Project,
    StandaloneConversations,
    UnmatchedRecords
});
string_enum!(ExplorerSessionKind {
    Session,
    OrphanSubagent
});
string_enum!(ReconciledAccountDayStatus {
    ExactOfficial,
    LocalTail,
    LocalOnlyAccount,
    Unknown
});
string_enum!(QuotaWindowKind {
    Weekly,
    Short,
    Custom,
    Unknown
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nullable<T>(pub Option<T>);

impl<T: JsonSchema> JsonSchema for Nullable<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("RequiredNullable_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("RequiredNullable<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        <Option<T> as JsonSchema>::json_schema(generator)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DataMode {
    Mock,
    Http,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TimeGrain {
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PeriodKey {
    Today,
    Week,
    Rolling7,
    Month,
    Rolling30,
    Weeks12,
    Months12,
    Lifetime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PeriodWindowKind {
    Calendar,
    Rolling,
    Lifetime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DisplayTotalKind {
    Official,
    OfficialPlusLocalTailLowerBound,
    LocalLowerBound,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MissingEstimateStatus {
    ConservativeFloor,
    InsufficientCoverage,
    NotApplicableToSingleAccount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MachineScope {
    AllDevices,
    ThisMachine,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input: f64,
    pub cached: f64,
    pub cache_write: f64,
    pub cache_write_observed_input: f64,
    pub cache_write_coverage: f64,
    pub uncached: f64,
    pub output: f64,
    pub reasoning: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QualityUsage {
    pub confirmed: TokenUsage,
    pub quarantined: TokenUsage,
    pub unknown: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DimensionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeriodOption {
    pub id: PeriodKey,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FilterCatalog {
    pub accounts: Vec<DimensionOption>,
    pub projects: Vec<DimensionOption>,
    pub models: Vec<DimensionOption>,
    pub periods: Vec<PeriodOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PeriodWindow {
    pub key: PeriodKey,
    pub label: String,
    pub start: String,
    pub end: String,
    pub timezone: String,
    pub definition: Option<String>,
    pub comparison_start: Option<String>,
    pub comparison_end: Option<String>,
    pub coverage_start: Option<String>,
    pub coverage_complete: Option<bool>,
    pub coverage_offset: Option<f64>,
    pub coverage_ratio: Option<f64>,
    pub comparison_available: Option<bool>,
    pub partial: Option<bool>,
    pub default_grain: Option<TimeGrain>,
    pub window_kind: PeriodWindowKind,
    pub crosses_month: bool,
    pub crosses_year: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OfficialUsagePoint {
    pub date: String,
    pub tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledAccountDay {
    pub date: String,
    pub value: f64,
    pub official_tokens: f64,
    pub local_tail_tokens: f64,
    pub local_only_tokens: f64,
    pub status: ReconciledAccountDayStatus,
    pub covered_accounts: f64,
    pub known_accounts: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfficialAccountCoverage {
    pub account_id: String,
    pub coverage_start: Nullable<String>,
    pub coverage_through: Nullable<String>,
    pub official_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsageView {
    pub source: String,
    pub primary_scope: bool,
    pub authoritative_for_account_total: bool,
    pub account_coverage_complete: bool,
    pub identity_scope_complete: bool,
    pub known_account_count: f64,
    pub observed_account_count: f64,
    pub user_confirmed_account_count: Nullable<f64>,
    pub unobserved_account_count: f64,
    pub verified_account_count: f64,
    pub missing_official_account_count: f64,
    pub provisional_identity_count: f64,
    pub provisional_local_tokens: f64,
    pub total_is_lower_bound: bool,
    pub display_total_tokens: Nullable<f64>,
    pub display_total_kind: DisplayTotalKind,
    pub display_is_lower_bound: bool,
    pub local_tail_tokens: f64,
    pub missing_account_local_tokens: f64,
    pub local_complement_tokens: f64,
    pub local_tail_start: Nullable<String>,
    pub total_tokens: Nullable<f64>,
    pub bucket_total_tokens: f64,
    pub lifetime_tokens: Nullable<f64>,
    pub peak_daily_tokens: Nullable<f64>,
    pub previous_total_tokens: Nullable<f64>,
    pub delta_tokens: Nullable<f64>,
    pub delta_percent: Nullable<f64>,
    pub display_previous_total_tokens: Nullable<f64>,
    pub previous_display_is_lower_bound: bool,
    pub display_delta_tokens: Nullable<f64>,
    pub display_delta_percent: Nullable<f64>,
    pub points: Vec<OfficialUsagePoint>,
    pub comparison_points: Vec<OfficialUsagePoint>,
    pub account_count: f64,
    pub observed_at: Nullable<String>,
    pub coverage_start: Nullable<String>,
    pub coverage_through: Nullable<String>,
    pub common_coverage_start: Nullable<String>,
    pub common_coverage_through: Nullable<String>,
    pub latest_coverage_through: Nullable<String>,
    pub account_coverage: Vec<OfficialAccountCoverage>,
    pub reconciled_points: Vec<ReconciledAccountDay>,
    pub reconciled_comparison_points: Vec<ReconciledAccountDay>,
    pub coverage_complete: bool,
    pub coverage_ratio: f64,
    pub period_exact: bool,
    pub backend_includes_today: bool,
    pub granularity: TimeGrain,
    pub last_error: Nullable<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingAccountEstimateBreakdown {
    pub id: String,
    pub label: String,
    pub usage: TokenUsage,
    pub source_events: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingAccountEstimateDay {
    pub date: String,
    pub usage: TokenUsage,
    pub source_events: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingSourceAccountExcess {
    pub account_id: String,
    pub account_label: String,
    pub aligned_days: f64,
    pub excess_days: f64,
    pub local_tokens: f64,
    pub official_tokens: f64,
    pub estimated_tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingAccountEstimate {
    pub definition_id: String,
    pub status: MissingEstimateStatus,
    pub applicable: bool,
    pub is_estimate: bool,
    pub is_conservative_floor: bool,
    pub can_split_by_missing_account: bool,
    pub combined_unobserved_account_count: f64,
    pub captured_account_count: f64,
    pub coverage_start: Nullable<String>,
    pub coverage_through: Nullable<String>,
    pub aligned_account_days: f64,
    pub excess_account_days: f64,
    pub excluded_account_days: f64,
    pub raw_residual_tokens: f64,
    pub allocation_rounding_delta: f64,
    pub component_invariant_mismatch_tokens: f64,
    pub local_assigned_on_aligned_days: f64,
    pub official_on_aligned_days: f64,
    pub known_local_capped_tokens: f64,
    pub selected_usage: TokenUsage,
    pub total_usage: TokenUsage,
    pub by_day: Vec<MissingAccountEstimateDay>,
    pub by_project: Vec<MissingAccountEstimateBreakdown>,
    pub by_model: Vec<MissingAccountEstimateBreakdown>,
    pub source_account_excess: Vec<MissingSourceAccountExcess>,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StandaloneConversationCoverage {
    pub current: f64,
    pub historical: f64,
    pub indexed: f64,
    pub with_local_evidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttributionGapBucket {
    pub id: String,
    pub label: String,
    pub tokens: f64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCoverage {
    pub definition_id: String,
    pub account_total_tokens: Nullable<f64>,
    pub official_base_tokens: f64,
    pub local_complement_tokens: f64,
    pub local_attributed_tokens: f64,
    pub selected_local_tokens: f64,
    pub named_project_tokens: f64,
    pub unassigned_tokens: f64,
    pub standalone_conversation_tokens: f64,
    pub standalone_conversations: StandaloneConversationCoverage,
    pub unattributed_tokens: f64,
    pub coverage_ratio: Nullable<f64>,
    pub official_window_start: Nullable<String>,
    pub official_window_through: Nullable<String>,
    pub local_window_start: Nullable<String>,
    pub local_window_through: Nullable<String>,
    pub official_day_count: f64,
    pub local_evidence_day_count: f64,
    pub local_on_official_days: f64,
    pub gap_buckets: Vec<AttributionGapBucket>,
    pub can_allocate_gap_to_projects: bool,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaPool {
    pub id: String,
    pub account_id: String,
    pub account_label: String,
    pub limit_id: String,
    pub label: String,
    pub used_percent: Nullable<f64>,
    pub window_minutes: Nullable<f64>,
    pub resets_at: Nullable<String>,
    pub observed_at: String,
    pub status: QuotaPoolStatus,
    pub stale: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaCycle {
    pub id: String,
    pub account_id: String,
    pub account_label: String,
    pub limit_id: String,
    pub label: String,
    pub role: String,
    pub window_kind: QuotaWindowKind,
    pub window_minutes: Nullable<f64>,
    pub cycle_start: Nullable<String>,
    pub cycle_end: Nullable<String>,
    pub first_observed_at: String,
    pub last_observed_at: String,
    pub first_used_percent: Nullable<f64>,
    pub used_percent: Nullable<f64>,
    pub used_delta_percent: Nullable<f64>,
    pub sample_count: f64,
    pub local_observation_start: String,
    pub local_coverage_ratio: Nullable<f64>,
    pub local_usage: TokenUsage,
    pub local_events: f64,
    pub empirical_tokens_per_used_percent: Nullable<f64>,
    pub empirical_ratio_is_conversion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricCoverage {
    pub complete: bool,
    pub ratio: f64,
    pub known_account_count: f64,
    pub missing_official_account_count: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMetric {
    pub value: Nullable<f64>,
    pub source: MetricSource,
    pub status: MetricStatus,
    pub window_start: Nullable<String>,
    pub window_end: Nullable<String>,
    pub timezone: String,
    pub account_scope: String,
    pub machine_scope: MachineScope,
    pub coverage: MetricCoverage,
    pub definition_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryMetrics {
    pub account_total: ResolvedMetric,
    pub local_attributed_total: ResolvedMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryComparison {
    pub usage: TokenUsage,
    pub delta_tokens: f64,
    pub delta_percent: Nullable<f64>,
    pub available: bool,
    pub previous_events: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryReconciliation {
    pub comparable: bool,
    pub official_total_tokens: Nullable<f64>,
    pub local_attributed_tokens: f64,
    pub attribution_gap_tokens: Nullable<f64>,
    pub local_coverage_complete: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResponse {
    pub generated_at: String,
    pub mode: DataMode,
    pub period: PeriodWindow,
    pub filters: FilterCatalog,
    pub usage: QualityUsage,
    pub official: OfficialUsageView,
    pub attribution_coverage: AttributionCoverage,
    pub missing_account_estimate: MissingAccountEstimate,
    pub metrics: SummaryMetrics,
    pub confirmed_events: f64,
    pub cache_rate: f64,
    pub latest_confirmed_at: Nullable<String>,
    pub quota_pools: Vec<QuotaPool>,
    pub quota_cycles: Vec<QuotaCycle>,
    pub comparison: SummaryComparison,
    pub average_per_day: f64,
    pub match_rate: f64,
    pub unmatched_events: f64,
    pub reconciliation: SummaryReconciliation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeseriesPoint {
    pub date: String,
    pub confirmed: TokenUsage,
    pub quarantined: TokenUsage,
    pub unknown: TokenUsage,
    pub confirmed_events: f64,
    pub quarantined_events: f64,
    pub unknown_events: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    pub at: String,
    pub kind: TimelineEventKind,
    pub account_id: Nullable<String>,
    pub title: String,
    pub detail: String,
    pub confidence: AttributionConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeseriesComparisonPoint {
    pub date: String,
    pub confirmed: TokenUsage,
    #[serde(rename = "confirmedEvents")]
    pub confirmed_events: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSeries {
    pub id: String,
    pub label: String,
    pub total_tokens: f64,
    pub points: Vec<TimeseriesComparisonPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeseriesResponse {
    pub generated_at: String,
    pub period: PeriodWindow,
    pub grain: TimeGrain,
    pub points: Vec<TimeseriesPoint>,
    pub comparison_points: Vec<TimeseriesComparisonPoint>,
    pub project_series: Vec<ProjectSeries>,
    pub official: OfficialUsageView,
    pub timeline: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownRow {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub usage: QualityUsage,
    pub confirmed_events: f64,
    pub share_of_confirmed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfficialAccountRow {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub official_available: bool,
    pub plan_type: Nullable<String>,
    pub today_tokens: Nullable<f64>,
    pub today_is_lower_bound: bool,
    pub week_tokens: Nullable<f64>,
    pub week_is_lower_bound: bool,
    pub month_tokens: Nullable<f64>,
    pub month_is_lower_bound: bool,
    pub lifetime_tokens: Nullable<f64>,
    pub lifetime_is_lower_bound: bool,
    pub coverage_start: Nullable<String>,
    pub coverage_through: Nullable<String>,
    pub observed_at: Nullable<String>,
    pub auth_epoch_count: f64,
    pub first_seen_at: Nullable<String>,
    pub last_seen_at: Nullable<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownsResponse {
    pub generated_at: String,
    pub period: PeriodWindow,
    pub account: Vec<BreakdownRow>,
    pub project: Vec<BreakdownRow>,
    pub model: Vec<BreakdownRow>,
    pub official_accounts: Vec<OfficialAccountRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityIssue {
    pub id: String,
    pub state: DataQuality,
    pub severity: QualitySeverity,
    pub title: String,
    pub detail: String,
    pub event_count: f64,
    pub token_count: Nullable<f64>,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityStateSummary {
    pub state: DataQuality,
    pub event_count: f64,
    pub token_count: Nullable<f64>,
    pub usage: TokenUsage,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceHealth {
    pub source_id: String,
    pub label: String,
    pub machine_label: String,
    pub status: SourceStatus,
    pub last_observed_at: String,
    pub lag_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReconstructionSummary {
    pub pending_sources: f64,
    pub reconstructing_sources: f64,
    pub reconstructed_sources: f64,
    pub unrecoverable_sources: f64,
    pub bytes_processed: f64,
    pub bytes_total: f64,
    pub selected_tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QualityResponse {
    #[serde(rename = "generatedAt")]
    pub generated_at: String,
    #[serde(rename = "trustedPolicy")]
    pub trusted_policy: String,
    pub states: Vec<QualityStateSummary>,
    pub issues: Vec<QualityIssue>,
    pub sources: Vec<SourceHealth>,
    pub reconstruction: ReconstructionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionStatus {
    pub mode: String,
    pub phase: CollectionPhase,
    pub items_total: f64,
    pub items_completed: f64,
    pub bytes_read: f64,
    pub events_inserted: f64,
    pub message: Option<String>,
    pub updated_at: String,
    pub rollup_items_total: f64,
    pub rollup_items_completed: f64,
    pub rollup_complete: bool,
    pub raw_retention_days: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerStandaloneStats {
    pub current: f64,
    pub historical: f64,
    pub indexed: f64,
    pub with_local_evidence: f64,
    pub lifetime_usage: TokenUsage,
    pub selected_period_usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerOfficialStats {
    pub today_tokens: Nullable<f64>,
    pub week_tokens: Nullable<f64>,
    pub month_tokens: Nullable<f64>,
    pub selected_period_tokens: Nullable<f64>,
    pub lifetime_tokens: Nullable<f64>,
    pub peak_daily_tokens: Nullable<f64>,
    pub coverage_through: Nullable<String>,
    pub observed_at: Nullable<String>,
    pub backend_includes_today: bool,
    pub account_coverage_complete: bool,
    pub known_account_count: f64,
    pub verified_account_count: Option<f64>,
    pub missing_official_account_count: f64,
    pub total_is_lower_bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerStats {
    pub project_count: f64,
    pub session_count: f64,
    pub subagent_count: f64,
    pub orphan_subagent_count: f64,
    pub historical_session_count: f64,
    pub historical_subagent_count: f64,
    pub standalone_conversations: ExplorerStandaloneStats,
    pub lifetime: TokenUsage,
    pub week: TokenUsage,
    pub today: TokenUsage,
    pub selected_period: TokenUsage,
    pub local_recent15_minutes: TokenUsage,
    pub local_recent15_events: f64,
    pub official: ExplorerOfficialStats,
    pub active_sessions: f64,
    pub latest_confirmed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerProject {
    pub id: String,
    pub label: String,
    pub kind: ExplorerProjectKind,
    pub session_count: f64,
    pub subagent_count: f64,
    pub orphan_subagent_count: f64,
    pub historical_session_count: f64,
    pub historical_subagent_count: f64,
    pub period_usage: TokenUsage,
    pub period_events: f64,
    pub previous_period_usage: TokenUsage,
    pub previous_period_events: f64,
    pub recent15_usage: TokenUsage,
    pub recent15_events: f64,
    pub active_session_count: f64,
    pub sparkline: Vec<f64>,
    pub lifetime_usage: TokenUsage,
    pub week_usage: TokenUsage,
    pub week_previous_usage: TokenUsage,
    pub month_usage: TokenUsage,
    pub month_previous_usage: TokenUsage,
    pub today_usage: TokenUsage,
    pub last_active_at: Nullable<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSession {
    pub id: String,
    pub title: String,
    pub model: Nullable<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
    pub present_in_codex: bool,
    pub has_user_event: bool,
    pub subagent_count: f64,
    pub own_usage: TokenUsage,
    pub tree_usage: TokenUsage,
    pub event_count: f64,
    pub active: bool,
    pub kind: ExplorerSessionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSessionNode {
    pub id: String,
    pub parent_id: Nullable<String>,
    pub project_id: Nullable<String>,
    pub project_name: Nullable<String>,
    pub title: String,
    pub model: Nullable<String>,
    pub agent_nickname: Nullable<String>,
    pub agent_role: Nullable<String>,
    pub agent_path: Nullable<String>,
    pub depth: f64,
    pub relative_depth: f64,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
    pub source_kind: String,
    pub present_in_codex: bool,
    pub own_usage: TokenUsage,
    pub subtree_usage: TokenUsage,
    pub event_count: f64,
    pub subtree_event_count: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SamplingTimelinePoint {
    pub bucket: String,
    pub events: f64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfficialThreadUsageGroup {
    pub model: Nullable<String>,
    pub reasoning_effort: Nullable<String>,
    pub speed: Nullable<String>,
    pub estimated_usage_credits_micros: f64,
    pub net_new_input_tokens: Nullable<f64>,
    pub cached_input_tokens: Nullable<f64>,
    pub cache_write_input_tokens: Nullable<f64>,
    pub input_tokens: Nullable<f64>,
    pub output_tokens: Nullable<f64>,
    pub total_tokens: Nullable<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfficialThreadUsage {
    pub observed_at: String,
    pub estimated_usage_credits_micros: f64,
    pub estimated_usage_usd_micros: Nullable<f64>,
    pub groups: Vec<OfficialThreadUsageGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSessionDetail {
    pub id: String,
    pub title: String,
    pub project_id: Nullable<String>,
    pub project_name: Nullable<String>,
    pub model: Nullable<String>,
    pub created_at: String,
    pub updated_at: String,
    pub present_in_codex: bool,
    pub own_usage: TokenUsage,
    pub tree_usage: TokenUsage,
    pub subagent_count: f64,
    pub sampling_timeline: Vec<SamplingTimelinePoint>,
    pub own_sampling_timeline: Vec<SamplingTimelinePoint>,
    pub sampling_grain: TimeGrain,
    pub official_thread_usage: Nullable<OfficialThreadUsage>,
    pub nodes: Vec<ExplorerSessionNode>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExplorerRankingWindows {
    pub week: PeriodWindow,
    pub month: PeriodWindow,
    pub lifetime: PeriodWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerResponse {
    pub generated_at: String,
    pub period: PeriodKey,
    pub ranking_windows: ExplorerRankingWindows,
    pub stats: ExplorerStats,
    pub projects: Vec<ExplorerProject>,
    pub sessions: Vec<ExplorerSession>,
    pub selected_session: Option<ExplorerSessionDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DashboardBundle {
    pub summary: SummaryResponse,
    pub timeseries: TimeseriesResponse,
    pub breakdowns: BreakdownsResponse,
    pub quality: QualityResponse,
    pub explorer: ExplorerResponse,
    pub collection: CollectionStatus,
}
