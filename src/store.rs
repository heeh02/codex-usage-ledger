use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Datelike, Duration as ChronoDuration, SecondsFormat, Timelike, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::identity::AuthIdentity;
use crate::official_usage::{OfficialAccountUsage, OfficialDailyUsageBucket, OfficialThreadUsage};
use crate::project::{ManualProjectAssignment, ProjectRecord, normalize_git_identity};
use crate::quota::{QuotaSnapshot, QuotaSource};
use crate::types::{
    AttributionConfidence, DataQuality, EventProvenance, ProjectAttribution, TokenUsage, UsageEvent,
};

mod account_repository;
mod core_repository;
mod dashboard_repository;
mod ingest_repository;
mod maintenance_repository;
mod migrations;
mod project_repository;
mod usage_repository;
#[cfg(test)]
use migrations::{
    CURRENT_SCHEMA_VERSION, MIGRATION_1, MIGRATION_2, MIGRATION_3, MIGRATION_4, MIGRATION_5,
    MIGRATION_6, MIGRATION_7, MIGRATION_8, MIGRATION_9, MIGRATION_10, MIGRATION_11, MIGRATION_12,
    MIGRATION_13, MIGRATION_14, MIGRATION_15,
};
pub const RAW_EVENT_RETENTION_DAYS: i64 = 7;
pub const STANDALONE_CONVERSATIONS_PROJECT_ID: &str = "__standalone_conversations__";
pub const UNASSIGNED_PROJECT_ID: &str = "unassigned";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{field} exceeds SQLite's signed 64-bit INTEGER range")]
    IntegerOverflow { field: &'static str },
    #[error("cursor regression for {machine_id}/{source_id}: {new_offset} is before {old_offset}")]
    CursorRegression {
        machine_id: String,
        source_id: String,
        old_offset: u64,
        new_offset: u64,
    },
    #[error("database schema {found} is newer than supported schema {supported}")]
    SchemaTooNew { found: i64, supported: i64 },
    #[error("invalid IANA timezone {0:?}")]
    InvalidTimezone(String),
    #[error("usage aggregate overflowed u64")]
    AggregateOverflow,
    #[error("daily rollup does not reconcile with raw event totals")]
    RollupMismatch,
    #[error("daily rollup must be verified before raw event compaction")]
    RollupNotVerified,
    #[error("compacted event {event_id} was replayed with different immutable usage data")]
    CompactedEventConflict { event_id: String },
    #[error("reconstruction event {event_id} was replayed with different immutable data")]
    ReconstructionEventConflict { event_id: String },
    #[error("confirmed token usage violates an accounting invariant: {0}")]
    InvalidConfirmedUsage(crate::types::TokenUsageInvariantError),
    #[error("{rows} persisted rows in {table} violate confirmed token invariants")]
    PersistedUsageInvariantViolation { table: &'static str, rows: u64 },
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCursor {
    pub machine_id: String,
    pub source_id: String,
    pub file_identity: String,
    pub byte_offset: u64,
    pub line_number: u64,
    /// Serialized replay/parser checkpoint needed to resume model/context and
    /// cumulative-token state exactly at `byte_offset`.
    pub parser_state_json: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionStatus {
    Pending,
    Reconstructing,
    Reconstructed,
    Unrecoverable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconstructionSourceStatus {
    pub machine_id: String,
    pub source_id: String,
    pub thread_id: String,
    pub file_identity: String,
    pub status: ReconstructionStatus,
    pub bytes_total: u64,
    pub bytes_processed: u64,
    pub prefix_events: u64,
    pub unchanged_events: u64,
    pub counter_resets: u64,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructionEvent {
    pub event: UsageEvent,
    pub counter_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Default)]
pub struct BatchOutcome {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
}

impl BatchOutcome {
    fn observe(&mut self, outcome: UpsertOutcome) {
        match outcome {
            UpsertOutcome::Inserted => self.inserted += 1,
            UpsertOutcome::Updated => self.updated += 1,
            UpsertOutcome::Unchanged => self.unchanged += 1,
        }
    }
}

/// Filters trusted usage aggregates. The default intentionally includes only
/// confirmed events; set `quality` to `None` to inspect every quality state.
#[derive(Debug, Clone)]
pub struct AggregateFilter {
    pub start_inclusive: Option<DateTime<Utc>>,
    pub end_exclusive: Option<DateTime<Utc>>,
    pub account_fingerprint: Option<String>,
    pub project_id: Option<String>,
    pub model: Option<String>,
    pub quality: Option<DataQuality>,
}

impl Default for AggregateFilter {
    fn default() -> Self {
        Self {
            start_inclusive: None,
            end_exclusive: None,
            account_fingerprint: None,
            project_id: None,
            model: None,
            quality: Some(DataQuality::Confirmed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateDimension {
    Model,
    Account,
    Project,
    Thread,
    Day,
    Quality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeGrain {
    Hour,
    Day,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSeriesBucket {
    pub time_key: String,
    pub dimension_key: Option<String>,
    pub event_count: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAggregate {
    pub event_count: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageBucket {
    /// `None` represents an unassigned model/account. Project aggregation uses
    /// explicit virtual keys for standalone-conversation and unmatched scopes.
    pub key: Option<String>,
    pub event_count: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootUsageBucket {
    pub root_thread_id: String,
    pub node_count: u64,
    pub own: UsageAggregate,
    pub tree: UsageAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthEpochRecord {
    pub epoch_id: i64,
    pub machine_id: String,
    pub source_id: String,
    pub generation: String,
    pub observed_from: DateTime<Utc>,
    pub observed_to: Option<DateTime<Utc>>,
    pub account_fingerprint: Option<String>,
    pub workspace_fingerprint: Option<String>,
    pub confidence: AttributionConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogMarkerRecord {
    pub log_id: u64,
    pub observed_at: DateTime<Utc>,
    pub kind: String,
    pub workspace_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalAuthEpochInput {
    pub observed_from: DateTime<Utc>,
    pub observed_to: Option<DateTime<Utc>>,
    pub account_fingerprint: String,
    pub workspace_fingerprint: String,
    pub confidence: AttributionConfidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredQuotaSnapshot {
    pub snapshot_id: String,
    pub account_fingerprint: String,
    pub auth_epoch: String,
    pub observed_at: DateTime<Utc>,
    pub snapshot: QuotaSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOfficialAccountUsage {
    pub snapshot_id: String,
    pub account_fingerprint: String,
    pub observed_at: DateTime<Utc>,
    pub usage: OfficialAccountUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialUsageSyncState {
    pub last_attempt_at: DateTime<Utc>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOfficialThreadUsage {
    pub account_fingerprint: String,
    pub thread_id: String,
    pub observed_at: DateTime<Utc>,
    pub usage: OfficialThreadUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredManualAssignment {
    pub assignment_key: String,
    pub assignment: ManualProjectAssignment,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadCatalogRecord {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
    pub depth: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived: bool,
    pub has_user_event: bool,
    pub source_kind: String,
}

/// Read-only catalog record used by dashboard query services. Unlike the
/// ingestion record, timestamps remain in their persisted representation so
/// the transport layer can preserve Codex's exact evidence boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardCatalogThread {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
    pub depth: u32,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
    pub has_user_event: bool,
    pub source_kind: String,
    pub present_in_codex: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DashboardCatalogCounts {
    pub current_sessions: usize,
    pub current_subagents: usize,
    pub current_orphan_subagents: usize,
    pub historical_sessions: usize,
    pub historical_subagents: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StandaloneConversationStats {
    pub current: u64,
    pub historical: u64,
    pub with_local_evidence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCursorHealth {
    pub machine_id: String,
    pub updated_at: String,
    pub file_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthEpochSummary {
    pub count: u64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconstructionSummary {
    pub pending: u64,
    pub reconstructing: u64,
    pub reconstructed: u64,
    pub unrecoverable: u64,
    pub bytes_processed: u64,
    pub bytes_total: u64,
    pub selected_tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerTableCounts {
    pub raw_events: u64,
    pub compacted_event_keys: u64,
    pub file_cursors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTimelineRow {
    pub epoch_id: i64,
    pub machine_id: String,
    pub source_id: String,
    pub observed_from: String,
    pub account_fingerprint: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualUsageRow {
    pub day: String,
    pub account: String,
    pub project: String,
    pub model: String,
    pub source_events: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollupProgress {
    pub last_backfilled_rowid: u64,
    pub target_rowid: u64,
    pub complete: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorStatus {
    pub mode: String,
    pub phase: String,
    pub items_total: u64,
    pub items_completed: u64,
    pub bytes_read: u64,
    pub events_inserted: u64,
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub struct LedgerStore {
    connection: Connection,
}

const EVENT_SELECT_COLUMNS: &str = "event_id, observed_at, source_timestamp, thread_id, \
parent_thread_id, model, cwd, account_fingerprint, account_confidence, project_id, \
project_name, project_confidence, project_method, input_tokens, cached_input_tokens, \
cache_write_input_tokens, cache_write_observed_input_tokens, output_tokens, \
reasoning_output_tokens, total_tokens, quality, quality_reason, machine_id, \
source_id, rollout_id, file_identity, byte_offset, line_number";

fn event_hash(event: &UsageEvent) -> StoreResult<String> {
    let encoded = serde_json::to_vec(event)?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn upsert_reconstruction_event_in(
    transaction: &rusqlite::Transaction<'_>,
    reconstruction: &ReconstructionEvent,
) -> StoreResult<UpsertOutcome> {
    let event = &reconstruction.event;
    event
        .usage
        .validate()
        .map_err(StoreError::InvalidConfirmedUsage)?;
    let new_hash = event_hash(event)?;
    let existing: Option<String> = transaction
        .query_row(
            "SELECT event_hash FROM reconstruction_usage_events WHERE event_id = ?1",
            [&event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if existing.as_deref() == Some(new_hash.as_str()) {
        return Ok(UpsertOutcome::Unchanged);
    }
    if existing.is_some() {
        return Err(StoreError::ReconstructionEventConflict {
            event_id: event.event_id.clone(),
        });
    }
    let source_timestamp = event.source_timestamp.unwrap_or(event.observed_at);
    transaction.execute(
        "INSERT INTO reconstruction_usage_events(
             event_id, event_hash, observed_at, source_timestamp, thread_id,
             parent_thread_id, model, cwd, account_fingerprint, account_confidence,
             project_id, project_name, project_confidence, project_method,
             input_tokens, cached_input_tokens, cache_write_input_tokens,
             cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
             total_tokens, machine_id, source_id, file_identity, byte_offset,
             line_number, counter_epoch
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
             ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
         )",
        params![
            event.event_id,
            new_hash,
            timestamp(event.observed_at),
            timestamp(source_timestamp),
            event.thread_id,
            event.parent_thread_id,
            event.model,
            event.cwd,
            event.account_fingerprint,
            confidence_name(event.account_confidence),
            event.project.project_id,
            event.project.project_name,
            confidence_name(event.project.confidence),
            event.project.method,
            sql_u64(event.usage.input_tokens, "input_tokens")?,
            sql_u64(event.usage.cached_input_tokens, "cached_input_tokens")?,
            sql_u64(
                event.usage.cache_write_input_tokens,
                "cache_write_input_tokens"
            )?,
            sql_u64(
                event.usage.cache_write_observed_input_tokens,
                "cache_write_observed_input_tokens"
            )?,
            sql_u64(event.usage.output_tokens, "output_tokens")?,
            sql_u64(
                event.usage.reasoning_output_tokens,
                "reasoning_output_tokens"
            )?,
            sql_u64(event.usage.total_tokens, "total_tokens")?,
            event.provenance.machine_id,
            event.provenance.source_id,
            event.provenance.file_identity,
            sql_u64(event.provenance.byte_offset, "byte_offset")?,
            sql_u64(event.provenance.line_number, "line_number")?,
            sql_u64(reconstruction.counter_epoch, "counter_epoch")?,
        ],
    )?;
    Ok(UpsertOutcome::Inserted)
}

fn upsert_reconstruction_source_in(
    transaction: &rusqlite::Transaction<'_>,
    source: &ReconstructionSourceStatus,
) -> StoreResult<()> {
    transaction.execute(
        "INSERT INTO reconstruction_sources(
             machine_id, source_id, thread_id, file_identity, status,
             bytes_total, bytes_processed, prefix_events, unchanged_events,
             counter_resets, last_error, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(machine_id, source_id) DO UPDATE SET
             thread_id = excluded.thread_id,
             file_identity = excluded.file_identity,
             status = excluded.status,
             bytes_total = excluded.bytes_total,
             bytes_processed = excluded.bytes_processed,
             prefix_events = excluded.prefix_events,
             unchanged_events = excluded.unchanged_events,
             counter_resets = excluded.counter_resets,
             last_error = excluded.last_error,
             updated_at = excluded.updated_at",
        params![
            source.machine_id,
            source.source_id,
            source.thread_id,
            source.file_identity,
            reconstruction_status_name(source.status),
            sql_u64(source.bytes_total, "bytes_total")?,
            sql_u64(source.bytes_processed, "bytes_processed")?,
            sql_u64(source.prefix_events, "prefix_events")?,
            sql_u64(source.unchanged_events, "unchanged_events")?,
            sql_u64(source.counter_resets, "counter_resets")?,
            source.last_error,
            timestamp(source.updated_at),
        ],
    )?;
    Ok(())
}

fn rebuild_reconstruction_rollups_in(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    transaction.execute_batch(
        "DELETE FROM reconstruction_daily_rollups;
         INSERT INTO reconstruction_daily_rollups(
             local_day, thread_key, account_key, project_key, model_key,
             event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
             cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
             total_tokens
         )
         SELECT date(source_timestamp, '+8 hours'), thread_id,
                COALESCE(account_fingerprint, ''), COALESCE(project_id, ''),
                COALESCE(model, ''), COUNT(*), SUM(input_tokens),
                SUM(cached_input_tokens), SUM(cache_write_input_tokens),
                SUM(cache_write_observed_input_tokens), SUM(output_tokens),
                SUM(reasoning_output_tokens), SUM(total_tokens)
         FROM reconstruction_usage_events
         GROUP BY 1, 2, 3, 4, 5;

         DELETE FROM reconstruction_hourly_rollups;
         INSERT INTO reconstruction_hourly_rollups(
             local_hour, thread_key, account_key, project_key, model_key,
             event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
             cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
             total_tokens
         )
         SELECT strftime('%Y-%m-%dT%H:00', source_timestamp, '+8 hours'), thread_id,
                COALESCE(account_fingerprint, ''), COALESCE(project_id, ''),
                COALESCE(model, ''), COUNT(*), SUM(input_tokens),
                SUM(cached_input_tokens), SUM(cache_write_input_tokens),
                SUM(cache_write_observed_input_tokens), SUM(output_tokens),
                SUM(reasoning_output_tokens), SUM(total_tokens)
         FROM reconstruction_usage_events
         GROUP BY 1, 2, 3, 4, 5;",
    )?;
    Ok(())
}

fn rebuild_standalone_membership_in(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    transaction.execute_batch(
        "DELETE FROM standalone_thread_membership;
         INSERT INTO standalone_thread_membership(thread_id, root_thread_id)
         WITH RECURSIVE tree(root_thread_id, thread_id) AS (
             SELECT thread_id, thread_id FROM thread_catalog
             WHERE parent_thread_id IS NULL AND COALESCE(depth, 0) = 0
               AND project_id IS NULL AND source_kind = 'state_5'
             UNION
             SELECT tree.root_thread_id, child.thread_id
             FROM tree JOIN thread_catalog child ON child.parent_thread_id = tree.thread_id
         )
         SELECT thread_id, MIN(root_thread_id) FROM tree GROUP BY thread_id;

         DELETE FROM thread_root_membership;
         INSERT INTO thread_root_membership(thread_id, root_thread_id, relative_depth)
         WITH RECURSIVE tree(root_thread_id, thread_id, relative_depth) AS (
             SELECT thread_id, thread_id, 0 FROM thread_catalog
             WHERE parent_thread_id IS NULL
             UNION
             SELECT tree.root_thread_id, child.thread_id, tree.relative_depth + 1
             FROM tree JOIN thread_catalog child ON child.parent_thread_id = tree.thread_id
             WHERE tree.relative_depth < 32
         )
         SELECT thread_id, MIN(root_thread_id), MIN(relative_depth)
         FROM tree GROUP BY thread_id;",
    )?;
    Ok(())
}

fn upsert_event_in(
    transaction: &rusqlite::Transaction<'_>,
    event: &UsageEvent,
) -> StoreResult<UpsertOutcome> {
    if event.quality == DataQuality::Confirmed {
        event
            .usage
            .validate()
            .map_err(StoreError::InvalidConfirmedUsage)?;
    }
    upsert_event_thread_catalog_in(transaction, event)?;
    let new_hash = event_hash(event)?;
    let old_hash: Option<String> = transaction
        .query_row(
            "SELECT event_hash FROM usage_events WHERE event_id = ?1",
            params![event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if old_hash.as_deref() == Some(new_hash.as_str()) {
        return Ok(UpsertOutcome::Unchanged);
    }

    transaction.execute(
        r#"INSERT INTO usage_events (
               event_id, event_hash, observed_at, source_timestamp, thread_id, parent_thread_id,
               model, cwd, account_fingerprint, account_confidence, project_id, project_name,
               project_confidence, project_method, input_tokens, cached_input_tokens,
               cache_write_input_tokens, cache_write_observed_input_tokens, output_tokens,
               reasoning_output_tokens, total_tokens, quality, quality_reason, machine_id, source_id,
               rollout_id, file_identity, byte_offset, line_number
           ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
               ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27,
               ?28, ?29
           ) ON CONFLICT(event_id) DO UPDATE SET
               event_hash = excluded.event_hash,
               observed_at = excluded.observed_at,
               source_timestamp = excluded.source_timestamp,
               thread_id = excluded.thread_id,
               parent_thread_id = excluded.parent_thread_id,
               model = excluded.model,
               cwd = excluded.cwd,
               account_fingerprint = excluded.account_fingerprint,
               account_confidence = excluded.account_confidence,
               project_id = excluded.project_id,
               project_name = excluded.project_name,
               project_confidence = excluded.project_confidence,
               project_method = excluded.project_method,
               input_tokens = excluded.input_tokens,
               cached_input_tokens = excluded.cached_input_tokens,
               cache_write_input_tokens = excluded.cache_write_input_tokens,
               cache_write_observed_input_tokens = excluded.cache_write_observed_input_tokens,
               output_tokens = excluded.output_tokens,
               reasoning_output_tokens = excluded.reasoning_output_tokens,
               total_tokens = excluded.total_tokens,
               quality = excluded.quality,
               quality_reason = excluded.quality_reason,
               machine_id = excluded.machine_id,
               source_id = excluded.source_id,
               rollout_id = excluded.rollout_id,
               file_identity = excluded.file_identity,
               byte_offset = excluded.byte_offset,
               line_number = excluded.line_number"#,
        params![
            event.event_id,
            new_hash,
            timestamp(event.observed_at),
            event.source_timestamp.map(timestamp),
            event.thread_id,
            event.parent_thread_id,
            event.model,
            event.cwd,
            event.account_fingerprint,
            confidence_name(event.account_confidence),
            event.project.project_id,
            event.project.project_name,
            confidence_name(event.project.confidence),
            event.project.method,
            sql_u64(event.usage.input_tokens, "input_tokens")?,
            sql_u64(event.usage.cached_input_tokens, "cached_input_tokens")?,
            sql_u64(
                event.usage.cache_write_input_tokens,
                "cache_write_input_tokens"
            )?,
            sql_u64(
                event.usage.cache_write_observed_input_tokens,
                "cache_write_observed_input_tokens"
            )?,
            sql_u64(event.usage.output_tokens, "output_tokens")?,
            sql_u64(
                event.usage.reasoning_output_tokens,
                "reasoning_output_tokens"
            )?,
            sql_u64(event.usage.total_tokens, "total_tokens")?,
            quality_name(event.quality),
            event.quality_reason,
            event.provenance.machine_id,
            event.provenance.source_id,
            event.provenance.rollout_id,
            event.provenance.file_identity,
            sql_u64(event.provenance.byte_offset, "byte_offset")?,
            sql_u64(event.provenance.line_number, "line_number")?,
        ],
    )?;
    Ok(if old_hash.is_some() {
        UpsertOutcome::Updated
    } else {
        UpsertOutcome::Inserted
    })
}

fn upsert_event_thread_catalog_in(
    transaction: &rusqlite::Transaction<'_>,
    event: &UsageEvent,
) -> StoreResult<()> {
    if let Some(thread_id) = event.thread_id.as_deref() {
        let effective_at = event.source_timestamp.unwrap_or(event.observed_at);
        upsert_thread_catalog_in(
            transaction,
            &ThreadCatalogRecord {
                thread_id: thread_id.to_owned(),
                parent_thread_id: event.parent_thread_id.clone(),
                project_id: event.project.project_id.clone(),
                project_name: event.project.project_name.clone(),
                title: None,
                model: event.model.clone(),
                agent_nickname: None,
                agent_role: None,
                agent_path: None,
                depth: event.parent_thread_id.as_ref().map(|_| 1),
                created_at: effective_at,
                updated_at: effective_at,
                archived: false,
                has_user_event: false,
                source_kind: "usage_events".to_owned(),
            },
        )?;
    }
    Ok(())
}

fn upsert_compact_event_in(
    transaction: &rusqlite::Transaction<'_>,
    event: &UsageEvent,
) -> StoreResult<UpsertOutcome> {
    let raw_hash: Option<String> = transaction
        .query_row(
            "SELECT event_hash FROM usage_events WHERE event_id = ?1",
            params![event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if raw_hash.is_some() {
        return upsert_event_in(transaction, event);
    }

    let new_hash = event_hash(event)?;
    let compacted_hash: Option<String> = transaction
        .query_row(
            "SELECT event_hash FROM compacted_event_keys WHERE event_id = ?1",
            params![event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing_hash) = compacted_hash {
        if existing_hash == new_hash {
            return Ok(UpsertOutcome::Unchanged);
        }
        return Err(StoreError::CompactedEventConflict {
            event_id: event.event_id.clone(),
        });
    }

    if event.quality == DataQuality::Confirmed {
        event
            .usage
            .validate()
            .map_err(StoreError::InvalidConfirmedUsage)?;
    }

    upsert_event_thread_catalog_in(transaction, event)?;
    transaction.execute(
        "INSERT INTO compacted_event_keys(event_id, event_hash, compacted_at)
         VALUES (?1, ?2, ?3)",
        params![event.event_id, new_hash, timestamp(Utc::now())],
    )?;
    upsert_rollup_delta_in(transaction, event)?;
    Ok(UpsertOutcome::Inserted)
}

fn upsert_rollup_delta_in(
    transaction: &rusqlite::Transaction<'_>,
    event: &UsageEvent,
) -> StoreResult<()> {
    let effective_at = event.source_timestamp.unwrap_or(event.observed_at);
    let local_day = effective_at
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .date_naive()
        .to_string();
    transaction.execute(
        "INSERT INTO daily_usage_rollups(
             local_day, thread_key, account_key, project_key, model_key, quality,
             event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
             cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
             total_tokens
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
         DO UPDATE SET
             event_count = event_count + 1,
             input_tokens = input_tokens + excluded.input_tokens,
             cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
             cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
             cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
             output_tokens = output_tokens + excluded.output_tokens,
             reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
             total_tokens = total_tokens + excluded.total_tokens",
        params![
            local_day,
            event.thread_id.as_deref().unwrap_or(""),
            event.account_fingerprint.as_deref().unwrap_or(""),
            event.project.project_id.as_deref().unwrap_or(""),
            event.model.as_deref().unwrap_or(""),
            quality_name(event.quality),
            sql_u64(event.usage.input_tokens, "input_tokens")?,
            sql_u64(event.usage.cached_input_tokens, "cached_input_tokens")?,
            sql_u64(
                event.usage.cache_write_input_tokens,
                "cache_write_input_tokens"
            )?,
            sql_u64(
                event.usage.cache_write_observed_input_tokens,
                "cache_write_observed_input_tokens"
            )?,
            sql_u64(event.usage.output_tokens, "output_tokens")?,
            sql_u64(
                event.usage.reasoning_output_tokens,
                "reasoning_output_tokens"
            )?,
            sql_u64(event.usage.total_tokens, "total_tokens")?,
        ],
    )?;
    Ok(())
}

fn upsert_thread_catalog_in(
    transaction: &rusqlite::Transaction<'_>,
    record: &ThreadCatalogRecord,
) -> StoreResult<()> {
    transaction.execute(
        r#"INSERT INTO thread_catalog(
               thread_id, parent_thread_id, project_id, project_name, title, model,
               agent_nickname, agent_role, agent_path, depth, created_at, updated_at,
               archived, has_user_event, source_kind, present_in_codex
           ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
           ) ON CONFLICT(thread_id) DO UPDATE SET
               parent_thread_id = COALESCE(excluded.parent_thread_id, thread_catalog.parent_thread_id),
               project_id = COALESCE(excluded.project_id, thread_catalog.project_id),
               project_name = COALESCE(excluded.project_name, thread_catalog.project_name),
               title = COALESCE(NULLIF(excluded.title, ''), thread_catalog.title),
               model = COALESCE(excluded.model, thread_catalog.model),
               agent_nickname = COALESCE(excluded.agent_nickname, thread_catalog.agent_nickname),
               agent_role = COALESCE(excluded.agent_role, thread_catalog.agent_role),
               agent_path = COALESCE(excluded.agent_path, thread_catalog.agent_path),
               depth = COALESCE(excluded.depth, thread_catalog.depth),
               created_at = MIN(thread_catalog.created_at, excluded.created_at),
               updated_at = MAX(thread_catalog.updated_at, excluded.updated_at),
               archived = CASE WHEN excluded.source_kind = 'state_5'
                               THEN excluded.archived ELSE thread_catalog.archived END,
               has_user_event = CASE WHEN excluded.source_kind = 'state_5'
                                     THEN excluded.has_user_event ELSE thread_catalog.has_user_event END,
               source_kind = CASE WHEN excluded.source_kind = 'state_5'
                                  THEN excluded.source_kind ELSE thread_catalog.source_kind END,
               present_in_codex = CASE WHEN excluded.source_kind = 'state_5'
                                       THEN 1 ELSE thread_catalog.present_in_codex END"#,
        params![
            record.thread_id,
            record.parent_thread_id,
            record.project_id,
            record.project_name,
            record.title,
            record.model,
            record.agent_nickname,
            record.agent_role,
            record.agent_path,
            record.depth.map(i64::from),
            timestamp(record.created_at),
            timestamp(record.updated_at),
            i64::from(record.archived),
            i64::from(record.has_user_event),
            record.source_kind,
            i64::from(record.source_kind == "state_5"),
        ],
    )?;
    Ok(())
}

fn advance_cursor_in(
    transaction: &rusqlite::Transaction<'_>,
    cursor: &FileCursor,
) -> StoreResult<()> {
    let existing: Option<(String, i64)> = transaction
        .query_row(
            "SELECT file_identity, byte_offset FROM file_cursors
             WHERE machine_id = ?1 AND source_id = ?2",
            params![cursor.machine_id, cursor.source_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((file_identity, old_offset)) = existing {
        let old_offset = u64_from_sql(old_offset, 1)?;
        if file_identity == cursor.file_identity && cursor.byte_offset < old_offset {
            return Err(StoreError::CursorRegression {
                machine_id: cursor.machine_id.clone(),
                source_id: cursor.source_id.clone(),
                old_offset,
                new_offset: cursor.byte_offset,
            });
        }
    }

    write_cursor_in(transaction, cursor)
}

fn local_hour_key(value: DateTime<Utc>) -> String {
    let local = value.with_timezone(&chrono_tz::Asia::Shanghai);
    format!(
        "{:04}-{:02}-{:02}T{:02}:00",
        local.year(),
        local.month(),
        local.day(),
        local.hour()
    )
}

fn floor_local_hour(value: DateTime<Utc>) -> String {
    local_hour_key(value)
}

fn ceil_local_hour(value: DateTime<Utc>) -> String {
    if value.minute() == 0 && value.second() == 0 && value.nanosecond() == 0 {
        local_hour_key(value)
    } else {
        local_hour_key(value + ChronoDuration::hours(1))
    }
}

fn floor_local_day(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .date_naive()
        .to_string()
}

fn ceil_local_day(value: DateTime<Utc>) -> String {
    let local = value.with_timezone(&chrono_tz::Asia::Shanghai);
    if local.hour() == 0 && local.minute() == 0 && local.second() == 0 && local.nanosecond() == 0 {
        local.date_naive().to_string()
    } else {
        (local.date_naive() + ChronoDuration::days(1)).to_string()
    }
}

fn write_cursor_in(
    transaction: &rusqlite::Transaction<'_>,
    cursor: &FileCursor,
) -> StoreResult<()> {
    transaction.execute(
        "INSERT INTO file_cursors(
             machine_id, source_id, file_identity, byte_offset, line_number,
             parser_state_json, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(machine_id, source_id) DO UPDATE SET
             file_identity = excluded.file_identity,
             byte_offset = excluded.byte_offset,
             line_number = excluded.line_number,
             parser_state_json = excluded.parser_state_json,
             updated_at = excluded.updated_at",
        params![
            cursor.machine_id,
            cursor.source_id,
            cursor.file_identity,
            sql_u64(cursor.byte_offset, "byte_offset")?,
            sql_u64(cursor.line_number, "line_number")?,
            cursor.parser_state_json,
            timestamp(cursor.updated_at),
        ],
    )?;
    Ok(())
}

fn uses_effective_source(filter: &AggregateFilter) -> bool {
    filter.quality == Some(DataQuality::Confirmed)
}

fn build_filter(filter: &AggregateFilter) -> (String, Vec<SqlValue>) {
    let mut predicates = Vec::<String>::new();
    let mut values = Vec::new();
    if let Some(start) = filter.start_inclusive {
        predicates.push("COALESCE(source_timestamp, observed_at) >= ?".to_owned());
        values.push(SqlValue::Text(timestamp(start)));
    }
    if let Some(end) = filter.end_exclusive {
        predicates.push("COALESCE(source_timestamp, observed_at) < ?".to_owned());
        values.push(SqlValue::Text(timestamp(end)));
    }
    if let Some(account) = &filter.account_fingerprint {
        predicates.push("account_fingerprint = ?".to_owned());
        values.push(SqlValue::Text(account.clone()));
    }
    if let Some(project_id) = &filter.project_id {
        push_classified_project_filter(
            &mut predicates,
            &mut values,
            project_id,
            "project_id",
            "usage_events.thread_id",
        );
    }
    if let Some(model) = &filter.model {
        predicates.push("model = ?".to_owned());
        values.push(SqlValue::Text(model.clone()));
    }
    if let Some(quality) = filter.quality {
        predicates.push("quality = ?".to_owned());
        values.push(SqlValue::Text(quality_name(quality).to_owned()));
    }
    let where_sql = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };
    (where_sql, values)
}

fn build_rollup_filter(filter: &AggregateFilter) -> (String, Vec<SqlValue>) {
    let mut predicates = Vec::<String>::new();
    let mut values = Vec::new();
    if let Some(start) = filter.start_inclusive {
        let day = start
            .with_timezone(&chrono_tz::Asia::Shanghai)
            .date_naive()
            .to_string();
        predicates.push("local_day >= ?".to_owned());
        values.push(SqlValue::Text(day));
    }
    if let Some(end) = filter.end_exclusive {
        let inclusive = end - ChronoDuration::nanoseconds(1);
        let day = inclusive
            .with_timezone(&chrono_tz::Asia::Shanghai)
            .date_naive()
            .to_string();
        predicates.push("local_day <= ?".to_owned());
        values.push(SqlValue::Text(day));
    }
    if let Some(account) = &filter.account_fingerprint {
        predicates.push("account_key = ?".to_owned());
        values.push(SqlValue::Text(account.clone()));
    }
    if let Some(project_id) = &filter.project_id {
        push_classified_project_filter(
            &mut predicates,
            &mut values,
            project_id,
            "project_key",
            "daily_usage_rollups.thread_key",
        );
    }
    if let Some(model) = &filter.model {
        predicates.push("model_key = ?".to_owned());
        values.push(SqlValue::Text(model.clone()));
    }
    if let Some(quality) = filter.quality {
        predicates.push("quality = ?".to_owned());
        values.push(SqlValue::Text(quality_name(quality).to_owned()));
    }
    let where_sql = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };
    (where_sql, values)
}

fn build_hourly_filter(filter: &AggregateFilter) -> (String, Vec<SqlValue>) {
    let mut predicates = Vec::<String>::new();
    let mut values = Vec::new();
    if let Some(start) = filter.start_inclusive {
        let local = start.with_timezone(&chrono_tz::Asia::Shanghai);
        predicates.push("local_hour >= ?".to_owned());
        values.push(SqlValue::Text(format!(
            "{:04}-{:02}-{:02}T{:02}:00",
            local.year(),
            local.month(),
            local.day(),
            local.hour()
        )));
    }
    if let Some(end) = filter.end_exclusive {
        let inclusive = end - ChronoDuration::nanoseconds(1);
        let local = inclusive.with_timezone(&chrono_tz::Asia::Shanghai);
        predicates.push("local_hour <= ?".to_owned());
        values.push(SqlValue::Text(format!(
            "{:04}-{:02}-{:02}T{:02}:00",
            local.year(),
            local.month(),
            local.day(),
            local.hour()
        )));
    }
    if let Some(account) = &filter.account_fingerprint {
        predicates.push("account_key = ?".to_owned());
        values.push(SqlValue::Text(account.clone()));
    }
    if let Some(project_id) = &filter.project_id {
        push_classified_project_filter(
            &mut predicates,
            &mut values,
            project_id,
            "project_key",
            "hourly_usage_rollups.thread_key",
        );
    }
    if let Some(model) = &filter.model {
        predicates.push("model_key = ?".to_owned());
        values.push(SqlValue::Text(model.clone()));
    }
    if let Some(quality) = filter.quality {
        predicates.push("quality = ?".to_owned());
        values.push(SqlValue::Text(quality_name(quality).to_owned()));
    }
    let where_sql = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };
    (where_sql, values)
}

fn classified_project_expression(project_column: &str, thread_column: &str) -> String {
    let standalone = standalone_thread_membership(thread_column);
    format!(
        "CASE WHEN {standalone} THEN '{STANDALONE_CONVERSATIONS_PROJECT_ID}' \
         WHEN COALESCE({project_column}, '') = '' THEN '{UNASSIGNED_PROJECT_ID}' \
         ELSE {project_column} END"
    )
}

fn push_classified_project_filter(
    predicates: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    project_id: &str,
    project_column: &str,
    thread_column: &str,
) {
    let standalone = standalone_thread_membership(thread_column);
    match project_id {
        STANDALONE_CONVERSATIONS_PROJECT_ID => predicates.push(standalone),
        UNASSIGNED_PROJECT_ID => predicates.push(format!(
            "COALESCE({project_column}, '') = '' AND NOT ({standalone})"
        )),
        _ => {
            predicates.push(format!("{project_column} = ? AND NOT ({standalone})"));
            values.push(SqlValue::Text(project_id.to_owned()));
        }
    }
}

fn standalone_thread_membership(thread_column: &str) -> String {
    format!(
        "EXISTS (
             SELECT 1 FROM standalone_thread_membership standalone
             WHERE standalone.thread_id = {thread_column}
         )"
    )
}

fn append_rollup_thread_filter(
    where_sql: &mut String,
    values: &mut Vec<SqlValue>,
    thread_ids: &[String],
) {
    let placeholders = std::iter::repeat_n("?", thread_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    if where_sql.is_empty() {
        *where_sql = format!("WHERE thread_key IN ({placeholders})");
    } else {
        where_sql.push_str(&format!(" AND thread_key IN ({placeholders})"));
    }
    values.extend(thread_ids.iter().cloned().map(SqlValue::Text));
}

fn aggregate_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsageAggregate> {
    Ok(UsageAggregate {
        event_count: u64_from_sql(row.get(0)?, 0)?,
        usage: TokenUsage {
            input_tokens: u64_from_sql(row.get(1)?, 1)?,
            cached_input_tokens: u64_from_sql(row.get(2)?, 2)?,
            cache_write_input_tokens: u64_from_sql(row.get(3)?, 3)?,
            cache_write_observed_input_tokens: u64_from_sql(row.get(4)?, 4)?,
            output_tokens: u64_from_sql(row.get(5)?, 5)?,
            reasoning_output_tokens: u64_from_sql(row.get(6)?, 6)?,
            total_tokens: u64_from_sql(row.get(7)?, 7)?,
        },
    })
}

fn checked_add_usage(total: &mut TokenUsage, next: TokenUsage) -> StoreResult<()> {
    total.input_tokens = total
        .input_tokens
        .checked_add(next.input_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.cached_input_tokens = total
        .cached_input_tokens
        .checked_add(next.cached_input_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.cache_write_input_tokens = total
        .cache_write_input_tokens
        .checked_add(next.cache_write_input_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.cache_write_observed_input_tokens = total
        .cache_write_observed_input_tokens
        .checked_add(next.cache_write_observed_input_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.output_tokens = total
        .output_tokens
        .checked_add(next.output_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.reasoning_output_tokens = total
        .reasoning_output_tokens
        .checked_add(next.reasoning_output_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    total.total_tokens = total
        .total_tokens
        .checked_add(next.total_tokens)
        .ok_or(StoreError::AggregateOverflow)?;
    Ok(())
}

fn stored_quota_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredQuotaSnapshot> {
    let observed_at: String = row.get(3)?;
    let normalized_json: String = row.get(4)?;
    let snapshot = serde_json::from_str(&normalized_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(StoredQuotaSnapshot {
        snapshot_id: row.get(0)?,
        account_fingerprint: row.get(1)?,
        auth_epoch: row.get(2)?,
        observed_at: parse_timestamp_column(observed_at, 3)?,
        snapshot,
    })
}

fn stored_official_usage_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredOfficialAccountUsage> {
    let observed_at: String = row.get(2)?;
    let normalized_json: String = row.get(3)?;
    let usage = serde_json::from_str(&normalized_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(StoredOfficialAccountUsage {
        snapshot_id: row.get(0)?,
        account_fingerprint: row.get(1)?,
        observed_at: parse_timestamp_column(observed_at, 2)?,
        usage,
    })
}

fn query_text_children(
    connection: &Connection,
    sql: &str,
    project_id: &str,
) -> StoreResult<Vec<String>> {
    let mut statement = connection.prepare(sql)?;
    let values = statement
        .query_map(params![project_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsageEvent> {
    let observed_at: String = row.get(1)?;
    let source_timestamp: Option<String> = row.get(2)?;
    let account_confidence: String = row.get(8)?;
    let project_confidence: String = row.get(11)?;
    let quality: String = row.get(20)?;
    Ok(UsageEvent {
        event_id: row.get(0)?,
        observed_at: parse_timestamp_column(observed_at, 1)?,
        source_timestamp: source_timestamp
            .map(|value| parse_timestamp_column(value, 2))
            .transpose()?,
        thread_id: row.get(3)?,
        parent_thread_id: row.get(4)?,
        model: row.get(5)?,
        cwd: row.get(6)?,
        account_fingerprint: row.get(7)?,
        account_confidence: parse_confidence_column(&account_confidence, 8)?,
        project: ProjectAttribution {
            project_id: row.get(9)?,
            project_name: row.get(10)?,
            confidence: parse_confidence_column(&project_confidence, 11)?,
            method: row.get(12)?,
        },
        usage: TokenUsage {
            input_tokens: u64_from_sql(row.get(13)?, 13)?,
            cached_input_tokens: u64_from_sql(row.get(14)?, 14)?,
            cache_write_input_tokens: u64_from_sql(row.get(15)?, 15)?,
            cache_write_observed_input_tokens: u64_from_sql(row.get(16)?, 16)?,
            output_tokens: u64_from_sql(row.get(17)?, 17)?,
            reasoning_output_tokens: u64_from_sql(row.get(18)?, 18)?,
            total_tokens: u64_from_sql(row.get(19)?, 19)?,
        },
        quality: parse_quality_column(&quality, 20)?,
        quality_reason: row.get(21)?,
        provenance: EventProvenance {
            machine_id: row.get(22)?,
            source_id: row.get(23)?,
            rollout_id: row.get(24)?,
            file_identity: row.get(25)?,
            byte_offset: u64_from_sql(row.get(26)?, 26)?,
            line_number: u64_from_sql(row.get(27)?, 27)?,
        },
    })
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn parse_timestamp_column(value: String, column: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn sql_u64(value: u64, field: &'static str) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::IntegerOverflow { field })
}

fn u64_from_sql(value: i64, column: usize) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn confidence_name(value: AttributionConfidence) -> &'static str {
    match value {
        AttributionConfidence::Verified => "verified",
        AttributionConfidence::Inferred => "inferred",
        AttributionConfidence::Unknown => "unknown",
    }
}

fn parse_confidence_column(value: &str, column: usize) -> rusqlite::Result<AttributionConfidence> {
    match value {
        "verified" => Ok(AttributionConfidence::Verified),
        "inferred" => Ok(AttributionConfidence::Inferred),
        "unknown" => Ok(AttributionConfidence::Unknown),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("unknown confidence {other:?}").into(),
        )),
    }
}

fn quality_name(value: DataQuality) -> &'static str {
    match value {
        DataQuality::Confirmed => "confirmed",
        DataQuality::Quarantined => "quarantined",
        DataQuality::Unknown => "unknown",
    }
}

fn reconstruction_status_name(value: ReconstructionStatus) -> &'static str {
    match value {
        ReconstructionStatus::Pending => "pending",
        ReconstructionStatus::Reconstructing => "reconstructing",
        ReconstructionStatus::Reconstructed => "reconstructed",
        ReconstructionStatus::Unrecoverable => "unrecoverable",
    }
}

fn reconstruction_status_from_name(
    value: &str,
    column: usize,
) -> rusqlite::Result<ReconstructionStatus> {
    match value {
        "pending" => Ok(ReconstructionStatus::Pending),
        "reconstructing" => Ok(ReconstructionStatus::Reconstructing),
        "reconstructed" => Ok(ReconstructionStatus::Reconstructed),
        "unrecoverable" => Ok(ReconstructionStatus::Unrecoverable),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("unknown reconstruction status {other:?}").into(),
        )),
    }
}

fn quota_source_name(value: QuotaSource) -> &'static str {
    match value {
        QuotaSource::WhamUsage => "wham_usage",
        QuotaSource::TokenCountEvent => "token_count_event",
    }
}

fn parse_quality_column(value: &str, column: usize) -> rusqlite::Result<DataQuality> {
    match value {
        "confirmed" => Ok(DataQuality::Confirmed),
        "quarantined" => Ok(DataQuality::Quarantined),
        "unknown" => Ok(DataQuality::Unknown),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("unknown quality {other:?}").into(),
        )),
    }
}

#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
