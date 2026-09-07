use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryView {
    pub instance: String,
    pub revision: i64,
    pub account: String,
    pub as_of: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryKey {
    pub at: String,
    pub snapshot_id: String,
    pub ordinal: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryCursor {
    pub view: QuotaHistoryView,
    pub before: QuotaHistoryKey,
    pub signature: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryInterval {
    pub id: String,
    pub account_id: String,
    pub key: QuotaHistoryKey,
    pub stream_key: String,
    pub pool_key: String,
    pub limit_id: String,
    pub limit_name: Nullable<String>,
    pub role: String,
    pub window_seconds: Nullable<String>,
    pub boundary_kind: String,
    pub boundary_after: Nullable<String>,
    pub first_observed_at: String,
    pub last_observed_at: String,
    pub sample_count: u64,
    pub first_used_percent: Nullable<f64>,
    pub last_used_percent: Nullable<f64>,
    pub nominal_start: Nullable<String>,
    pub reported_reset: Nullable<String>,
    pub token_sample_start: Nullable<String>,
    pub token_sample_end: Nullable<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryResponse {
    pub scope: String,
    pub index_ready: bool,
    pub source_history_complete: bool,
    pub view: QuotaHistoryView,
    pub intervals: Vec<QuotaHistoryInterval>,
    pub next: Nullable<QuotaHistoryCursor>,
    pub selections: Option<Vec<QuotaHistoryCursor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaIntervalUsageGroup {
    pub id: Nullable<String>,
    pub label: Nullable<String>,
    pub events: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuotaIntervalUsageResponse {
    pub scope: String,
    pub status: String,
    pub interval_id: String,
    pub account_id: String,
    pub quota_view: QuotaHistoryView,
    pub observed_at: String,
    pub start: Nullable<String>,
    pub end: Nullable<String>,
    pub coverage_complete: bool,
    pub pool_attribution: bool,
    pub events: Nullable<u64>,
    pub usage: Nullable<TokenUsage>,
    pub models: Vec<QuotaIntervalUsageGroup>,
    pub projects: Vec<QuotaIntervalUsageGroup>,
}
