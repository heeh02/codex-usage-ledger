use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::Infallible,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Sse, sse::Event},
    routing::{get, post},
};
use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, LocalResult, NaiveDate, NaiveTime, TimeZone,
    Timelike, Utc,
};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::RwLock;

use crate::{
    official_usage::{fetch_official_account_usage, fetch_official_thread_usage},
    store::{
        AggregateDimension, AggregateFilter, DashboardCatalogCounts,
        DashboardCatalogThread as CatalogThread, LedgerStore, ResidualUsageRow,
        STANDALONE_CONVERSATIONS_PROJECT_ID as STANDALONE_PROJECT_ID, StoreError, TimeGrain,
        UNASSIGNED_PROJECT_ID, UsageAggregate,
    },
    types::{DataQuality, TokenUsage},
};

const STANDALONE_PROJECT_LABEL: &str = "独立对话";
const UNASSIGNED_PROJECT_LABEL: &str = "未匹配记录";
// Matches the UI's ledger-change debounce. Manual refreshes invalidate the
// cache immediately, while ordinary live updates remain at most one UI tick
// behind and repeated navigation never rescans the ledger.
const QUERY_CACHE_TTL: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct CachedValue {
    created_at: Instant,
    value: serde_json::Value,
}

mod dto;
pub use dto::*;
mod explorer;
#[cfg(test)]
use explorer::thread_label;
use explorer::{http_explorer, standalone_conversation_stats};
mod period;
#[cfg(test)]
use period::resolve_period_at;
use period::{local_midnight_utc, resolve_period};
mod presentation;
use presentation::{
    account_label, add_usage_saturating, breakdown_rows, earliest_event_at, filter_and_period,
    filter_catalog, latest_confirmed_at, parse_timestamp, period_value, quality_state_value,
    quality_usage_value, selected, source_health, token_value,
};
#[cfg(test)]
use presentation::{window_crosses_month, window_crosses_year};
mod quota;
use quota::{quota_cycle_views, quota_views, timeline_views};
#[cfg(test)]
use quota::{quota_display_label, quota_duration_label, quota_reset_events};
mod queries;
use queries::{
    aggregate_selected_period, aggregate_selected_period_by, http_breakdowns, http_bundle,
    http_quality, http_timeseries,
};
mod reconciliation;
use reconciliation::{
    account_registry, account_registry_value, aggregate_date_key, compact_official_usage_view,
    missing_account_estimate, official_day_bounds, official_usage_view,
    resolved_account_total_metric,
};
mod routes;
#[cfg(test)]
use routes::accepts_local_origin;
pub use routes::{ApiError, UsageQuery, router};
mod snapshot;
use snapshot::aggregate_for_quality;
pub use snapshot::snapshot_from_store;
mod summary;
use summary::http_summary;
#[cfg(test)]
use summary::project_attribution_coverage;
pub mod wire;

#[derive(Clone)]
pub struct ApiState {
    snapshot: Arc<RwLock<DashboardSnapshot>>,
    store: Option<Arc<Mutex<LedgerStore>>>,
    query_path: Option<PathBuf>,
    query_cache: Arc<Mutex<HashMap<String, CachedValue>>>,
}

impl ApiState {
    pub fn new(snapshot: DashboardSnapshot) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
            store: None,
            query_path: None,
            query_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_store(store: LedgerStore) -> Self {
        let query_path = store.database_path();
        Self {
            snapshot: Arc::new(RwLock::new(DashboardSnapshot::default())),
            store: Some(Arc::new(Mutex::new(store))),
            query_path,
            query_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn replace(&self, mut snapshot: DashboardSnapshot) {
        let mut current = self.snapshot.write().await;
        snapshot.revision = current.revision.saturating_add(1);
        snapshot.generated_at = Utc::now();
        *current = snapshot;
    }

    pub async fn snapshot(&self) -> DashboardSnapshot {
        self.snapshot.read().await.clone()
    }

    pub async fn snapshot_for_query(
        &self,
        query: UsageQuery,
    ) -> Result<DashboardSnapshot, ApiError> {
        let Some(store) = self.store.clone() else {
            return Ok(self.snapshot().await);
        };
        tokio::task::spawn_blocking(move || {
            let guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
            snapshot_from_store(&guard, &query).map_err(ApiError::from)
        })
        .await
        .map_err(|_| ApiError::WorkerStopped)?
    }

    pub async fn quota_json(&self) -> Result<serde_json::Value, ApiError> {
        self.query_value(UsageQuery::default(), |store, query| {
            Ok(serde_json::json!({"pools": quota_views(store, query)?}))
        })
        .await
    }

    pub async fn timeline_json(&self) -> Result<serde_json::Value, ApiError> {
        self.query_value(UsageQuery::default(), |store, query| {
            Ok(serde_json::json!({"items": timeline_views(store, query)?}))
        })
        .await
    }

    async fn query_value<F>(
        &self,
        query: UsageQuery,
        operation: F,
    ) -> Result<serde_json::Value, ApiError>
    where
        F: FnOnce(&LedgerStore, &UsageQuery) -> Result<serde_json::Value, StoreError>
            + Send
            + 'static,
    {
        let store = self.store.clone().ok_or(ApiError::StoreUnavailable)?;
        let query_path = self.query_path.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(path) = query_path {
                let reader = LedgerStore::open(path).map_err(ApiError::from)?;
                return operation(&reader, &query).map_err(ApiError::from);
            }
            let guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
            operation(&guard, &query).map_err(ApiError::from)
        })
        .await
        .map_err(|_| ApiError::WorkerStopped)?
    }

    async fn cached_query_value<F>(
        &self,
        namespace: &'static str,
        query: UsageQuery,
        operation: F,
    ) -> Result<serde_json::Value, ApiError>
    where
        F: FnOnce(&LedgerStore, &UsageQuery) -> Result<serde_json::Value, StoreError>
            + Send
            + 'static,
    {
        let encoded_query = serde_json::to_string(&query).unwrap_or_default();
        let key = format!("{namespace}:{encoded_query}");
        if let Ok(cache) = self.query_cache.lock()
            && let Some(entry) = cache.get(&key)
            && entry.created_at.elapsed() <= QUERY_CACHE_TTL
        {
            return Ok(entry.value.clone());
        }
        let value = self.query_value(query, operation).await?;
        if let Ok(mut cache) = self.query_cache.lock() {
            cache.retain(|_, entry| entry.created_at.elapsed() <= QUERY_CACHE_TTL);
            cache.insert(
                key,
                CachedValue {
                    created_at: Instant::now(),
                    value: value.clone(),
                },
            );
        }
        Ok(value)
    }

    fn invalidate_query_cache(&self) {
        if let Ok(mut cache) = self.query_cache.lock() {
            cache.clear();
        }
    }

    async fn refresh_official_usage(&self) -> Result<serde_json::Value, ApiError> {
        let store = self.store.clone().ok_or(ApiError::StoreUnavailable)?;
        let account = {
            let store = store.clone();
            tokio::task::spawn_blocking(move || {
                let guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
                guard
                    .active_account_fingerprint()
                    .map_err(ApiError::from)?
                    .ok_or(ApiError::ActiveAccountUnavailable)
            })
            .await
            .map_err(|_| ApiError::WorkerStopped)??
        };
        let usage = tokio::task::spawn_blocking(fetch_official_account_usage)
            .await
            .map_err(|_| ApiError::WorkerStopped)?
            .map_err(|error| ApiError::OfficialUsage(error.to_string()))?;
        let observed_at = Utc::now();
        let summary = usage.summary.clone();
        let bucket_count = usage.daily_usage_buckets.len();
        tokio::task::spawn_blocking(move || {
            let mut guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
            guard
                .upsert_official_account_usage(&account, observed_at, &usage)
                .map_err(ApiError::from)?;
            Ok(serde_json::json!({
                "status": "ok",
                "observedAt": observed_at,
                "lifetimeTokens": summary.lifetime_tokens,
                "peakDailyTokens": summary.peak_daily_tokens,
                "dailyBuckets": bucket_count,
            }))
        })
        .await
        .map_err(|_| ApiError::WorkerStopped)?
    }

    async fn refresh_official_thread_usage(
        &self,
        thread_id: String,
    ) -> Result<serde_json::Value, ApiError> {
        let store = self.store.clone().ok_or(ApiError::StoreUnavailable)?;
        let account = {
            let store = store.clone();
            tokio::task::spawn_blocking(move || {
                let guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
                guard
                    .active_account_fingerprint()
                    .map_err(ApiError::from)?
                    .ok_or(ApiError::ActiveAccountUnavailable)
            })
            .await
            .map_err(|_| ApiError::WorkerStopped)??
        };
        let requested_thread = thread_id.clone();
        let usage =
            tokio::task::spawn_blocking(move || fetch_official_thread_usage(&requested_thread))
                .await
                .map_err(|_| ApiError::WorkerStopped)?
                .map_err(|error| ApiError::OfficialUsage(error.to_string()))?;
        let observed_at = Utc::now();
        let Some(usage) = usage else {
            return Ok(serde_json::json!({"status": "unavailable", "threadId": thread_id}));
        };
        let response = usage.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
            guard
                .upsert_official_thread_usage(&account, observed_at, &usage)
                .map_err(ApiError::from)?;
            Ok(serde_json::json!({
                "status": "ok",
                "threadId": response.thread_id,
                "observedAt": observed_at,
                "groups": response.groups.len(),
                "estimatedUsageCreditsMicros": response.estimated_usage_credits_micros,
            }))
        })
        .await
        .map_err(|_| ApiError::WorkerStopped)?
    }
}

#[cfg(test)]
#[path = "api/tests.rs"]
mod tests;
