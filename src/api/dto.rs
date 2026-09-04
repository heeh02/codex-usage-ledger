use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::TokenUsage;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub cache_write_observed_input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

impl From<TokenUsage> for UsageTotals {
    fn from(value: TokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            cache_write_input_tokens: value.cache_write_input_tokens,
            cache_write_observed_input_tokens: value.cache_write_observed_input_tokens,
            uncached_input_tokens: value.uncached_input_tokens(),
            output_tokens: value.output_tokens,
            reasoning_output_tokens: value.reasoning_output_tokens,
            total_tokens: value.total_tokens,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualitySummary {
    pub confirmed: UsageTotals,
    pub quarantined: UsageTotals,
    pub unknown: UsageTotals,
    pub confirmed_events: u64,
    pub quarantined_events: u64,
    pub unknown_events: u64,
    pub missing_model_events: u64,
    pub missing_account_events: u64,
    pub replay_events_removed: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDescriptor {
    pub label: String,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub timezone: String,
    pub comparison_start: Option<DateTime<Utc>>,
    pub comparison_end: Option<DateTime<Utc>>,
    pub default_grain: String,
    pub partial: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    Official,
    Local,
    Reconciled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Exact,
    LowerBound,
    LocalSample,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricCoverage {
    pub complete: bool,
    pub ratio: f64,
    pub known_account_count: u64,
    pub missing_official_account_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMetric {
    pub value: Option<u64>,
    pub source: MetricSource,
    pub status: MetricStatus,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub timezone: String,
    pub account_scope: String,
    pub machine_scope: String,
    pub coverage: MetricCoverage,
    pub definition_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountDayStatus {
    ExactOfficial,
    LocalTail,
    LocalOnlyAccount,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledAccountDay {
    pub date: String,
    pub value: u64,
    pub official_tokens: u64,
    pub local_tail_tokens: u64,
    pub local_only_tokens: u64,
    pub status: AccountDayStatus,
    pub covered_accounts: u64,
    pub known_accounts: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesPoint {
    pub bucket: String,
    pub usage: UsageTotals,
    pub quarantined_tokens: u64,
    pub unknown_tokens: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownItem {
    pub key: String,
    pub label: String,
    pub usage: UsageTotals,
    pub event_count: u64,
    pub confidence: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindowView {
    pub role: String,
    pub duration_minutes: Option<u64>,
    pub used_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaPoolView {
    pub account_fingerprint: Option<String>,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub route_model: Option<String>,
    pub windows: Vec<QuotaWindowView>,
    pub observed_at: Option<DateTime<Utc>>,
    pub stale: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSwitchView {
    pub observed_at: DateTime<Utc>,
    pub from_fingerprint: Option<String>,
    pub to_fingerprint: Option<String>,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub generated_at: DateTime<Utc>,
    pub period: PeriodDescriptor,
    pub trusted_usage: UsageTotals,
    pub quality: QualitySummary,
    pub timeseries: Vec<SeriesPoint>,
    pub by_model: Vec<BreakdownItem>,
    pub by_account: Vec<BreakdownItem>,
    pub by_project: Vec<BreakdownItem>,
    pub quota_pools: Vec<QuotaPoolView>,
    pub account_switches: Vec<AccountSwitchView>,
    pub source_freshness: Option<DateTime<Utc>>,
}

impl Default for DashboardSnapshot {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            generated_at: Utc::now(),
            period: PeriodDescriptor {
                label: "lifetime".into(),
                start: None,
                end: None,
                timezone: "local".into(),
                comparison_start: None,
                comparison_end: None,
                default_grain: "day".into(),
                partial: false,
            },
            trusted_usage: UsageTotals::default(),
            quality: QualitySummary::default(),
            timeseries: Vec::new(),
            by_model: Vec::new(),
            by_account: Vec::new(),
            by_project: Vec::new(),
            quota_pools: Vec::new(),
            account_switches: Vec::new(),
            source_freshness: None,
        }
    }
}
