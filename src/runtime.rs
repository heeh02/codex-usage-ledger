use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    identity::{AuthIdentity, read_auth_identity},
    ingest::{IncrementalJsonlTailer, TailCheckpoint, TailReset, physical_file_identity},
    project::{ProjectRecord, ProjectResolutionInput, resolve_project},
    quota::normalize_quota_payload,
    replay::{ReplayCheckpoint, ReplayConfig, ReplayGuard},
    store::{BatchOutcome, CollectorStatus, FileCursor, LedgerStore, ThreadCatalogRecord},
    types::{AttributionConfidence, DataQuality, ProjectAttribution},
};

const DURABLE_FILE_STATE_VERSION: u32 = 1;
const QUOTA_TAIL_BOOTSTRAP_BYTES: u64 = 4 * 1024 * 1024;
const QUOTA_RECENT_THREAD_SECONDS: i64 = 12 * 60 * 60;
const QUOTA_RECENT_THREAD_LIMIT: i64 = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBinding {
    pub account_fingerprint: Option<String>,
    pub confidence: AttributionConfidence,
    pub auth_generation: Option<String>,
}

impl AccountBinding {
    pub fn from_identity(identity: &AuthIdentity) -> Self {
        Self {
            account_fingerprint: identity.account_fingerprint.clone(),
            confidence: identity.confidence,
            auth_generation: Some(identity.auth_epoch.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurableFileState {
    schema_version: u32,
    tail: TailCheckpoint,
    replay: ReplayCheckpoint,
    account: AccountBinding,
}

#[derive(Debug, Clone)]
struct QuotaAccountEpoch {
    observed_from: DateTime<Utc>,
    observed_to: Option<DateTime<Utc>>,
    account_fingerprint: String,
    generation: String,
    confidence: AttributionConfidence,
}

#[derive(Debug)]
struct QuotaLineCandidate {
    byte_offset: u64,
    observed_at: DateTime<Utc>,
    snapshot: crate::quota::QuotaSnapshot,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestReport {
    pub files_discovered: u64,
    pub files_advanced: u64,
    pub bytes_read: u64,
    pub inserted_events: u64,
    pub updated_events: u64,
    pub unchanged_events: u64,
    pub confirmed_events: u64,
    pub quarantined_events: u64,
    pub unknown_events: u64,
    pub quota_snapshots: u64,
    pub issues: Vec<IngestIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestIssue {
    pub source_id: String,
    pub message: String,
}

impl IngestReport {
    fn observe_batch(&mut self, batch: BatchOutcome) {
        self.inserted_events = self.inserted_events.saturating_add(batch.inserted as u64);
        self.updated_events = self.updated_events.saturating_add(batch.updated as u64);
        self.unchanged_events = self.unchanged_events.saturating_add(batch.unchanged as u64);
    }
}

#[derive(Debug, Clone, Default)]
struct NativeProjectIndex {
    projects: Vec<ProjectRecord>,
    thread_project: HashMap<String, String>,
    thread_git: HashMap<String, String>,
    thread_count: usize,
}

/// Refreshes the lightweight project/session directory from Codex state.
/// This reads `state_5.sqlite` only; it never scans rollout JSONL or changes
/// Codex-owned data, so dashboard-only mode can expose navigation safely.
pub fn sync_native_catalog(store: &mut LedgerStore, codex_home: &Path) -> Result<usize> {
    Ok(load_native_projects(codex_home, store, true)?.thread_count)
}

pub fn prepare_store(path: &Path) -> Result<LedgerStore> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create ledger data directory {}", parent.display()))?;
        set_private_permissions(parent, true)?;
    }
    let store = LedgerStore::open(path)
        .with_context(|| format!("open ledger database {}", path.display()))?;
    set_private_permissions(path, false)?;
    Ok(store)
}

pub fn load_or_create_machine_id(data_dir: &Path) -> Result<String> {
    let path = data_dir.join("machine-id");
    load_or_create_private_text(&path, || uuid::Uuid::new_v4().to_string())
}

pub fn load_or_create_hmac_key(data_dir: &Path) -> Result<Vec<u8>> {
    let path = data_dir.join("identity.key");
    let encoded = load_or_create_private_text(&path, || {
        let mut digest = Sha256::new();
        digest.update(uuid::Uuid::new_v4().as_bytes());
        digest.update(uuid::Uuid::new_v4().as_bytes());
        digest.update(
            Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
                .to_le_bytes(),
        );
        hex::encode(digest.finalize())
    })?;
    hex::decode(encoded.trim()).context("decode identity HMAC key")
}

pub fn observe_auth(
    store: &mut LedgerStore,
    auth_path: &Path,
    hmac_key: &[u8],
    machine_id: &str,
) -> Result<Option<AccountBinding>> {
    let auth_source = auth_source_id(auth_path.parent().unwrap_or_else(|| Path::new(".")));
    if !auth_path.is_file() {
        store
            .close_current_auth_epoch(machine_id, &auth_source, Utc::now())
            .context("close auth epoch after auth file removal")?;
        return Ok(None);
    }
    let identity = read_auth_identity(auth_path, hmac_key)
        .with_context(|| format!("read identity snapshot from {}", auth_path.display()))?;
    store
        .append_auth_epoch(machine_id, &auth_source, &identity, Utc::now())
        .context("append auth epoch")?;
    if let (Some(workspace), Some(account)) = (
        identity.workspace_fingerprint.as_deref(),
        identity.account_fingerprint.as_deref(),
    ) && let Some(previous) = store
        .upsert_workspace_account_alias(workspace, account, true, Utc::now())
        .context("link active workspace to account")?
    {
        store
            .remap_account_fingerprint(&previous, account)
            .context("merge provisional historical account")?;
    }
    Ok(Some(AccountBinding::from_identity(&identity)))
}

pub fn discover_rollouts(codex_home: &Path) -> Result<Vec<PathBuf>> {
    if let Ok(paths) = indexed_rollout_paths(codex_home)
        && !paths.is_empty()
    {
        return Ok(paths);
    }

    // Compatibility fallback for older Codex installations without the
    // session index. Current builds use state_5.sqlite above and never need a
    // recursive scan during normal operation.
    let mut paths = Vec::new();
    for source in ["sessions", "archived_sessions"] {
        let root = codex_home.join(source);
        if !root.exists() {
            continue;
        }
        let mut directories = vec![root];
        while let Some(directory) = directories.pop() {
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error).with_context(|| directory.display().to_string()),
            };
            for entry in entries {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    directories.push(entry.path());
                } else if file_type.is_file()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .strip_prefix("rollout-")
                        .is_some_and(|rest| rest.ends_with(".jsonl"))
                {
                    paths.push(entry.path());
                }
            }
        }
    }
    paths.sort();
    Ok(paths)
}

/// Returns only rollout files whose current length differs from their durable
/// cursor. The authoritative file list comes from Codex's own session index;
/// the ledger does not reopen and parse every historical rollout on startup.
pub fn discover_pending_rollouts(
    store: &LedgerStore,
    codex_home: &Path,
    machine_id: &str,
) -> Result<Vec<PathBuf>> {
    let paths = discover_rollouts(codex_home)?;
    let mut by_source = HashMap::<String, (String, u64)>::new();
    let mut by_identity = HashMap::<String, u64>::new();
    let mut statement = store.connection().prepare(
        "SELECT source_id, file_identity, byte_offset FROM file_cursors
         WHERE machine_id = ?1",
    )?;
    for row in statement.query_map([machine_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (source, identity, offset) = row?;
        let offset = u64::try_from(offset).unwrap_or_default();
        by_source.insert(source, (identity.clone(), offset));
        by_identity
            .entry(identity)
            .and_modify(|value| *value = (*value).max(offset))
            .or_insert(offset);
    }

    let mut pending = Vec::new();
    for path in paths {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                pending.push(path);
                continue;
            }
        };
        let identity = physical_file_identity(&path, &metadata).unwrap_or_default();
        let source = source_id(codex_home, &path);
        let offset = by_source
            .get(&source)
            .filter(|(cursor_identity, _)| cursor_identity == &identity)
            .map(|(_, offset)| *offset)
            .or_else(|| by_identity.get(&identity).copied());
        if offset != Some(metadata.len()) {
            pending.push(path);
        }
    }
    Ok(pending)
}

fn indexed_rollout_paths(codex_home: &Path) -> Result<Vec<PathBuf>> {
    let index = codex_home.join("state_5.sqlite");
    if !index.is_file() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        index,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;
    let roots = [
        codex_home.join("sessions"),
        codex_home.join("archived_sessions"),
    ];
    let mut statement = connection.prepare(
        "SELECT DISTINCT rollout_path FROM threads
         WHERE rollout_path IS NOT NULL AND rollout_path <> ''
         ORDER BY rollout_path",
    )?;
    let mut paths = BTreeSet::new();
    for path in statement.query_map([], |row| row.get::<_, String>(0))? {
        let path = PathBuf::from(path?);
        if roots.iter().any(|root| path.starts_with(root))
            && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        {
            paths.insert(path);
        }
    }
    Ok(paths.into_iter().collect())
}

fn indexed_recent_quota_rollouts(codex_home: &Path) -> Result<Vec<(String, PathBuf)>> {
    let index = codex_home.join("state_5.sqlite");
    if !index.is_file() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        index,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;
    let threshold = Utc::now()
        .timestamp()
        .saturating_sub(QUOTA_RECENT_THREAD_SECONDS);
    let roots = [
        codex_home.join("sessions"),
        codex_home.join("archived_sessions"),
    ];
    let thread_columns = connection
        .prepare("PRAGMA table_info(threads)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut root_predicate = String::new();
    if thread_columns.contains("thread_source") {
        root_predicate.push_str(" AND COALESCE(thread_source, '') <> 'subagent'");
    } else if thread_columns.contains("agent_path") {
        root_predicate.push_str(" AND agent_path IS NULL");
    }
    if thread_columns.contains("first_user_message") {
        root_predicate
            .push_str(" AND instr(COALESCE(first_user_message, ''), '<codex_delegation>') = 0");
    }
    let has_spawn_edges: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master
         WHERE type = 'table' AND name = 'thread_spawn_edges')",
        [],
        |row| row.get(0),
    )?;
    if has_spawn_edges {
        root_predicate.push_str(
            " AND NOT EXISTS(
                SELECT 1 FROM thread_spawn_edges edge
                WHERE edge.child_thread_id = threads.id
             )",
        );
    }
    let mut statement = connection.prepare(&format!(
        "SELECT threads.id, threads.rollout_path FROM threads
         WHERE rollout_path IS NOT NULL AND rollout_path <> '' AND updated_at >= ?1{root_predicate}
         ORDER BY updated_at DESC, id DESC LIMIT ?2"
    ))?;
    let rows = statement.query_map([threshold, QUOTA_RECENT_THREAD_LIMIT], |row| {
        Ok((
            row.get::<_, String>(0)?,
            PathBuf::from(row.get::<_, String>(1)?),
        ))
    })?;
    let mut paths = Vec::new();
    for row in rows {
        let (thread_id, path) = row?;
        if roots.iter().any(|root| path.starts_with(root))
            && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
            && path.is_file()
        {
            paths.push((thread_id, path));
        }
    }
    Ok(paths)
}

fn load_quota_account_epochs(
    store: &LedgerStore,
    machine_id: &str,
) -> Result<Vec<QuotaAccountEpoch>> {
    let mut statement = store.connection().prepare(
        "SELECT observed_from, observed_to, account_fingerprint, generation, confidence
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
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut epochs = Vec::new();
    for row in rows {
        let (from, to, account_fingerprint, generation, confidence) = row?;
        epochs.push(QuotaAccountEpoch {
            observed_from: DateTime::parse_from_rfc3339(&from)?.with_timezone(&Utc),
            observed_to: to
                .map(|value| DateTime::parse_from_rfc3339(&value))
                .transpose()?
                .map(|value| value.with_timezone(&Utc)),
            account_fingerprint,
            generation,
            confidence: match confidence.as_str() {
                "verified" => AttributionConfidence::Verified,
                "inferred" => AttributionConfidence::Inferred,
                _ => AttributionConfidence::Unknown,
            },
        });
    }
    Ok(epochs)
}

fn quota_account_epoch_at(
    epochs: &[QuotaAccountEpoch],
    observed_at: DateTime<Utc>,
) -> Option<&QuotaAccountEpoch> {
    epochs
        .iter()
        .filter(|epoch| {
            observed_at >= epoch.observed_from
                && epoch
                    .observed_to
                    .is_none_or(|observed_to| observed_at < observed_to)
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

fn read_quota_candidates(
    path: &Path,
    start_offset: u64,
) -> Result<(Vec<QuotaLineCandidate>, u64, u64)> {
    let mut file =
        fs::File::open(path).with_context(|| format!("open quota tail {}", path.display()))?;
    let file_len = file.metadata()?.len();
    let start_offset = start_offset.min(file_len);
    let needs_alignment = if start_offset > 0 {
        file.seek(SeekFrom::Start(start_offset - 1))?;
        let mut previous = [0_u8; 1];
        file.read_exact(&mut previous)?;
        previous[0] != b'\n'
    } else {
        false
    };
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::new(file);
    if needs_alignment {
        let mut partial = String::new();
        reader.read_line(&mut partial)?;
    }
    let aligned_start = reader.stream_position()?;
    let mut durable_offset = aligned_start;
    let mut candidates = Vec::new();
    let mut line = String::new();
    loop {
        let line_start = reader.stream_position()?;
        line.clear();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            break;
        }
        if !line.ends_with('\n') {
            durable_offset = line_start;
            break;
        }
        durable_offset = reader.stream_position()?;
        if !line.contains(r#""rate_limits""#) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(observed_at) = value
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        else {
            continue;
        };
        let Ok(snapshot) = normalize_quota_payload(&value) else {
            continue;
        };
        candidates.push(QuotaLineCandidate {
            byte_offset: line_start,
            observed_at,
            snapshot,
        });
    }
    Ok((
        candidates,
        durable_offset,
        durable_offset.saturating_sub(start_offset),
    ))
}

/// Incrementally captures only official `rate_limits` snapshots from recently
/// active Codex rollouts. It has its own cursor namespace, never emits Token
/// usage events, and bootstraps from a bounded tail rather than rescanning
/// historical rollout content.
pub fn ingest_quota_tails(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
) -> Result<IngestReport> {
    let rollouts = indexed_recent_quota_rollouts(codex_home)?;
    let epochs = load_quota_account_epochs(store, machine_id)?;
    let mut report = IngestReport {
        files_discovered: rollouts.len() as u64,
        ..IngestReport::default()
    };
    for (thread_id, path) in rollouts {
        let source_id = format!("quota-rollout:{thread_id}");
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                report.issues.push(IngestIssue {
                    source_id,
                    message: error.to_string(),
                });
                continue;
            }
        };
        let file_identity = physical_file_identity(&path, &metadata)?;
        let existing = store.get_cursor(machine_id, &source_id)?;
        let cursor_matches = existing
            .as_ref()
            .is_some_and(|cursor| cursor.file_identity == file_identity);
        let start_offset = if cursor_matches {
            existing
                .as_ref()
                .map(|cursor| cursor.byte_offset)
                .unwrap_or_default()
        } else {
            metadata.len().saturating_sub(QUOTA_TAIL_BOOTSTRAP_BYTES)
        };
        let (candidates, durable_offset, bytes_read) =
            match read_quota_candidates(&path, start_offset) {
                Ok(value) => value,
                Err(error) => {
                    report.issues.push(IngestIssue {
                        source_id,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
        report.bytes_read = report.bytes_read.saturating_add(bytes_read);
        let mut safe_offset = durable_offset;
        for candidate in candidates {
            let Some(epoch) = quota_account_epoch_at(&epochs, candidate.observed_at) else {
                safe_offset = safe_offset.min(candidate.byte_offset);
                break;
            };
            let unchanged = store
                .latest_quota_snapshot(&epoch.account_fingerprint)?
                .is_some_and(|latest| latest.snapshot == candidate.snapshot);
            if !unchanged {
                store.append_quota_snapshot(
                    &epoch.account_fingerprint,
                    &epoch.generation,
                    candidate.observed_at,
                    &candidate.snapshot,
                )?;
                report.quota_snapshots = report.quota_snapshots.saturating_add(1);
            }
        }
        let next_cursor = FileCursor {
            machine_id: machine_id.to_owned(),
            source_id,
            file_identity,
            byte_offset: safe_offset,
            line_number: safe_offset,
            parser_state_json: Some(r#"{"source":"quota_tail","version":1}"#.to_owned()),
            updated_at: Utc::now(),
        };
        let cursor_changed = existing.as_ref().is_none_or(|cursor| {
            cursor.file_identity != next_cursor.file_identity
                || cursor.byte_offset != next_cursor.byte_offset
        });
        if cursor_changed {
            if cursor_matches {
                store.advance_cursor(&next_cursor)?;
            } else {
                store.reset_cursor(&next_cursor)?;
            }
            report.files_advanced = report.files_advanced.saturating_add(1);
        }
    }
    Ok(report)
}

pub fn ingest_all(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    binding_for_new_files: Option<&AccountBinding>,
) -> Result<IngestReport> {
    let paths = discover_pending_rollouts(store, codex_home, machine_id)?;
    ingest_paths(store, codex_home, machine_id, &paths, binding_for_new_files)
}

pub fn ingest_paths(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    paths: &[PathBuf],
    binding_for_new_files: Option<&AccountBinding>,
) -> Result<IngestReport> {
    let native = load_native_projects(codex_home, store, false).unwrap_or_default();
    let tracked_status = store
        .collector_status()
        .ok()
        .filter(|status| matches!(status.phase.as_str(), "syncing" | "backfill"));
    let mut report = IngestReport {
        files_discovered: paths.len() as u64,
        ..IngestReport::default()
    };

    for (index, path) in paths.iter().enumerate() {
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        let source_id = source_id(codex_home, path);
        if let Err(error) = ingest_one_file(
            store,
            codex_home,
            machine_id,
            &source_id,
            path,
            binding_for_new_files,
            &native,
            &mut report,
        ) {
            report.issues.push(IngestIssue {
                source_id,
                message: error.to_string(),
            });
        }
        if let Some(status) = tracked_status.as_ref()
            && ((index + 1) % 5 == 0 || index + 1 == paths.len())
        {
            store.set_collector_status(&CollectorStatus {
                mode: status.mode.clone(),
                phase: status.phase.clone(),
                items_total: status.items_total.max(paths.len() as u64),
                items_completed: status
                    .items_completed
                    .saturating_add((index + 1) as u64)
                    .min(status.items_total.max(paths.len() as u64)),
                bytes_read: report.bytes_read,
                events_inserted: report.inserted_events,
                message: status.message.clone(),
                updated_at: Utc::now(),
            })?;
        }
    }
    Ok(report)
}

pub fn prepare_fast_ledger(store: &mut LedgerStore, mode: &str) -> Result<()> {
    let initial = store.rollup_progress()?;
    if !initial.complete {
        store.set_collector_status(&CollectorStatus {
            mode: mode.to_owned(),
            phase: "optimizing".to_owned(),
            items_total: initial.target_rowid,
            items_completed: initial.last_backfilled_rowid,
            bytes_read: 0,
            events_inserted: 0,
            message: Some("正在建立一次性历史日汇总".to_owned()),
            updated_at: Utc::now(),
        })?;
        let mut chunks = 0_u64;
        loop {
            let progress = store.backfill_rollup_chunk(50_000)?;
            chunks = chunks.saturating_add(1);
            store.set_collector_status(&CollectorStatus {
                mode: mode.to_owned(),
                phase: "optimizing".to_owned(),
                items_total: progress.target_rowid,
                items_completed: progress.last_backfilled_rowid,
                bytes_read: 0,
                events_inserted: 0,
                message: Some("正在建立一次性历史日汇总".to_owned()),
                updated_at: Utc::now(),
            })?;
            if progress.complete {
                break;
            }
            if chunks.is_multiple_of(10) {
                store.checkpoint_wal()?;
            }
        }
        store.checkpoint_wal()?;
    }
    store.verify_rollup_before_compaction()?;
    Ok(())
}

pub fn compact_expired_raw_events(store: &mut LedgerStore, mode: &str) -> Result<u64> {
    let retain_since = Utc::now() - chrono::Duration::days(crate::store::RAW_EVENT_RETENTION_DAYS);
    let total: i64 = store.connection().query_row(
        "SELECT COUNT(*) FROM usage_events
         WHERE COALESCE(source_timestamp, observed_at) < ?1",
        [retain_since.to_rfc3339()],
        |row| row.get(0),
    )?;
    let total = total.max(0) as u64;
    if total == 0 {
        return Ok(0);
    }
    store.set_collector_status(&CollectorStatus {
        mode: mode.to_owned(),
        phase: "compacting".to_owned(),
        items_total: total,
        items_completed: 0,
        bytes_read: 0,
        events_inserted: 0,
        message: Some(format!(
            "正在压缩 {} 天以前的冗余事件明细",
            crate::store::RAW_EVENT_RETENTION_DAYS
        )),
        updated_at: Utc::now(),
    })?;
    let mut deleted = 0_u64;
    loop {
        let count = store.compact_raw_events_chunk(retain_since, 50_000)? as u64;
        if count == 0 {
            break;
        }
        deleted = deleted.saturating_add(count);
        store.set_collector_status(&CollectorStatus {
            mode: mode.to_owned(),
            phase: "compacting".to_owned(),
            items_total: total,
            items_completed: deleted.min(total),
            bytes_read: 0,
            events_inserted: 0,
            message: Some(format!(
                "正在压缩 {} 天以前的冗余事件明细",
                crate::store::RAW_EVENT_RETENTION_DAYS
            )),
            updated_at: Utc::now(),
        })?;
        store.checkpoint_wal()?;
    }
    Ok(deleted)
}

#[allow(clippy::too_many_arguments)]
fn ingest_one_file(
    store: &mut LedgerStore,
    _codex_home: &Path,
    machine_id: &str,
    source_id: &str,
    path: &Path,
    binding_for_new_files: Option<&AccountBinding>,
    native: &NativeProjectIndex,
    report: &mut IngestReport,
) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let physical_identity = physical_file_identity(path, &metadata)?;
    let cursor = match store.get_cursor(machine_id, source_id)? {
        Some(cursor) => Some(cursor),
        None => store.get_cursor_by_file_identity(machine_id, &physical_identity)?,
    };

    let (mut tailer, mut guard, account) = if let Some(cursor) = cursor.as_ref() {
        let encoded = cursor
            .parser_state_json
            .as_deref()
            .ok_or_else(|| anyhow!("cursor has no durable parser checkpoint"))?;
        let state: DurableFileState =
            serde_json::from_str(encoded).context("decode durable parser checkpoint")?;
        if state.schema_version != DURABLE_FILE_STATE_VERSION {
            bail!(
                "unsupported durable parser checkpoint version {}",
                state.schema_version
            );
        }
        let config = replay_config(machine_id, source_id, &physical_identity, &state.account);
        let guard = ReplayGuard::from_checkpoint(config, state.replay)
            .context("restore replay checkpoint")?;
        (
            IncrementalJsonlTailer::from_checkpoint(state.tail),
            guard,
            state.account,
        )
    } else {
        let account = binding_for_new_files.cloned().unwrap_or(AccountBinding {
            account_fingerprint: None,
            confidence: AttributionConfidence::Unknown,
            auth_generation: None,
        });
        let config = replay_config(machine_id, source_id, &physical_identity, &account);
        (
            IncrementalJsonlTailer::new(),
            ReplayGuard::new(config),
            account,
        )
    };

    loop {
        let before = tailer.checkpoint().next_offset;
        let batch = tailer
            .poll_path(path)
            .with_context(|| format!("tail {}", path.display()))?;
        if batch.reset.is_some() {
            if cursor.is_some() {
                bail!(
                    "file reset detected ({:?}); run doctor/rebuild before replacing its audit history",
                    batch.reset
                );
            }
            let file_identity = batch
                .checkpoint
                .file_identity
                .clone()
                .unwrap_or_else(|| physical_identity.clone());
            guard = ReplayGuard::new(replay_config(
                machine_id,
                source_id,
                &file_identity,
                &account,
            ));
        }

        let mut events = Vec::new();
        let mut quota_candidates = Vec::new();
        for line in &batch.lines {
            let parsed = line.parse_json().ok();
            let outcome = guard.process_line(line, Utc::now());
            if account.account_fingerprint.is_some()
                && account.auth_generation.is_some()
                && !matches!(
                    guard.phase(),
                    crate::replay::ReplayPhase::ReplayingForeignHistory
                )
                && guard.canonical().is_some()
                && let Some(value) = parsed.as_ref()
                && let Ok(snapshot) = normalize_quota_payload(value)
            {
                let source_timestamp = value
                    .get("timestamp")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);
                quota_candidates.push((source_timestamp, snapshot));
            }
            if let Some(mut event) = outcome.event {
                event.project = resolve_event_project(store, native, &event)?;
                match event.quality {
                    DataQuality::Confirmed => report.confirmed_events += 1,
                    DataQuality::Quarantined => report.quarantined_events += 1,
                    DataQuality::Unknown => report.unknown_events += 1,
                }
                events.push(event);
            }
        }

        let state = DurableFileState {
            schema_version: DURABLE_FILE_STATE_VERSION,
            tail: batch.checkpoint.clone(),
            replay: guard.checkpoint(),
            account: account.clone(),
        };
        let file_identity = batch
            .checkpoint
            .file_identity
            .clone()
            .unwrap_or_else(|| physical_identity.clone());
        let next_cursor = FileCursor {
            machine_id: machine_id.to_owned(),
            source_id: source_id.to_owned(),
            file_identity,
            byte_offset: batch.checkpoint.next_offset,
            line_number: batch.checkpoint.completed_lines,
            parser_state_json: Some(serde_json::to_string(&state)?),
            updated_at: Utc::now(),
        };
        let outcome = store.upsert_events_and_cursor(&events, &next_cursor)?;
        report.observe_batch(outcome);
        report.bytes_read = report
            .bytes_read
            .saturating_add(batch.checkpoint.next_offset.saturating_sub(before));
        if batch.checkpoint.next_offset > before {
            report.files_advanced += 1;
        }

        if let (Some(account_fingerprint), Some(auth_generation)) = (
            account.account_fingerprint.as_deref(),
            account.auth_generation.as_deref(),
        ) {
            for (observed_at, snapshot) in quota_candidates {
                let unchanged = store
                    .latest_quota_snapshot(account_fingerprint)?
                    .is_some_and(|latest| latest.snapshot == snapshot);
                if !unchanged {
                    store.append_quota_snapshot(
                        account_fingerprint,
                        auth_generation,
                        observed_at,
                        &snapshot,
                    )?;
                    report.quota_snapshots += 1;
                }
            }
        }

        let current_len = fs::metadata(path)?.len();
        if batch.checkpoint.next_offset >= current_len || batch.checkpoint.next_offset == before {
            break;
        }
    }
    Ok(())
}

fn replay_config(
    machine_id: &str,
    source_id: &str,
    file_identity: &str,
    account: &AccountBinding,
) -> ReplayConfig {
    let mut config = ReplayConfig::new(machine_id, source_id, file_identity);
    config.account_fingerprint = account.account_fingerprint.clone();
    config.account_confidence = account.confidence;
    config
}

fn resolve_event_project(
    store: &LedgerStore,
    native: &NativeProjectIndex,
    event: &crate::types::UsageEvent,
) -> Result<ProjectAttribution> {
    let thread_id = event.thread_id.as_deref();
    let manual = thread_id
        .map(|thread_id| store.get_manual_assignment(thread_id))
        .transpose()?
        .flatten();
    let native_project_id = thread_id.and_then(|id| native.thread_project.get(id));
    let git_identity = thread_id.and_then(|id| native.thread_git.get(id));
    Ok(resolve_project(
        ProjectResolutionInput {
            manual: manual.as_ref().map(|value| &value.assignment),
            native_project_id: native_project_id.map(String::as_str),
            cwd: event.cwd.as_deref().map(Path::new),
            git_identity: git_identity.map(String::as_str),
            parent: None,
        },
        &native.projects,
    ))
}

fn load_native_projects(
    codex_home: &Path,
    store: &mut LedgerStore,
    sync_catalog: bool,
) -> Result<NativeProjectIndex> {
    let path = codex_home.join("state_5.sqlite");
    if !path.is_file() {
        return Ok(NativeProjectIndex::default());
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;

    // Codex currently leaves threads.project_id empty for many saved projects.
    // Learn a project's repository identity from threads whose cwd is already
    // inside one of its configured roots; that identity can then recognize
    // Codex worktrees of the same repository.
    let mut root_git_evidence = Vec::<(PathBuf, String)>::new();
    let mut root_git_statement = connection.prepare(
        "SELECT DISTINCT cwd, git_origin_url FROM threads
         WHERE git_origin_url IS NOT NULL AND git_origin_url <> ''",
    )?;
    for row in root_git_statement.query_map([], |row| {
        Ok((
            PathBuf::from(row.get::<_, String>(0)?),
            row.get::<_, String>(1)?,
        ))
    })? {
        root_git_evidence.push(row?);
    }

    let mut projects = Vec::new();
    let mut project_statement = connection.prepare("SELECT id, name FROM projects")?;
    let base = project_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (project_id, project_name) in base {
        let roots = query_paths(
            &connection,
            "SELECT path FROM project_roots WHERE project_id = ?1 ORDER BY position",
            &project_id,
        )?;
        let mut git_identities = query_strings(
            &connection,
            "SELECT DISTINCT git_origin_url FROM threads
             WHERE project_id = ?1 AND git_origin_url IS NOT NULL AND git_origin_url <> ''",
            &project_id,
        )?;
        git_identities = merge_root_git_evidence(&roots, git_identities, &root_git_evidence);
        let project = ProjectRecord {
            project_id,
            project_name,
            roots,
            git_identities,
        };
        store.upsert_project(&project, Utc::now())?;
        projects.push(project);
    }

    let mut thread_project = HashMap::new();
    let mut thread_git = HashMap::new();
    let thread_columns = connection
        .prepare("PRAGMA table_info(threads)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    let optional_column = |name: &'static str| {
        if thread_columns.contains(name) {
            name.to_owned()
        } else {
            format!("NULL AS {name}")
        }
    };
    if !sync_catalog {
        let project_id = optional_column("project_id");
        let project_predicate = if thread_columns.contains("project_id") {
            " OR project_id IS NOT NULL"
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT id, {project_id}, git_origin_url FROM threads
             WHERE git_origin_url IS NOT NULL{project_predicate}",
        ))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (thread_id, project_id, git_origin) = row?;
            if let Some(project_id) = project_id {
                thread_project.insert(thread_id.clone(), project_id);
            }
            if let Some(git_origin) = git_origin.filter(|value| !value.is_empty()) {
                thread_git.insert(thread_id, git_origin);
            }
        }
        return Ok(NativeProjectIndex {
            projects,
            thread_project,
            thread_git,
            thread_count: 0,
        });
    }

    let mut catalog = Vec::new();
    let thread_sql = format!(
        "SELECT id, cwd, title, {}, {}, {}, {}, {}, {}, {}, created_at, updated_at,
                archived, has_user_event, source, {}, {}, git_origin_url
         FROM threads",
        optional_column("name"),
        optional_column("model"),
        optional_column("agent_nickname"),
        optional_column("agent_role"),
        optional_column("agent_path"),
        optional_column("created_at_ms"),
        optional_column("updated_at_ms"),
        optional_column("thread_source"),
        optional_column("project_id"),
    );
    let mut thread_statement = connection.prepare(&thread_sql)?;
    let rows = thread_statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<i64>>(9)?,
            row.get::<_, i64>(10)?,
            row.get::<_, i64>(11)?,
            row.get::<_, bool>(12)?,
            row.get::<_, bool>(13)?,
            row.get::<_, String>(14)?,
            row.get::<_, Option<String>>(15)?,
            row.get::<_, Option<String>>(16)?,
            row.get::<_, Option<String>>(17)?,
        ))
    })?;
    for row in rows {
        let (
            thread_id,
            cwd,
            title,
            name,
            model,
            column_nickname,
            column_role,
            column_agent_path,
            created_at_ms,
            updated_at_ms,
            created_at,
            updated_at,
            archived,
            has_user_event,
            source,
            thread_source,
            native_project_id,
            git_origin,
        ) = row?;
        let source_json = serde_json::from_str::<serde_json::Value>(&source).ok();
        let spawn = source_json
            .as_ref()
            .and_then(|value| value.pointer("/subagent/thread_spawn"));
        let parent_thread_id = spawn
            .and_then(|value| value.get("parent_thread_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let depth = spawn
            .and_then(|value| value.get("depth"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .or_else(|| {
                (thread_source.as_deref() == Some("subagent") || spawn.is_some()).then_some(1)
            })
            .or(Some(0));
        let agent_nickname = spawn
            .and_then(|value| value.get("agent_nickname"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or(column_nickname);
        let agent_role = spawn
            .and_then(|value| value.get("agent_role"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or(column_role);
        let agent_path = spawn
            .and_then(|value| value.get("agent_path"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or(column_agent_path);
        let manual = store
            .get_manual_assignment(&thread_id)?
            .map(|value| value.assignment);
        let project = resolve_project(
            ProjectResolutionInput {
                manual: manual.as_ref(),
                native_project_id: native_project_id.as_deref(),
                cwd: Some(Path::new(&cwd)),
                git_identity: git_origin.as_deref(),
                parent: None,
            },
            &projects,
        );
        if let Some(project_id) = project.project_id.as_ref() {
            thread_project.insert(thread_id.clone(), project_id.clone());
        }
        if let Some(git_origin) = git_origin.as_ref().filter(|value| !value.is_empty()) {
            thread_git.insert(thread_id.clone(), git_origin.clone());
        }
        let created_at = DateTime::<Utc>::from_timestamp_millis(
            created_at_ms.unwrap_or(created_at.saturating_mul(1_000)),
        )
        .unwrap_or_else(Utc::now);
        let updated_at = DateTime::<Utc>::from_timestamp_millis(
            updated_at_ms.unwrap_or(updated_at.saturating_mul(1_000)),
        )
        .unwrap_or(created_at);
        let display_title = name
            .filter(|value| !value.trim().is_empty())
            .or_else(|| (!title.trim().is_empty()).then_some(title));
        catalog.push(ThreadCatalogRecord {
            thread_id,
            parent_thread_id,
            project_id: project.project_id,
            project_name: project.project_name,
            title: display_title,
            model,
            agent_nickname,
            agent_role,
            agent_path,
            depth,
            created_at,
            updated_at,
            archived,
            has_user_event,
            source_kind: "state_5".to_owned(),
        });
    }
    store.sync_native_thread_catalog_batch(&catalog)?;

    Ok(NativeProjectIndex {
        projects,
        thread_project,
        thread_git,
        thread_count: catalog.len(),
    })
}

fn query_paths(connection: &Connection, sql: &str, id: &str) -> Result<Vec<PathBuf>> {
    Ok(query_strings(connection, sql, id)?
        .into_iter()
        .map(PathBuf::from)
        .collect())
}

fn query_strings(connection: &Connection, sql: &str, id: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement
        .query_map([id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn merge_root_git_evidence(
    roots: &[PathBuf],
    mut explicit: Vec<String>,
    evidence: &[(PathBuf, String)],
) -> Vec<String> {
    for (cwd, git_identity) in evidence {
        if roots.iter().any(|root| cwd.starts_with(root)) {
            explicit.push(git_identity.clone());
        }
    }
    explicit.sort();
    explicit.dedup();
    explicit
}

fn source_id(codex_home: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(codex_home).unwrap_or(path);
    let mut digest = Sha256::new();
    digest.update(
        codex_home
            .canonicalize()
            .unwrap_or_else(|_| codex_home.to_path_buf())
            .to_string_lossy()
            .as_bytes(),
    );
    digest.update([0]);
    digest.update(relative.to_string_lossy().as_bytes());
    format!("rollout:{}", &hex::encode(digest.finalize())[..24])
}

fn auth_source_id(codex_home: &Path) -> String {
    let canonical = codex_home
        .canonicalize()
        .unwrap_or_else(|_| codex_home.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    format!("auth:{}", &hex::encode(digest)[..24])
}

fn load_or_create_private_text(path: &Path, create: impl FnOnce() -> String) -> Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_private_permissions(parent, true)?;
    }
    if path.is_file() {
        let mut value = String::new();
        fs::File::open(path)?.read_to_string(&mut value)?;
        return Ok(value.trim().to_owned());
    }

    let value = create();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(value.as_bytes())?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let mut stored = String::new();
    fs::File::open(path)?.read_to_string(&mut stored)?;
    Ok(stored.trim().to_owned())
}

fn set_private_permissions(_path: &Path, directory: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if directory { 0o700 } else { 0o600 };
        fs::set_permissions(_path, fs::Permissions::from_mode(mode))?;
    }
    let _ = directory;
    Ok(())
}

pub fn account_labels(store: &LedgerStore) -> Result<BTreeSet<String>> {
    let mut labels = BTreeSet::new();
    let mut statement = store.connection().prepare(
        "SELECT DISTINCT account_fingerprint FROM usage_events
         WHERE account_fingerprint IS NOT NULL
         UNION SELECT DISTINCT account_fingerprint FROM auth_epochs
         WHERE account_fingerprint IS NOT NULL",
    )?;
    for value in statement.query_map([], |row| row.get::<_, String>(0))? {
        labels.insert(value?);
    }
    Ok(labels)
}

pub fn latest_auth_binding(
    store: &LedgerStore,
    machine_id: &str,
) -> Result<Option<(DateTime<Utc>, AccountBinding)>> {
    let mut statement = store.connection().prepare(
        "SELECT generation, observed_from, account_fingerprint, confidence
         FROM auth_epochs WHERE machine_id = ?1
         ORDER BY observed_from DESC, epoch_id DESC LIMIT 1",
    )?;
    let latest = statement
        .query_row([machine_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .optional()?;
    let Some((generation, observed_from, account_fingerprint, confidence)) = latest else {
        return Ok(None);
    };
    let observed_from = DateTime::parse_from_rfc3339(&observed_from)
        .map(|value| value.with_timezone(&Utc))
        .context("parse stored auth epoch timestamp")?;
    let confidence = match confidence.as_str() {
        "verified" => AttributionConfidence::Verified,
        "inferred" => AttributionConfidence::Inferred,
        _ => AttributionConfidence::Unknown,
    };
    Ok(Some((
        observed_from,
        AccountBinding {
            account_fingerprint,
            confidence,
            auth_generation: Some(generation),
        },
    )))
}

pub fn is_reset(error: TailReset) -> bool {
    matches!(
        error,
        TailReset::FileIdentityChanged | TailReset::FileTruncated
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ClaimSource;

    #[test]
    fn configured_root_bootstraps_git_identity_for_codex_worktrees() {
        let identities = merge_root_git_evidence(
            &[PathBuf::from("/Users/example/project-alpha")],
            Vec::new(),
            &[
                (
                    PathBuf::from("/Users/example/project-alpha"),
                    "https://github.com/example/project-alpha.git".to_owned(),
                ),
                (
                    PathBuf::from("/Users/example/other"),
                    "https://github.com/example/other.git".to_owned(),
                ),
            ],
        );
        assert_eq!(
            identities,
            vec!["https://github.com/example/project-alpha.git"]
        );
        let project = ProjectRecord {
            project_id: "project-alpha".to_owned(),
            project_name: "Project Alpha".to_owned(),
            roots: vec![PathBuf::from("/Users/example/project-alpha")],
            git_identities: identities,
        };
        let resolved = resolve_project(
            ProjectResolutionInput {
                cwd: Some(Path::new(
                    "/Users/example/.codex/worktrees/abc/project-alpha",
                )),
                git_identity: Some("https://github.com/example/project-alpha.git"),
                ..ProjectResolutionInput::default()
            },
            &[project],
        );
        assert_eq!(resolved.project_id.as_deref(), Some("project-alpha"));
        assert_eq!(resolved.method, "git_identity");
    }

    fn quota_test_identity() -> AuthIdentity {
        AuthIdentity {
            account_fingerprint: Some("quota-account".to_owned()),
            person_fingerprint: Some("quota-person".to_owned()),
            workspace_fingerprint: Some("quota-workspace".to_owned()),
            auth_epoch: "quota-generation".to_owned(),
            confidence: AttributionConfidence::Verified,
            person_claim_source: ClaimSource::ChatGptUserId,
            workspace_claim_source: ClaimSource::ChatGptAccountId,
            workspace_claim_consistent: true,
            issuer_fingerprint: None,
            plan_type: Some("pro".to_owned()),
            access_token_expires_at: None,
        }
    }

    fn quota_event(at: DateTime<Utc>, used_percent: f64) -> String {
        serde_json::json!({
            "type": "event_msg",
            "timestamp": at.to_rfc3339(),
            "payload": {
                "type": "token_count",
                "rate_limits": {
                    "limit_id": "weekly",
                    "limit_name": "Weekly",
                    "primary": {
                        "used_percent": used_percent,
                        "window_minutes": 10_080,
                        "resets_at": at.timestamp() + 86_400,
                    }
                }
            }
        })
        .to_string()
    }

    fn quota_test_rollout(codex_home: &Path, thread_id: &str) -> PathBuf {
        let session_dir = codex_home.join("sessions/2026/09/01");
        fs::create_dir_all(&session_dir).unwrap();
        let rollout = session_dir.join(format!("rollout-{thread_id}.jsonl"));
        let index = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(
                    id TEXT PRIMARY KEY,
                    rollout_path TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    thread_source TEXT,
                    first_user_message TEXT
                 );",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads(
                    id, rollout_path, updated_at, thread_source, first_user_message
                 ) VALUES (?1, ?2, ?3, 'root', '')",
                rusqlite::params![
                    thread_id,
                    rollout.to_string_lossy().as_ref(),
                    Utc::now().timestamp(),
                ],
            )
            .unwrap();
        rollout
    }

    #[test]
    fn end_to_end_fixture_is_replay_safe_and_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let codex_home = temporary.path().join("codex-home");
        let session_dir = codex_home.join("sessions/2026/08/31");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("rollout-replay.jsonl"),
            include_bytes!("../tests/fixtures/subagent-replay.jsonl"),
        )
        .unwrap();
        let database = temporary.path().join("ledger/ledger.sqlite3");
        let mut store = prepare_store(&database).unwrap();

        let first = ingest_all(&mut store, &codex_home, "machine-test", None).unwrap();
        assert!(first.confirmed_events > 0);
        assert!(first.quarantined_events > 0);
        assert_eq!(first.issues.len(), 0);

        let trusted = store.aggregate_usage(&Default::default()).unwrap();
        let quarantined_filter = crate::store::AggregateFilter {
            quality: Some(DataQuality::Quarantined),
            ..Default::default()
        };
        let quarantined = store.aggregate_usage(&quarantined_filter).unwrap();
        assert!(trusted.usage.total_tokens < quarantined.usage.total_tokens);

        let second = ingest_all(&mut store, &codex_home, "machine-test", None).unwrap();
        assert_eq!(second.bytes_read, 0);
        assert_eq!(second.inserted_events, 0);
        assert_eq!(second.updated_events, 0);
    }

    #[test]
    fn codex_index_is_the_rollout_directory_and_cursors_select_only_pending_files() {
        let temporary = tempfile::tempdir().unwrap();
        let codex_home = temporary.path().join("codex-home");
        let session_dir = codex_home.join("sessions/2026/08/31");
        fs::create_dir_all(&session_dir).unwrap();
        let indexed = session_dir.join("rollout-indexed.jsonl");
        let unindexed = session_dir.join("rollout-unindexed.jsonl");
        fs::write(
            &indexed,
            include_bytes!("../tests/fixtures/subagent-replay.jsonl"),
        )
        .unwrap();
        fs::write(
            &unindexed,
            include_bytes!("../tests/fixtures/subagent-replay.jsonl"),
        )
        .unwrap();
        let index = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        index
            .execute_batch("CREATE TABLE threads(rollout_path TEXT NOT NULL);")
            .unwrap();
        index
            .execute(
                "INSERT INTO threads(rollout_path) VALUES (?1)",
                [indexed.to_string_lossy().as_ref()],
            )
            .unwrap();
        drop(index);

        let mut store = LedgerStore::open_in_memory().unwrap();
        store
            .set_collector_status(&CollectorStatus {
                mode: "daemon".to_owned(),
                phase: "syncing".to_owned(),
                items_total: 2,
                items_completed: 1,
                bytes_read: 0,
                events_inserted: 0,
                message: None,
                updated_at: Utc::now(),
            })
            .unwrap();
        assert_eq!(
            discover_rollouts(&codex_home).unwrap(),
            vec![indexed.clone()]
        );
        assert_eq!(
            discover_pending_rollouts(&store, &codex_home, "machine-test").unwrap(),
            vec![indexed.clone()]
        );
        let first = ingest_all(&mut store, &codex_home, "machine-test", None).unwrap();
        assert!(first.inserted_events > 0);
        let progress = store.collector_status().unwrap();
        assert_eq!(progress.items_total, 2);
        assert_eq!(progress.items_completed, 2);
        assert!(
            discover_pending_rollouts(&store, &codex_home, "machine-test")
                .unwrap()
                .is_empty()
        );
        assert!(unindexed.is_file());
    }

    #[test]
    fn quota_snapshot_is_captured_even_without_confirmed_token_delta() {
        let temporary = tempfile::tempdir().unwrap();
        let codex_home = temporary.path().join("codex-home");
        let session_dir = codex_home.join("sessions/2026/09/01");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("rollout-quota.jsonl"),
            concat!(
                "{\"type\":\"session_meta\",\"timestamp\":\"2026-09-01T00:00:00Z\",\"payload\":{\"id\":\"quota-thread\"}}\n",
                "{\"type\":\"event_msg\",\"timestamp\":\"2026-09-01T00:01:00Z\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"weekly\",\"limit_name\":\"Weekly\",\"primary\":{\"used_percent\":22,\"window_minutes\":10080,\"resets_at\":2000000000}}}}\n"
            ),
        )
        .unwrap();
        let mut store = LedgerStore::open_in_memory().unwrap();
        let binding = AccountBinding {
            account_fingerprint: Some("account".to_owned()),
            confidence: AttributionConfidence::Verified,
            auth_generation: Some("epoch".to_owned()),
        };

        let first = ingest_all(&mut store, &codex_home, "machine-test", Some(&binding)).unwrap();
        assert_eq!(first.quota_snapshots, 1);
        let latest = store.latest_quota_snapshot("account").unwrap().unwrap();
        assert_eq!(latest.snapshot.pools[0].windows[0].used_percent, Some(22.0));

        let second = ingest_all(&mut store, &codex_home, "machine-test", Some(&binding)).unwrap();
        assert_eq!(second.bytes_read, 0);
        assert_eq!(second.quota_snapshots, 0);
        assert_eq!(store.list_quota_snapshots("account", 10).unwrap().len(), 1);
    }

    #[test]
    fn daemon_quota_tail_is_bounded_incremental_and_never_emits_token_usage() {
        let temporary = tempfile::tempdir().unwrap();
        let codex_home = temporary.path().join("codex-home");
        let rollout = quota_test_rollout(&codex_home, "quota-tail-thread");
        let at = Utc::now() - chrono::Duration::minutes(2);
        fs::write(
            &rollout,
            format!("{{\"type\":\"noise\"}}\n{}\n", quota_event(at, 22.0)),
        )
        .unwrap();
        let subagent_rollout = rollout.with_file_name("rollout-quota-subagent.jsonl");
        fs::write(&subagent_rollout, format!("{}\n", quota_event(at, 99.0))).unwrap();
        let index = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        index
            .execute(
                "INSERT INTO threads(id, rollout_path, updated_at, thread_source)
                 VALUES ('quota-subagent', ?1, ?2, 'subagent')",
                rusqlite::params![
                    subagent_rollout.to_string_lossy().as_ref(),
                    Utc::now().timestamp(),
                ],
            )
            .unwrap();
        let orphan_rollout = rollout.with_file_name("rollout-quota-orphan.jsonl");
        fs::write(&orphan_rollout, format!("{}\n", quota_event(at, 98.0))).unwrap();
        index
            .execute(
                "INSERT INTO threads(
                    id, rollout_path, updated_at, thread_source, first_user_message
                 ) VALUES ('quota-orphan', ?1, ?2, 'user', '<codex_delegation>')",
                rusqlite::params![
                    orphan_rollout.to_string_lossy().as_ref(),
                    Utc::now().timestamp(),
                ],
            )
            .unwrap();
        drop(index);
        let mut store = LedgerStore::open_in_memory().unwrap();
        store
            .append_auth_epoch(
                "machine-test",
                "auth-test",
                &quota_test_identity(),
                at - chrono::Duration::hours(1),
            )
            .unwrap();

        let first = ingest_quota_tails(&mut store, &codex_home, "machine-test").unwrap();
        assert_eq!(first.files_discovered, 1);
        assert_eq!(first.quota_snapshots, 1);
        assert!(first.bytes_read <= QUOTA_TAIL_BOOTSTRAP_BYTES);
        assert_eq!(
            store
                .aggregate_usage(&Default::default())
                .unwrap()
                .usage
                .total_tokens,
            0
        );

        let second = ingest_quota_tails(&mut store, &codex_home, "machine-test").unwrap();
        assert_eq!(second.bytes_read, 0);
        assert_eq!(second.quota_snapshots, 0);

        let mut file = OpenOptions::new().append(true).open(&rollout).unwrap();
        writeln!(
            file,
            "{}",
            quota_event(at + chrono::Duration::minutes(1), 29.0)
        )
        .unwrap();
        let third = ingest_quota_tails(&mut store, &codex_home, "machine-test").unwrap();
        assert_eq!(third.quota_snapshots, 1);
        assert_eq!(
            store
                .list_quota_snapshots("quota-account", 10)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn quota_tail_waits_for_account_epoch_without_losing_the_snapshot() {
        let temporary = tempfile::tempdir().unwrap();
        let codex_home = temporary.path().join("codex-home");
        let rollout = quota_test_rollout(&codex_home, "quota-unbound-thread");
        let at = Utc::now() - chrono::Duration::minutes(1);
        fs::write(&rollout, format!("{}\n", quota_event(at, 41.0))).unwrap();
        let mut store = LedgerStore::open_in_memory().unwrap();

        let unbound = ingest_quota_tails(&mut store, &codex_home, "machine-test").unwrap();
        assert_eq!(unbound.quota_snapshots, 0);
        assert_eq!(
            store
                .get_cursor("machine-test", "quota-rollout:quota-unbound-thread")
                .unwrap()
                .unwrap()
                .byte_offset,
            0
        );

        store
            .append_auth_epoch(
                "machine-test",
                "auth-test",
                &quota_test_identity(),
                at - chrono::Duration::hours(1),
            )
            .unwrap();
        let rebound = ingest_quota_tails(&mut store, &codex_home, "machine-test").unwrap();
        assert_eq!(rebound.quota_snapshots, 1);
        assert_eq!(
            store
                .latest_quota_snapshot("quota-account")
                .unwrap()
                .unwrap()
                .snapshot
                .pools[0]
                .windows[0]
                .used_percent,
            Some(41.0)
        );
    }

    #[test]
    fn verified_quota_epoch_wins_over_overlapping_inferred_history() {
        let at = Utc::now();
        let epochs = vec![
            QuotaAccountEpoch {
                observed_from: at - chrono::Duration::days(1),
                observed_to: None,
                account_fingerprint: "verified".to_owned(),
                generation: "verified-generation".to_owned(),
                confidence: AttributionConfidence::Verified,
            },
            QuotaAccountEpoch {
                observed_from: at - chrono::Duration::hours(1),
                observed_to: None,
                account_fingerprint: "newer-inferred".to_owned(),
                generation: "inferred-generation".to_owned(),
                confidence: AttributionConfidence::Inferred,
            },
        ];
        assert_eq!(
            quota_account_epoch_at(&epochs, at).map(|epoch| epoch.account_fingerprint.as_str()),
            Some("verified")
        );
    }
}
