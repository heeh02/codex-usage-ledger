//! Incremental replay-safe reconstruction of retained Codex rollout JSONL.
//!
//! This source is intentionally isolated from the post-sampling ledger. The
//! store selects one complete source per `thread × local day`; callers must
//! never add reconstruction facts to sampling facts directly.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ingest::{
        DEFAULT_MAX_LINE_BYTES, IncrementalJsonlTailer, JsonlLine, TailCheckpoint, TailLimits,
        physical_file_identity,
    },
    replay::parse_token_sample,
    store::{
        BatchOutcome, FileCursor, LedgerStore, ReconstructionEvent, ReconstructionSourceStatus,
        ReconstructionStatus,
    },
    types::{
        AttributionConfidence, DataQuality, EventProvenance, ProjectAttribution, TokenUsage,
        UsageEvent,
    },
};

pub const RECONSTRUCTION_SOURCE_PREFIX: &str = "rollout-reconstruction-v1";
const STATE_SCHEMA_VERSION: u32 = 1;
const INHERITED_PREFIX_GAP_MILLIS: i64 = 2_000;
// Reconstruction must finish a complete JSONL record in one slice. Persisting
// an unfinished multi-megabyte record as a JSON byte array can use more than
// three times the source bytes and used to make a round-robin backfill grow the
// ledger by gigabytes. One extra byte leaves room for the trailing newline.
const RECONSTRUCTION_READ_CHUNK_BYTES: usize = DEFAULT_MAX_LINE_BYTES + 1;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconstructionReport {
    pub indexed_sources: u64,
    pub pending_sources: u64,
    pub files_advanced: u64,
    pub bytes_read: u64,
    pub inserted_events: u64,
    pub unchanged_events: u64,
    pub prefix_events: u64,
    pub counter_resets: u64,
    pub completed_sources: u64,
    pub unrecoverable_sources: u64,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone)]
struct Target {
    thread_id: String,
    parent_thread_id: Option<String>,
    path: PathBuf,
    cwd: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone)]
struct AccountEpoch {
    observed_from: DateTime<Utc>,
    observed_to: Option<DateTime<Utc>>,
    account_fingerprint: String,
    confidence: AttributionConfidence,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReconstructionPhase {
    #[default]
    AwaitingCanonical,
    ChildPrefix,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReconstructionCheckpoint {
    schema_version: u32,
    tail: TailCheckpoint,
    phase: ReconstructionPhase,
    canonical_at: Option<DateTime<Utc>>,
    previous_total: Option<TokenUsage>,
    last_token_at: Option<DateTime<Utc>>,
    model: Option<String>,
    cwd: Option<String>,
    counter_epoch: u64,
    prefix_events: u64,
    unchanged_events: u64,
    counter_resets: u64,
}

impl ReconstructionCheckpoint {
    fn new(target: &Target) -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            tail: TailCheckpoint::default(),
            phase: ReconstructionPhase::AwaitingCanonical,
            canonical_at: None,
            previous_total: None,
            last_token_at: None,
            model: target.model.clone(),
            cwd: target.cwd.clone(),
            counter_epoch: 0,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct TargetAttribution {
    project: ProjectAttribution,
    parent_thread_id: Option<String>,
}

/// Advances a bounded working set of retained rollout files. Each selected
/// file reads at most one complete-record-sized chunk, so history converges
/// without persisting large partial records across the whole corpus.
pub fn ingest_reconstruction_batch(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    max_files: usize,
) -> Result<ReconstructionReport> {
    ingest_reconstruction_batch_for_project(store, codex_home, machine_id, max_files, None)
}

pub fn ingest_reconstruction_batch_for_project(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    max_files: usize,
    project_id: Option<&str>,
) -> Result<ReconstructionReport> {
    // Older builds retained multi-megabyte parser checkpoints even after a
    // source became unrecoverable. The derived facts are independent rows, so
    // these unusable cursors can be reclaimed safely.
    store.remove_unrecoverable_reconstruction_cursors(machine_id)?;
    let mut targets = load_targets(codex_home)?;
    if let Some(project_id) = project_id {
        let mut statement = store
            .connection()
            .prepare("SELECT thread_id FROM thread_catalog WHERE project_id = ?1")?;
        let allowed = statement
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        targets.retain(|target| allowed.contains(&target.thread_id));
    }
    let target_by_source = targets
        .iter()
        .cloned()
        .map(|target| (source_id(&target.thread_id), target))
        .collect::<HashMap<_, _>>();
    let roots = [
        codex_home.join("sessions"),
        codex_home.join("archived_sessions"),
    ];
    let account_epochs = load_account_epochs(store, machine_id)?;
    let existing_sources = store.reconstruction_sources()?;
    let existing_by_source = existing_sources
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .map(|source| (source.source_id.clone(), source.clone()))
        .collect::<HashMap<_, _>>();
    let mut report = ReconstructionReport {
        indexed_sources: targets.len() as u64,
        ..ReconstructionReport::default()
    };

    let mut initial_statuses = Vec::new();
    let now = Utc::now();
    for target in &targets {
        let source_id = source_id(&target.thread_id);
        if existing_by_source.contains_key(&source_id) {
            continue;
        }
        let (file_identity, bytes_total, status, error) = match fs::metadata(&target.path) {
            Ok(metadata) => (
                physical_file_identity(&target.path, &metadata).unwrap_or_default(),
                metadata.len(),
                ReconstructionStatus::Pending,
                None,
            ),
            Err(error) => (
                String::new(),
                0,
                ReconstructionStatus::Unrecoverable,
                Some(error.to_string()),
            ),
        };
        initial_statuses.push(ReconstructionSourceStatus {
            machine_id: machine_id.to_owned(),
            source_id,
            thread_id: target.thread_id.clone(),
            file_identity,
            status,
            bytes_total,
            bytes_processed: 0,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
            last_error: error,
            updated_at: now,
        });
    }
    if !initial_statuses.is_empty() {
        store.upsert_reconstruction_sources(&initial_statuses)?;
    }

    let mut refreshed = store.reconstruction_sources()?;
    let mut replacements = Vec::new();
    for status in refreshed
        .iter()
        .filter(|source| source.machine_id == machine_id)
    {
        let Some(target) = target_by_source.get(&status.source_id) else {
            if project_id.is_none()
                && matches!(
                    status.status,
                    ReconstructionStatus::Pending | ReconstructionStatus::Reconstructing
                )
            {
                let mut missing = status.clone();
                missing.status = ReconstructionStatus::Unrecoverable;
                missing.last_error = Some("rollout is no longer indexed by Codex".to_owned());
                missing.updated_at = Utc::now();
                store.upsert_reconstruction_source(&missing)?;
                store.remove_cursor(machine_id, &missing.source_id)?;
            }
            continue;
        };
        let Ok(metadata) = fs::metadata(&target.path) else {
            continue;
        };
        let current_identity = physical_file_identity(&target.path, &metadata)?;
        if !status.file_identity.is_empty() && status.file_identity != current_identity {
            replacements.push(ReconstructionSourceStatus {
                machine_id: machine_id.to_owned(),
                source_id: status.source_id.clone(),
                thread_id: status.thread_id.clone(),
                file_identity: current_identity,
                status: ReconstructionStatus::Pending,
                bytes_total: metadata.len(),
                bytes_processed: 0,
                prefix_events: 0,
                unchanged_events: 0,
                counter_resets: 0,
                last_error: None,
                updated_at: Utc::now(),
            });
        }
    }
    store.replace_reconstruction_sources(&replacements)?;
    if !replacements.is_empty() {
        refreshed = store.reconstruction_sources()?;
    }
    let status_by_source = refreshed
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .map(|source| (source.source_id.clone(), source.clone()))
        .collect::<HashMap<_, _>>();
    report.pending_sources = refreshed
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .filter(|source| {
            matches!(
                source.status,
                ReconstructionStatus::Pending | ReconstructionStatus::Reconstructing
            )
        })
        .count() as u64;
    report.unrecoverable_sources = refreshed
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .filter(|source| source.status == ReconstructionStatus::Unrecoverable)
        .count() as u64;

    let mut queue = refreshed
        .into_iter()
        .filter(|source| source.machine_id == machine_id)
        .filter(|status| {
            matches!(
                status.status,
                ReconstructionStatus::Pending
                    | ReconstructionStatus::Reconstructing
                    | ReconstructionStatus::Reconstructed
            )
        })
        .filter_map(|status| {
            let target = target_by_source.get(&status.source_id)?.clone();
            let len = fs::metadata(&target.path).ok()?.len();
            (len != status.bytes_processed).then_some((status, target, len))
        })
        .collect::<Vec<_>>();
    // Finish a small working set before opening more large partial JSON lines.
    // Round-robin across every rollout would persist hundreds of multi-megabyte
    // partial-line checkpoints at once and inflate the ledger on disk.
    queue.sort_by(|(left, _, _), (right, _, _)| {
        let priority = |status: ReconstructionStatus| match status {
            ReconstructionStatus::Reconstructing => 0_u8,
            ReconstructionStatus::Reconstructed => 1_u8,
            ReconstructionStatus::Pending => 2_u8,
            ReconstructionStatus::Unrecoverable => 3_u8,
        };
        priority(left.status)
            .cmp(&priority(right.status))
            .then_with(|| right.bytes_processed.cmp(&left.bytes_processed))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });

    for (previous_status, target, file_len) in queue.into_iter().take(max_files.max(1)) {
        if !roots.iter().any(|root| target.path.starts_with(root)) {
            report.issues.push(format!(
                "refused rollout outside Codex roots: {}",
                target.path.display()
            ));
            continue;
        }
        match ingest_target(
            store,
            machine_id,
            &target,
            file_len,
            &previous_status,
            &account_epochs,
        ) {
            Ok((batch, status, bytes_read)) => {
                report.files_advanced = report.files_advanced.saturating_add(1);
                report.bytes_read = report.bytes_read.saturating_add(bytes_read);
                report.inserted_events =
                    report.inserted_events.saturating_add(batch.inserted as u64);
                report.unchanged_events = report
                    .unchanged_events
                    .saturating_add(status.unchanged_events);
                report.prefix_events = report.prefix_events.saturating_add(status.prefix_events);
                report.counter_resets = report.counter_resets.saturating_add(status.counter_resets);
                if status.status == ReconstructionStatus::Reconstructed {
                    report.completed_sources = report.completed_sources.saturating_add(1);
                }
            }
            Err(error) => {
                let mut status = status_by_source
                    .get(&source_id(&target.thread_id))
                    .cloned()
                    .unwrap_or(previous_status);
                status.status = ReconstructionStatus::Unrecoverable;
                status.last_error = Some(error.to_string());
                status.updated_at = Utc::now();
                store.upsert_reconstruction_source(&status)?;
                // A replaced/truncated source cannot resume from its old
                // parser state. Keep all derived facts, but discard the large
                // checkpoint blob so an unrecoverable file cannot consume
                // storage forever.
                store.remove_cursor(machine_id, &status.source_id)?;
                report.unrecoverable_sources = report.unrecoverable_sources.saturating_add(1);
                report.issues.push(format!("{}: {error}", target.thread_id));
            }
        }
    }
    let final_sources = store.reconstruction_sources()?;
    report.pending_sources = final_sources
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .filter(|source| {
            matches!(
                source.status,
                ReconstructionStatus::Pending | ReconstructionStatus::Reconstructing
            )
        })
        .count() as u64;
    report.unrecoverable_sources = final_sources
        .iter()
        .filter(|source| source.machine_id == machine_id)
        .filter(|source| source.status == ReconstructionStatus::Unrecoverable)
        .count() as u64;
    Ok(report)
}

fn ingest_target(
    store: &mut LedgerStore,
    machine_id: &str,
    target: &Target,
    _queued_file_len: u64,
    previous_status: &ReconstructionSourceStatus,
    account_epochs: &[AccountEpoch],
) -> Result<(BatchOutcome, ReconstructionSourceStatus, u64)> {
    let source_id = source_id(&target.thread_id);
    let metadata = fs::metadata(&target.path)?;
    let current_file_len = metadata.len();
    let file_identity = physical_file_identity(&target.path, &metadata)?;
    let existing_cursor = store.get_cursor(machine_id, &source_id)?;
    if let Some(cursor) = existing_cursor.as_ref()
        && cursor.file_identity != file_identity
    {
        return Err(anyhow!(
            "rollout file identity changed after reconstruction began"
        ));
    }
    let mut state = match existing_cursor
        .as_ref()
        .and_then(|cursor| cursor.parser_state_json.as_deref())
    {
        Some(encoded) => {
            let state: ReconstructionCheckpoint =
                serde_json::from_str(encoded).context("decode reconstruction checkpoint")?;
            if state.schema_version != STATE_SCHEMA_VERSION {
                return Err(anyhow!(
                    "unsupported reconstruction checkpoint version {}",
                    state.schema_version
                ));
            }
            state
        }
        None => ReconstructionCheckpoint::new(target),
    };
    let before = state.tail.next_offset;
    if !state.tail.partial_line.is_empty() {
        // Older builds persisted incomplete records after every 4 MiB slice.
        // Rewind only to the start of that record; no completed event is lost,
        // and the next larger slice replaces the oversized checkpoint.
        state.tail.next_offset = state.tail.partial_offset;
        state.tail.partial_line.clear();
    }
    let mut tailer = IncrementalJsonlTailer::with_limits(
        state.tail.clone(),
        TailLimits {
            read_chunk_bytes: RECONSTRUCTION_READ_CHUNK_BYTES,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        },
    )?;
    let batch = tailer
        .poll_path(&target.path)
        .with_context(|| format!("tail reconstruction {}", target.path.display()))?;
    if batch.reset.is_some() && existing_cursor.is_some() {
        return Err(anyhow!(
            "rollout truncated or replaced after reconstruction began"
        ));
    }

    let attribution = target_attribution(store, target)?;
    let mut events = Vec::new();
    for line in &batch.lines {
        if let Some(event) = process_line(
            &mut state,
            line,
            target,
            machine_id,
            &source_id,
            &file_identity,
            &attribution,
            account_epochs,
        )? {
            events.push(event);
        }
    }
    state.tail = batch.checkpoint.clone();
    let complete = state.tail.next_offset >= current_file_len && state.tail.partial_line.is_empty();
    let status = ReconstructionSourceStatus {
        machine_id: machine_id.to_owned(),
        source_id: source_id.clone(),
        thread_id: target.thread_id.clone(),
        file_identity: file_identity.clone(),
        status: if complete {
            ReconstructionStatus::Reconstructed
        } else {
            ReconstructionStatus::Reconstructing
        },
        bytes_total: current_file_len,
        bytes_processed: state.tail.next_offset,
        prefix_events: state.prefix_events,
        unchanged_events: state.unchanged_events,
        counter_resets: state.counter_resets,
        last_error: None,
        updated_at: Utc::now(),
    };
    let cursor = FileCursor {
        machine_id: machine_id.to_owned(),
        source_id,
        file_identity,
        byte_offset: state.tail.next_offset,
        line_number: state.tail.completed_lines,
        parser_state_json: Some(serde_json::to_string(&state)?),
        updated_at: status.updated_at,
    };
    let outcome = store.upsert_reconstruction_events_and_cursor(&events, &status, &cursor)?;
    let bytes_read = state.tail.next_offset.saturating_sub(before);
    let _ = previous_status;
    Ok((outcome, status, bytes_read))
}

#[allow(clippy::too_many_arguments)]
fn process_line(
    state: &mut ReconstructionCheckpoint,
    line: &JsonlLine,
    target: &Target,
    machine_id: &str,
    source_id: &str,
    file_identity: &str,
    attribution: &TargetAttribution,
    account_epochs: &[AccountEpoch],
) -> Result<Option<ReconstructionEvent>> {
    if line.is_blank() {
        return Ok(None);
    }
    let record = match line.parse_json() {
        Ok(record) => record,
        Err(_) => return Ok(None),
    };
    let kind = record
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind == "session_meta" && state.phase == ReconstructionPhase::AwaitingCanonical {
        let id = record
            .pointer("/payload/id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id != target.thread_id {
            return Ok(None);
        }
        state.canonical_at = source_timestamp(&record);
        state.cwd = record
            .pointer("/payload/cwd")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| state.cwd.clone());
        state.phase = if target.parent_thread_id.is_some() {
            ReconstructionPhase::ChildPrefix
        } else {
            ReconstructionPhase::Live
        };
        return Ok(None);
    }
    if kind == "turn_context" && state.phase == ReconstructionPhase::Live {
        state.model = record
            .pointer("/payload/model")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| state.model.clone());
        state.cwd = record
            .pointer("/payload/cwd")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| state.cwd.clone());
        return Ok(None);
    }
    if kind != "event_msg"
        || record.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
        || state.phase == ReconstructionPhase::AwaitingCanonical
    {
        return Ok(None);
    }
    let Some(at) = source_timestamp(&record) else {
        return Ok(None);
    };
    let sample = parse_token_sample(&record);
    let Some(total) = sample.total.filter(|usage| valid_usage(*usage)) else {
        return Ok(None);
    };

    if state.phase == ReconstructionPhase::ChildPrefix {
        let starts_immediately = state
            .canonical_at
            .is_some_and(|canonical| millis_between(canonical, at) <= INHERITED_PREFIX_GAP_MILLIS);
        match (state.previous_total, state.last_token_at) {
            (None, _) if !starts_immediately => state.phase = ReconstructionPhase::Live,
            (None, _) => {
                state.previous_total = Some(total);
                state.last_token_at = Some(at);
                state.prefix_events = state.prefix_events.saturating_add(1);
                return Ok(None);
            }
            (Some(_), Some(previous_at))
                if millis_between(previous_at, at) <= INHERITED_PREFIX_GAP_MILLIS =>
            {
                state.previous_total = Some(total);
                state.last_token_at = Some(at);
                state.prefix_events = state.prefix_events.saturating_add(1);
                return Ok(None);
            }
            _ => state.phase = ReconstructionPhase::Live,
        }
    }

    let (usage, reset) = match state.previous_total {
        None => (total, false),
        Some(previous) if total == previous => {
            state.last_token_at = Some(at);
            state.unchanged_events = state.unchanged_events.saturating_add(1);
            return Ok(None);
        }
        Some(previous) => match total.checked_delta(previous) {
            Some(delta) if valid_usage(delta) => (delta, false),
            _ => {
                let Some(last) = sample.last.filter(|usage| valid_usage(*usage)) else {
                    state.previous_total = Some(total);
                    state.last_token_at = Some(at);
                    state.counter_epoch = state.counter_epoch.saturating_add(1);
                    state.counter_resets = state.counter_resets.saturating_add(1);
                    return Ok(None);
                };
                (last, true)
            }
        },
    };
    state.previous_total = Some(total);
    state.last_token_at = Some(at);
    if reset {
        state.counter_epoch = state.counter_epoch.saturating_add(1);
        state.counter_resets = state.counter_resets.saturating_add(1);
    }
    if usage.is_zero() {
        return Ok(None);
    }
    let account = account_epoch_at(account_epochs, at);
    let event = UsageEvent {
        event_id: stable_event_id(
            machine_id,
            file_identity,
            &target.thread_id,
            line.byte_offset,
        ),
        observed_at: Utc::now(),
        source_timestamp: Some(at),
        thread_id: Some(target.thread_id.clone()),
        parent_thread_id: attribution.parent_thread_id.clone(),
        model: state.model.clone().or_else(|| target.model.clone()),
        cwd: state.cwd.clone().or_else(|| target.cwd.clone()),
        account_fingerprint: account.map(|epoch| epoch.account_fingerprint.clone()),
        account_confidence: account
            .map(|epoch| epoch.confidence)
            .unwrap_or(AttributionConfidence::Unknown),
        project: attribution.project.clone(),
        usage,
        quality: DataQuality::Confirmed,
        quality_reason: Some("rollout_reconstruction_selected_by_thread_day".to_owned()),
        provenance: EventProvenance {
            machine_id: machine_id.to_owned(),
            source_id: source_id.to_owned(),
            rollout_id: target.thread_id.clone(),
            file_identity: file_identity.to_owned(),
            byte_offset: line.byte_offset,
            line_number: line.line_number,
        },
    };
    Ok(Some(ReconstructionEvent {
        event,
        counter_epoch: state.counter_epoch,
    }))
}

fn load_targets(codex_home: &Path) -> Result<Vec<Target>> {
    let state_path = codex_home.join("state_5.sqlite");
    if !state_path.is_file() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        state_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;
    let mut statement = connection.prepare(
        "SELECT id, rollout_path, cwd, model, source FROM threads
         WHERE rollout_path IS NOT NULL AND rollout_path <> '' ORDER BY id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let (thread_id, rollout_path, cwd, model, source) = row?;
        if !seen.insert(thread_id.clone()) {
            continue;
        }
        let parent_thread_id = source
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .and_then(|value| {
                value
                    .pointer("/subagent/thread_spawn/parent_thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        targets.push(Target {
            thread_id,
            parent_thread_id,
            path: PathBuf::from(rollout_path),
            cwd,
            model,
        });
    }
    Ok(targets)
}

fn target_attribution(store: &LedgerStore, target: &Target) -> Result<TargetAttribution> {
    let row = store
        .connection()
        .query_row(
            "SELECT parent_thread_id, project_id, project_name
             FROM thread_catalog WHERE thread_id = ?1",
            [&target.thread_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let (catalog_parent, project_id, project_name) = row.unwrap_or((None, None, None));
    Ok(TargetAttribution {
        project: ProjectAttribution {
            confidence: if project_id.is_some() {
                AttributionConfidence::Inferred
            } else {
                AttributionConfidence::Unknown
            },
            method: if project_id.is_some() {
                "thread_catalog".to_owned()
            } else {
                "unassigned".to_owned()
            },
            project_id,
            project_name,
        },
        parent_thread_id: target.parent_thread_id.clone().or(catalog_parent),
    })
}

fn load_account_epochs(store: &LedgerStore, machine_id: &str) -> Result<Vec<AccountEpoch>> {
    let mut statement = store.connection().prepare(
        "SELECT observed_from, observed_to, account_fingerprint, confidence
         FROM auth_epochs
         WHERE machine_id = ?1 AND account_fingerprint IS NOT NULL
         ORDER BY observed_from",
    )?;
    let rows = statement.query_map([machine_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut epochs = Vec::new();
    for row in rows {
        let (from, to, account_fingerprint, confidence) = row?;
        epochs.push(AccountEpoch {
            observed_from: DateTime::parse_from_rfc3339(&from)?.with_timezone(&Utc),
            observed_to: to
                .map(|value| DateTime::parse_from_rfc3339(&value))
                .transpose()?
                .map(|value| value.with_timezone(&Utc)),
            account_fingerprint,
            confidence: match confidence.as_str() {
                "verified" => AttributionConfidence::Verified,
                "inferred" => AttributionConfidence::Inferred,
                _ => AttributionConfidence::Unknown,
            },
        });
    }
    Ok(epochs)
}

fn account_epoch_at(epochs: &[AccountEpoch], at: DateTime<Utc>) -> Option<&AccountEpoch> {
    epochs
        .iter()
        .filter(|epoch| {
            at >= epoch.observed_from
                && epoch.observed_to.is_none_or(|observed_to| at < observed_to)
        })
        .max_by_key(|epoch| {
            let confidence = match epoch.confidence {
                AttributionConfidence::Verified => 2_u8,
                AttributionConfidence::Inferred => 1_u8,
                AttributionConfidence::Unknown => 0_u8,
            };
            (confidence, epoch.observed_from)
        })
}

fn source_timestamp(record: &Value) -> Option<DateTime<Utc>> {
    record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn millis_between(earlier: DateTime<Utc>, later: DateTime<Utc>) -> i64 {
    later
        .signed_duration_since(earlier)
        .num_milliseconds()
        .max(0)
}

fn valid_usage(usage: TokenUsage) -> bool {
    usage.validate().is_ok()
}

fn source_id(thread_id: &str) -> String {
    format!("{RECONSTRUCTION_SOURCE_PREFIX}:{thread_id}")
}

fn stable_event_id(
    machine_id: &str,
    file_identity: &str,
    thread_id: &str,
    byte_offset: u64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(RECONSTRUCTION_SOURCE_PREFIX.as_bytes());
    digest.update([0]);
    digest.update(machine_id.as_bytes());
    digest.update([0]);
    digest.update(file_identity.as_bytes());
    digest.update([0]);
    digest.update(thread_id.as_bytes());
    digest.update([0]);
    digest.update(byte_offset.to_le_bytes());
    format!("reconstruction:{}", hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{AggregateFilter, LedgerStore};

    fn line(number: u64, raw: Value) -> JsonlLine {
        JsonlLine {
            byte_offset: number * 100,
            line_number: number,
            raw: serde_json::to_vec(&raw).unwrap(),
        }
    }

    fn token(at: &str, total: u64, last: u64) -> Value {
        serde_json::json!({
            "timestamp": at,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": total - 10,
                        "cached_input_tokens": total - 20,
                        "output_tokens": 10,
                        "reasoning_output_tokens": 4,
                        "total_tokens": total
                    },
                    "last_token_usage": {
                        "input_tokens": last - 10,
                        "cached_input_tokens": last - 20,
                        "output_tokens": 10,
                        "reasoning_output_tokens": 4,
                        "total_tokens": last
                    }
                }
            }
        })
    }

    fn child_target() -> Target {
        Target {
            thread_id: "child".to_owned(),
            parent_thread_id: Some("parent".to_owned()),
            path: PathBuf::from("child.jsonl"),
            cwd: Some("/tmp/project".to_owned()),
            model: Some("gpt-test".to_owned()),
        }
    }

    #[test]
    fn reconstruction_rejects_invalid_cache_write_coverage_deltas() {
        let mut usage = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 40,
            cache_write_input_tokens: 10,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        };
        assert!(valid_usage(usage));
        usage.cache_write_observed_input_tokens = 101;
        assert!(!valid_usage(usage));
    }

    #[test]
    fn child_dense_prefix_becomes_baseline_not_usage() {
        let target = child_target();
        let mut state = ReconstructionCheckpoint::new(&target);
        let attribution = TargetAttribution {
            project: ProjectAttribution {
                project_id: Some("p".to_owned()),
                project_name: Some("P".to_owned()),
                confidence: AttributionConfidence::Inferred,
                method: "test".to_owned(),
            },
            parent_thread_id: target.parent_thread_id.clone(),
        };
        let meta = line(
            1,
            serde_json::json!({
                "timestamp": "2026-09-01T00:00:00Z",
                "type": "session_meta",
                "payload": {"id": "child", "cwd": "/tmp/project"}
            }),
        );
        assert!(
            process_line(&mut state, &meta, &target, "m", "s", "f", &attribution, &[])
                .unwrap()
                .is_none()
        );
        for (number, value) in [
            (2, token("2026-09-01T00:00:00.100Z", 100, 100)),
            (3, token("2026-09-01T00:00:00.200Z", 200, 100)),
        ] {
            assert!(
                process_line(
                    &mut state,
                    &line(number, value),
                    &target,
                    "m",
                    "s",
                    "f",
                    &attribution,
                    &[]
                )
                .unwrap()
                .is_none()
            );
        }
        let live = process_line(
            &mut state,
            &line(4, token("2026-09-01T00:00:03Z", 250, 50)),
            &target,
            "m",
            "s",
            "f",
            &attribution,
            &[],
        )
        .unwrap()
        .unwrap();
        assert_eq!(state.prefix_events, 2);
        assert_eq!(live.event.usage.total_tokens, 50);
    }

    #[test]
    fn effective_view_selects_larger_source_without_adding_both() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let base = UsageEvent {
            event_id: "sampling".to_owned(),
            observed_at: DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            source_timestamp: Some(
                DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            thread_id: Some("thread".to_owned()),
            parent_thread_id: None,
            model: Some("gpt-test".to_owned()),
            cwd: None,
            account_fingerprint: None,
            account_confidence: AttributionConfidence::Unknown,
            project: ProjectAttribution {
                project_id: Some("p".to_owned()),
                project_name: Some("P".to_owned()),
                confidence: AttributionConfidence::Inferred,
                method: "test".to_owned(),
            },
            usage: TokenUsage {
                input_tokens: 90,
                cached_input_tokens: 70,
                output_tokens: 10,
                reasoning_output_tokens: 4,
                total_tokens: 100,
                ..TokenUsage::default()
            },
            quality: DataQuality::Confirmed,
            quality_reason: None,
            provenance: EventProvenance {
                machine_id: "m".to_owned(),
                source_id: "sampling".to_owned(),
                rollout_id: "thread".to_owned(),
                file_identity: "sf".to_owned(),
                byte_offset: 1,
                line_number: 1,
            },
        };
        store.upsert_event(&base).unwrap();
        let mut reconstructed = base;
        reconstructed.event_id = "reconstructed".to_owned();
        reconstructed.usage = TokenUsage {
            input_tokens: 190,
            cached_input_tokens: 170,
            output_tokens: 10,
            reasoning_output_tokens: 4,
            total_tokens: 200,
            ..TokenUsage::default()
        };
        reconstructed.provenance.source_id = source_id("thread");
        reconstructed.provenance.file_identity = "rf".to_owned();
        let status = ReconstructionSourceStatus {
            machine_id: "m".to_owned(),
            source_id: source_id("thread"),
            thread_id: "thread".to_owned(),
            file_identity: "rf".to_owned(),
            status: ReconstructionStatus::Reconstructed,
            bytes_total: 10,
            bytes_processed: 10,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
            last_error: None,
            updated_at: Utc::now(),
        };
        let cursor = FileCursor {
            machine_id: "m".to_owned(),
            source_id: source_id("thread"),
            file_identity: "rf".to_owned(),
            byte_offset: 10,
            line_number: 1,
            parser_state_json: Some("{}".to_owned()),
            updated_at: Utc::now(),
        };
        store
            .upsert_reconstruction_events_and_cursor(
                &[ReconstructionEvent {
                    event: reconstructed,
                    counter_epoch: 0,
                }],
                &status,
                &cursor,
            )
            .unwrap();
        let usage = store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap();
        assert_eq!(usage.usage.total_tokens, 200);

        let replacement = ReconstructionSourceStatus {
            file_identity: "rf-replaced".to_owned(),
            status: ReconstructionStatus::Pending,
            bytes_processed: 0,
            ..status
        };
        assert_eq!(
            store
                .replace_reconstruction_sources(&[replacement])
                .unwrap(),
            1
        );
        assert!(
            store
                .get_cursor("m", &source_id("thread"))
                .unwrap()
                .is_none()
        );
        let usage_after_replacement = store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap();
        assert_eq!(usage_after_replacement.usage.total_tokens, 100);
    }

    #[test]
    fn reconstruction_cursor_is_incremental_and_restart_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let sessions = temp.path().join("sessions/2026/09/01");
        fs::create_dir_all(&sessions).unwrap();
        let rollout = sessions.join("rollout-root.jsonl");
        let records = [
            serde_json::json!({
                "timestamp": "2026-09-01T00:00:00Z",
                "type": "session_meta",
                "payload": {"id": "root", "cwd": "/tmp/project"}
            }),
            token("2026-09-01T00:00:01Z", 100, 100),
            token("2026-09-01T00:00:02Z", 150, 50),
        ];
        let mut encoded = records
            .iter()
            .map(|value| serde_json::to_string(value).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        encoded.push('\n');
        fs::write(&rollout, &encoded).unwrap();

        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(
                     id TEXT PRIMARY KEY, rollout_path TEXT, cwd TEXT, model TEXT, source TEXT
                 );",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads(id, rollout_path, cwd, model, source)
                 VALUES ('root', ?1, '/tmp/project', 'gpt-test', '{}')",
                [rollout.to_string_lossy().as_ref()],
            )
            .unwrap();
        drop(index);

        let mut store = LedgerStore::open_in_memory().unwrap();
        let first = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(first.inserted_events, 2);
        assert_eq!(first.pending_sources, 0);
        assert_eq!(
            store
                .aggregate_rollup_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            150
        );

        let second = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(second.files_advanced, 0);
        assert_eq!(second.inserted_events, 0);

        encoded.push_str(&serde_json::to_string(&token("2026-09-01T00:00:03Z", 200, 50)).unwrap());
        encoded.push('\n');
        fs::write(&rollout, encoded).unwrap();
        let third = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(third.inserted_events, 1);
        assert_eq!(
            store
                .aggregate_rollup_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            200
        );
    }
}
