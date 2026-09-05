use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ingest::physical_file_identity,
    project::{ProjectRecord, ProjectResolutionInput, resolve_project},
    store::{BatchOutcome, FileCursor, LedgerStore},
    types::{
        AttributionConfidence, DataQuality, EventProvenance, ProjectAttribution, TokenUsage,
        UsageEvent,
    },
};

pub const POST_SAMPLING_SOURCE_ID: &str = "logs2-post-sampling-v1";
const MATCH_TOLERANCE_NANOS: i64 = 250_000_000;
const NEW_THREAD_TAIL_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingImportReport {
    pub observations: u64,
    pub matched: u64,
    pub unmatched: u64,
    pub missing_threads: u64,
    pub bytes_read: u64,
    pub inserted_events: u64,
    pub updated_events: u64,
    pub unchanged_events: u64,
    pub first_observed_at: Option<DateTime<Utc>>,
    pub last_observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct Observation {
    anchor_key: String,
    receipt_key: Option<String>,
    log_id: u64,
    observed_at: DateTime<Utc>,
    thread_id: String,
    turn_id: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ThreadInfo {
    rollout_path: Option<PathBuf>,
    parent_thread_id: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    project_id: Option<String>,
    project_name: Option<String>,
}

#[derive(Debug, Clone)]
struct UsageCandidate {
    byte_offset: u64,
    observed_at: DateTime<Utc>,
    usage: TokenUsage,
    used: bool,
}

#[derive(Debug, Clone)]
struct AccountEpoch {
    observed_from: DateTime<Utc>,
    observed_to: Option<DateTime<Utc>>,
    account_fingerprint: String,
    confidence: AttributionConfidence,
}

pub fn ingest_post_sampling(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
) -> Result<SamplingImportReport> {
    let sources = sampling_log_sources(codex_home);
    if sources.is_empty() {
        return Err(anyhow!("Codex logs_2.sqlite is unavailable"));
    }
    let mut combined = SamplingImportReport::default();
    let legacy_binding = store
        .get_cursor(machine_id, POST_SAMPLING_SOURCE_ID)?
        .and_then(|cursor| cursor.parser_state_json)
        .and_then(|state| serde_json::from_str::<Value>(&state).ok())
        .and_then(|state| {
            state
                .get("relativePath")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    for (index, logs_path) in sources.iter().enumerate() {
        let relative = relative_source(codex_home, logs_path);
        let named_source = format!("{POST_SAMPLING_SOURCE_ID}:{relative}");
        // A previously namespaced source must not inherit another source's
        // high-water mark merely because an earlier path disappeared.
        let namespaced = store.get_cursor(machine_id, &named_source)?.is_some()
            || legacy_binding
                .as_ref()
                .map_or(index > 0, |bound| bound != &relative);
        let source_id = if namespaced {
            named_source
        } else {
            POST_SAMPLING_SOURCE_ID.to_owned()
        };
        let namespace = namespaced.then_some(relative);
        let report = ingest_post_sampling_source(
            store,
            codex_home,
            machine_id,
            logs_path,
            &source_id,
            namespace.as_deref(),
        )?;
        merge_report(&mut combined, report);
    }
    Ok(combined)
}

fn ingest_post_sampling_source(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    logs_path: &Path,
    source_id: &str,
    namespace: Option<&str>,
) -> Result<SamplingImportReport> {
    let saved = store.get_cursor(machine_id, source_id)?;
    let saved_state = saved
        .as_ref()
        .and_then(|cursor| cursor.parser_state_json.as_deref())
        .and_then(|state| serde_json::from_str::<Value>(state).ok());
    let physical = physical_file_identity(logs_path, &logs_path.metadata()?)?;
    let physical_replaced = saved_state
        .as_ref()
        .and_then(|state| state.get("physicalIdentity"))
        .and_then(Value::as_str)
        .is_some_and(|previous| previous != physical);
    let previous_id = saved
        .as_ref()
        .map(|cursor| cursor.byte_offset)
        .unwrap_or_default();
    let previous_anchor = saved_state
        .as_ref()
        .and_then(|state| state.get("anchorKey"))
        .and_then(Value::as_str);
    let safe_before = Utc::now() - chrono::Duration::seconds(5);
    let (observations, replaced) = read_observations(
        logs_path,
        previous_id,
        safe_before,
        machine_id,
        previous_anchor,
        physical_replaced,
    )?;
    let mut generation = saved_state
        .as_ref()
        .and_then(|state| state.get("generation"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let effective_namespace = if replaced {
        generation = generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("sampling source generation exhausted"))?;
        Some(format!(
            "{}:generation-{generation}",
            namespace.unwrap_or("primary")
        ))
    } else {
        saved_state
            .as_ref()
            .and_then(|state| state.get("eventNamespace"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| namespace.map(str::to_owned))
    };
    let namespace = effective_namespace.as_deref();
    let last_log_id = if replaced { 0 } else { previous_id };
    let bootstrap = last_log_id == 0;
    if physical_file_identity(logs_path, &logs_path.metadata()?)? != physical {
        return Err(anyhow!(
            "sampling source changed during read; retry without advancing its cursor"
        ));
    }
    if replaced
        && observations
            .iter()
            .any(|observation| observation.receipt_key.is_none())
    {
        return Err(anyhow!(
            "replaced sampling source lacks receipt identity; continuity audit required"
        ));
    }
    if observations.is_empty() {
        return Ok(SamplingImportReport::default());
    }
    let first_observed_at = observations.first().map(|value| value.observed_at);
    let last_observed_at = observations.last().map(|value| value.observed_at);
    let max_log_id = observations
        .last()
        .map(|value| value.log_id)
        .unwrap_or(last_log_id);
    let anchor_keys: BTreeMap<_, _> = observations
        .iter()
        .map(|observation| (observation.log_id, observation.anchor_key.clone()))
        .collect();
    let state_path = logs_path
        .parent()
        .map(|parent| parent.join("state_5.sqlite"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| codex_home.join("state_5.sqlite"));
    let thread_index = load_thread_index(&state_path)?;
    let account_epochs = load_account_epochs(store, machine_id)?;
    let mut by_thread = BTreeMap::<String, Vec<Observation>>::new();
    for observation in observations {
        by_thread
            .entry(observation.thread_id.clone())
            .or_default()
            .push(observation);
    }

    let mut report = SamplingImportReport {
        observations: by_thread.values().map(|values| values.len() as u64).sum(),
        first_observed_at,
        last_observed_at,
        ..SamplingImportReport::default()
    };
    let mut events = Vec::<UsageEvent>::with_capacity(report.observations as usize);
    let mut candidate_cursors = Vec::<(FileCursor, bool)>::new();
    for (thread_id, observations) in by_thread {
        let thread = thread_index.get(&thread_id).cloned().unwrap_or_default();
        let mut rollout_identity = None;
        let mut candidates = match thread.rollout_path.as_deref() {
            Some(path) if path.is_file() => {
                let candidate_id = candidate_source_id(&thread_id, namespace);
                let metadata = path.metadata()?;
                rollout_identity = Some(physical_file_identity(path, &metadata)?);
                let candidate_identity = format!(
                    "sampling-rollout:{thread_id}:{}",
                    physical_file_identity(path, &metadata)?
                );
                let existing = store.get_cursor(machine_id, &candidate_id)?;
                let can_resume = existing.as_ref().is_some_and(|cursor| {
                    cursor.file_identity == candidate_identity
                        && cursor.byte_offset <= metadata.len()
                });
                let stored_offset = if bootstrap {
                    0
                } else if can_resume {
                    existing
                        .as_ref()
                        .map(|cursor| cursor.byte_offset)
                        .unwrap_or_default()
                } else {
                    metadata.len().saturating_sub(NEW_THREAD_TAIL_BYTES)
                };
                let (candidates, next_offset) = read_usage_candidates(
                    path,
                    stored_offset,
                    &mut report.bytes_read,
                    safe_before,
                )?;
                let must_reset = existing.as_ref().is_some_and(|cursor| {
                    cursor.file_identity != candidate_identity || next_offset < cursor.byte_offset
                });
                candidate_cursors.push((
                    FileCursor {
                        machine_id: machine_id.to_owned(),
                        source_id: candidate_id,
                        file_identity: candidate_identity,
                        byte_offset: next_offset,
                        line_number: next_offset,
                        parser_state_json: Some(
                            r#"{"source":"rollout_token_candidates","version":1}"#.to_owned(),
                        ),
                        updated_at: Utc::now(),
                    },
                    must_reset,
                ));
                candidates
            }
            _ => {
                report.missing_threads = report.missing_threads.saturating_add(1);
                Vec::new()
            }
        };
        candidates.sort_by_key(|candidate| candidate.observed_at);
        let mut start = 0_usize;
        for observation in observations {
            let observed_nanos = timestamp_nanos(observation.observed_at);
            while start < candidates.len()
                && timestamp_nanos(candidates[start].observed_at)
                    < observed_nanos.saturating_sub(MATCH_TOLERANCE_NANOS)
            {
                start += 1;
            }
            let mut best: Option<(i64, usize)> = None;
            let mut ambiguous = false;
            for (index, candidate) in candidates.iter().enumerate().skip(start) {
                let candidate_nanos = timestamp_nanos(candidate.observed_at);
                if candidate_nanos > observed_nanos.saturating_add(MATCH_TOLERANCE_NANOS) {
                    break;
                }
                if candidate.used {
                    continue;
                }
                let delta = candidate_nanos.abs_diff(observed_nanos) as i64;
                if best.is_none_or(|(best_delta, _)| delta < best_delta) {
                    best = Some((delta, index));
                    ambiguous = false;
                } else if best.is_some_and(|(best_delta, _)| delta == best_delta) {
                    ambiguous = true;
                }
            }
            let (usage, quality, reason) = if let Some((_, index)) = best.filter(|_| !ambiguous) {
                candidates[index].used = true;
                report.matched = report.matched.saturating_add(1);
                (candidates[index].usage, DataQuality::Confirmed, None)
            } else {
                report.unmatched = report.unmatched.saturating_add(1);
                (
                    TokenUsage::default(),
                    DataQuality::Unknown,
                    Some(
                        if ambiguous {
                            "post_sampling_ambiguous_nearby_last_token_usage"
                        } else {
                            "post_sampling_without_nearby_last_token_usage"
                        }
                        .to_owned(),
                    ),
                )
            };
            let mut event = event_from_observation(
                observation,
                &thread_id,
                &thread,
                machine_id,
                usage,
                quality,
                reason,
                &account_epochs,
                source_id,
                namespace,
            );
            if generation > 0 {
                event.provenance.file_identity = format!(
                    "{}:sampling-generation-{generation}",
                    event.provenance.file_identity
                );
            }
            if !ambiguous && let Some((_, index)) = best {
                event.provenance.candidate_rollout_event_id =
                    rollout_identity.as_deref().map(|identity| {
                        crate::reconstruction::stable_event_id(
                            machine_id,
                            identity,
                            &thread_id,
                            candidates[index].byte_offset,
                        )
                    });
            }
            events.push(event);
        }
    }
    events.sort_by_key(|event| event.provenance.line_number);

    let file_identity = format!("logs2-physical:{physical}");
    for batch in events.chunks(1_000) {
        let batch_end = batch
            .last()
            .map(|event| event.provenance.line_number)
            .unwrap_or(max_log_id);
        let outcome = store.upsert_verified_events_and_cursor(
            batch,
            &FileCursor {
                machine_id: machine_id.to_owned(),
                source_id: source_id.to_owned(),
                file_identity: file_identity.clone(),
                byte_offset: batch_end,
                line_number: batch_end,
                parser_state_json: Some(
                    serde_json::json!({"source":"logs_2_post_sampling","version":4,
                        "relativePath":relative_source(codex_home,logs_path),"physicalIdentity":physical,
                        "anchorKey":anchor_keys.get(&batch_end),
                        "generation":generation,"eventNamespace":namespace})
                    .to_string(),
                ),
                updated_at: Utc::now(),
            },
        )?;
        observe_batch(&mut report, outcome);
    }
    if events.is_empty() {
        store.advance_cursor(&FileCursor {
            machine_id: machine_id.to_owned(),
            source_id: source_id.to_owned(),
            file_identity,
            byte_offset: max_log_id,
            line_number: max_log_id,
            parser_state_json: Some(
                serde_json::json!({"source":"logs_2_post_sampling","version":4,
                "relativePath":relative_source(codex_home,logs_path),"physicalIdentity":physical,
                "anchorKey":anchor_keys.get(&max_log_id),
                "generation":generation,"eventNamespace":namespace})
                .to_string(),
            ),
            updated_at: Utc::now(),
        })?;
    }
    for (cursor, must_reset) in candidate_cursors {
        if must_reset {
            store.reset_cursor(&cursor)?;
        } else {
            store.advance_cursor(&cursor)?;
        }
    }
    Ok(report)
}

fn sampling_log_sources(codex_home: &Path) -> Vec<PathBuf> {
    let primary = codex_home.join("logs_2.sqlite");
    let legacy = codex_home.join("sqlite/logs_2.sqlite");
    let mut paths = Vec::new();
    if primary.is_file() {
        paths.push(primary);
    }
    if legacy.is_file() {
        paths.push(legacy);
    }
    paths
}

fn relative_source(codex_home: &Path, path: &Path) -> String {
    path.strip_prefix(codex_home)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('/', "_")
}

fn read_observations(
    path: &Path,
    after_id: u64,
    safe_before: DateTime<Utc>,
    machine_id: &str,
    previous_anchor: Option<&str>,
    physical_replaced: bool,
) -> Result<(Vec<Observation>, bool)> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;
    // The anchor and appended rows must describe the same source snapshot.
    let transaction = connection.unchecked_transaction()?;
    let anchor_changed = if let Some(expected) = previous_anchor {
        let anchor = read_observation_rows(
            &transaction,
            after_id.saturating_sub(1),
            Some(after_id),
            DateTime::<Utc>::MAX_UTC,
            machine_id,
        )?;
        anchor.first().is_none_or(|row| row.anchor_key != expected)
    } else {
        false // Legacy cursors have no retrospective continuity proof.
    };
    let replaced = physical_replaced || anchor_changed;
    let rows = read_observation_rows(
        &transaction,
        if replaced { 0 } else { after_id },
        None,
        safe_before,
        machine_id,
    )?;
    transaction.commit()?;
    Ok((rows, replaced))
}

fn read_observation_rows(
    connection: &Connection,
    after_id: u64,
    through_id: Option<u64>,
    safe_before: DateTime<Utc>,
    machine_id: &str,
) -> Result<Vec<Observation>> {
    let columns = connection
        .prepare("PRAGMA table_info(logs)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    let process_column = if columns.iter().any(|column| column == "process_uuid") {
        "process_uuid"
    } else {
        "NULL"
    };
    let mut statement = connection.prepare(&format!(
        "SELECT id, ts, ts_nanos, thread_id, feedback_log_body, {process_column}
         FROM logs
         WHERE id > ?1
           AND id <= ?3
           AND ts <= ?2
           AND target = 'codex_core::session::turn'
           AND instr(feedback_log_body, ' post sampling token usage ') > 0
           AND thread_id IS NOT NULL
         ORDER BY id"
    ))?;
    let rows = statement.query_map(
        params![
            i64::try_from(after_id).unwrap_or(i64::MAX),
            safe_before.timestamp(),
            through_id
                .and_then(|id| i64::try_from(id).ok())
                .unwrap_or(i64::MAX)
        ],
        |row| {
            let id: i64 = row.get(0)?;
            let seconds: i64 = row.get(1)?;
            let nanos: i64 = row.get(2)?;
            let body: String = row.get(4)?;
            let thread_id: String = row.get(3)?;
            let process: Option<String> = row.get(5)?;
            Ok(Observation {
                // This checkpoint digest is not a cross-source receipt. Even
                // weak sources can detect mutation, but cannot authorize replay.
                anchor_key: hex::encode(Sha256::digest(
                    serde_json::to_vec(&(id, seconds, nanos, &thread_id, &body, &process))
                        .expect("source scalar tuple is serializable"),
                )),
                receipt_key: source_receipt_key(
                    machine_id,
                    process.as_deref(),
                    id,
                    seconds,
                    nanos,
                    &thread_id,
                    &body,
                ),
                log_id: u64::try_from(id).unwrap_or_default(),
                observed_at: DateTime::<Utc>::from_timestamp(
                    seconds,
                    nanos.clamp(0, 999_999_999) as u32,
                )
                .unwrap_or_else(Utc::now),
                thread_id,
                turn_id: extract_field(&body, "turn.id="),
                model: extract_field(&body, " model="),
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn source_receipt_key(
    machine: &str,
    process: Option<&str>,
    id: i64,
    seconds: i64,
    nanos: i64,
    thread: &str,
    body: &str,
) -> Option<String> {
    let process = process.map(str::trim).filter(|value| !value.is_empty())?;
    let bytes = serde_json::to_vec(&(machine, process, id, seconds, nanos, thread, body)).ok()?;
    Some(format!(
        "sampling-receipt-v1:{}",
        hex::encode(Sha256::digest(bytes))
    ))
}

fn load_thread_index(path: &Path) -> Result<HashMap<String, ThreadInfo>> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.pragma_update(None, "query_only", "ON")?;
    let has_projects = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='projects')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let project_rows = if has_projects {
        connection
            .prepare("SELECT id, name FROM projects")?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let mut projects = Vec::with_capacity(project_rows.len());
    for (project_id, project_name) in project_rows {
        let roots = connection
            .prepare("SELECT path FROM project_roots WHERE project_id = ?1 ORDER BY position")?
            .query_map([&project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        projects.push(ProjectRecord {
            project_id,
            project_name,
            roots,
            git_identities: Vec::new(),
        });
    }
    let has_project_id = connection
        .prepare("PRAGMA table_info(threads)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column| column == "project_id");
    let project_column = if has_project_id { "project_id" } else { "NULL" };
    let mut statement = connection.prepare(&format!(
        "SELECT id, rollout_path, source, model, cwd, {project_column} FROM threads"
    ))?;
    let rows = statement.query_map([], |row| {
        let source: String = row.get(2)?;
        let native_project_id: Option<String> = row.get(5)?;
        let cwd: Option<String> = row.get(4)?;
        let project = resolve_project(
            ProjectResolutionInput {
                manual: None,
                native_project_id: native_project_id.as_deref(),
                cwd: cwd.as_deref().map(Path::new),
                git_identity: None,
                parent: None,
            },
            &projects,
        );
        let parent_thread_id = serde_json::from_str::<Value>(&source)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/subagent/thread_spawn/parent_thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        Ok((
            row.get::<_, String>(0)?,
            ThreadInfo {
                rollout_path: row
                    .get::<_, Option<String>>(1)?
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from),
                parent_thread_id,
                model: row.get(3)?,
                cwd,
                project_name: project.project_name,
                project_id: project.project_id,
            },
        ))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(Into::into)
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
        let observed_from = DateTime::parse_from_rfc3339(&from)?.with_timezone(&Utc);
        let observed_to = to
            .map(|value| DateTime::parse_from_rfc3339(&value))
            .transpose()?
            .map(|value| value.with_timezone(&Utc));
        epochs.push(AccountEpoch {
            observed_from,
            observed_to,
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

fn read_usage_candidates(
    path: &Path,
    start_offset: u64,
    bytes_read: &mut u64,
    safe_before: DateTime<Utc>,
) -> Result<(Vec<UsageCandidate>, u64)> {
    let mut file = File::open(path).with_context(|| format!("open rollout {}", path.display()))?;
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
    let mut candidates = Vec::new();
    let mut durable_offset = reader.stream_position()?;
    let safe_before = safe_before.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
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
        if extract_json_timestamp(&line).is_some_and(|timestamp| timestamp > safe_before.as_str()) {
            durable_offset = line_start;
            break;
        }
        durable_offset = reader.stream_position()?;
        if !line.contains(r#""type":"token_count""#) {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(usage) = value.pointer("/payload/info/last_token_usage") else {
            continue;
        };
        let Some(observed_at) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        else {
            continue;
        };
        let cache_write = usage
            .get("cache_write_input_tokens")
            .and_then(Value::as_u64);
        let input_tokens = usage_u64(usage, "input_tokens");
        let usage = TokenUsage {
            input_tokens,
            cached_input_tokens: usage_u64(usage, "cached_input_tokens"),
            cache_write_input_tokens: cache_write.unwrap_or_default(),
            cache_write_observed_input_tokens: cache_write.map_or(0, |_| input_tokens),
            output_tokens: usage_u64(usage, "output_tokens"),
            reasoning_output_tokens: usage_u64(usage, "reasoning_output_tokens"),
            total_tokens: usage_u64(usage, "total_tokens"),
        };
        if usage.validate().is_err() {
            continue;
        }
        candidates.push(UsageCandidate {
            byte_offset: line_start,
            observed_at,
            usage,
            used: false,
        });
    }
    let next_offset = durable_offset;
    *bytes_read = bytes_read.saturating_add(next_offset.saturating_sub(start_offset));
    Ok((candidates, next_offset))
}

fn candidate_source_id(thread_id: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) => format!("sampling-rollout:{namespace}:{thread_id}"),
        None => format!("sampling-rollout:{thread_id}"),
    }
}

fn extract_json_timestamp(line: &str) -> Option<&str> {
    let marker = r#""timestamp":""#;
    let start = line.find(marker)? + marker.len();
    let tail = &line[start..];
    let end = tail.find('"')?;
    Some(&tail[..end])
}

#[allow(clippy::too_many_arguments)]
fn event_from_observation(
    observation: Observation,
    thread_id: &str,
    thread: &ThreadInfo,
    machine_id: &str,
    usage: TokenUsage,
    quality: DataQuality,
    quality_reason: Option<String>,
    account_epochs: &[AccountEpoch],
    source_id: &str,
    namespace: Option<&str>,
) -> UsageEvent {
    let account = account_epoch_at(account_epochs, observation.observed_at);
    UsageEvent {
        event_id: match namespace {
            Some(namespace) => format!("logs2-post-sampling:{namespace}:{}", observation.log_id),
            None => format!("logs2-post-sampling:{}", observation.log_id),
        },
        observed_at: observation.observed_at,
        source_timestamp: Some(observation.observed_at),
        thread_id: Some(thread_id.to_owned()),
        parent_thread_id: thread.parent_thread_id.clone(),
        model: observation.model.or_else(|| thread.model.clone()),
        cwd: thread.cwd.clone(),
        account_fingerprint: account.map(|epoch| epoch.account_fingerprint.clone()),
        account_confidence: account
            .map(|epoch| epoch.confidence)
            .unwrap_or(AttributionConfidence::Unknown),
        project: ProjectAttribution {
            project_id: thread.project_id.clone(),
            project_name: thread.project_name.clone(),
            confidence: if thread.project_id.is_some() {
                AttributionConfidence::Verified
            } else {
                AttributionConfidence::Unknown
            },
            method: if thread.project_id.is_some() {
                "state_5_project_id".to_owned()
            } else {
                "unassigned".to_owned()
            },
        },
        usage,
        quality,
        quality_reason,
        provenance: EventProvenance {
            source_turn_id: observation.turn_id.clone(),
            candidate_rollout_event_id: None,
            sampling_receipt_key: observation.receipt_key,
            machine_id: machine_id.to_owned(),
            source_id: source_id.to_owned(),
            rollout_id: thread_id.to_owned(),
            file_identity: observation.turn_id.unwrap_or_else(|| thread_id.to_owned()),
            byte_offset: observation.log_id,
            line_number: observation.log_id,
        },
    }
}

fn account_epoch_at(epochs: &[AccountEpoch], observed_at: DateTime<Utc>) -> Option<&AccountEpoch> {
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

fn extract_field(body: &str, marker: &str) -> Option<String> {
    let start = body.find(marker)? + marker.len();
    let tail = &body[start..];
    let end = tail
        .find(|character: char| character.is_whitespace() || character == '}')
        .unwrap_or(tail.len());
    (end > 0).then(|| tail[..end].trim_matches('"').to_owned())
}

fn usage_u64(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or_default()
}

fn timestamp_nanos(value: DateTime<Utc>) -> i64 {
    value.timestamp_nanos_opt().unwrap_or_else(|| {
        value
            .timestamp()
            .saturating_mul(1_000_000_000)
            .saturating_add(i64::from(value.timestamp_subsec_nanos()))
    })
}

fn observe_batch(report: &mut SamplingImportReport, outcome: BatchOutcome) {
    report.inserted_events = report
        .inserted_events
        .saturating_add(outcome.inserted as u64);
    report.updated_events = report.updated_events.saturating_add(outcome.updated as u64);
    report.unchanged_events = report
        .unchanged_events
        .saturating_add(outcome.unchanged as u64);
}

fn merge_report(combined: &mut SamplingImportReport, report: SamplingImportReport) {
    combined.observations = combined.observations.saturating_add(report.observations);
    combined.matched = combined.matched.saturating_add(report.matched);
    combined.unmatched = combined.unmatched.saturating_add(report.unmatched);
    combined.missing_threads = combined
        .missing_threads
        .saturating_add(report.missing_threads);
    combined.bytes_read = combined.bytes_read.saturating_add(report.bytes_read);
    combined.inserted_events = combined
        .inserted_events
        .saturating_add(report.inserted_events);
    combined.updated_events = combined
        .updated_events
        .saturating_add(report.updated_events);
    combined.unchanged_events = combined
        .unchanged_events
        .saturating_add(report.unchanged_events);
    combined.first_observed_at = match (combined.first_observed_at, report.first_observed_at) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (None, value) | (value, None) => value,
    };
    combined.last_observed_at = match (combined.last_observed_at, report.last_observed_at) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (None, value) | (value, None) => value,
    };
}

#[cfg(test)]
mod tests {
    use std::{fs, fs::OpenOptions, io::Write};

    use chrono::SecondsFormat;
    use tempfile::tempdir;

    use super::*;
    use crate::store::AggregateFilter;

    fn token_line(at: DateTime<Utc>, total: u64) -> String {
        let input = total - 10;
        serde_json::json!({
            "timestamp": at.to_rfc3339_opts(SecondsFormat::Nanos, true),
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {"last_token_usage": {
                    "input_tokens": input,
                    "cached_input_tokens": input - 20,
                    "output_tokens": 10,
                    "reasoning_output_tokens": 3,
                    "total_tokens": total
                }}
            }
        })
        .to_string()
    }

    fn insert_log(connection: &Connection, at: DateTime<Utc>, turn: &str) {
        connection
            .execute(
                "INSERT INTO logs(ts, ts_nanos, level, target, feedback_log_body,
                                  thread_id, process_uuid, estimated_bytes)
                 VALUES (?1, ?2, 'TRACE', 'codex_core::session::turn', ?3,
                         'thread-1', 'process', 1)",
                params![
                    at.timestamp(),
                    at.timestamp_subsec_nanos(),
                    format!(
                        "session_loop{{thread_id=thread-1}}:turn{{turn.id={turn} model=gpt-5.6-sol}}: post sampling token usage turn_id={turn} total_usage_tokens=100"
                    )
                ],
            )
            .unwrap();
    }

    #[test]
    fn post_sampling_and_rollout_cursors_only_read_appended_evidence() {
        let temporary = tempdir().unwrap();
        let codex_home = temporary.path();
        let session_dir = codex_home.join("sessions/2026/08/31");
        fs::create_dir_all(&session_dir).unwrap();
        let rollout = session_dir.join("rollout-thread-1.jsonl");
        let first_at = Utc::now() - chrono::Duration::minutes(2);
        fs::write(&rollout, format!("{}\n", token_line(first_at, 100))).unwrap();

        let state = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        state
            .execute_batch(
                "CREATE TABLE projects(id TEXT PRIMARY KEY, name TEXT NOT NULL);
                 CREATE TABLE project_roots(
                    project_id TEXT NOT NULL, path TEXT NOT NULL, position INTEGER NOT NULL
                 );
                 CREATE TABLE threads(
                    id TEXT PRIMARY KEY, rollout_path TEXT, source TEXT, model TEXT,
                    cwd TEXT, project_id TEXT
                 );
                 INSERT INTO projects VALUES ('project-1', 'Project One');",
            )
            .unwrap();
        state
            .execute(
                "INSERT INTO threads VALUES ('thread-1', ?1, 'vscode',
                                             'gpt-5.6-sol', '/work', 'project-1')",
                [rollout.to_string_lossy().as_ref()],
            )
            .unwrap();

        let logs = Connection::open(codex_home.join("logs_2.sqlite")).unwrap();
        logs.execute_batch(
            "CREATE TABLE logs(
                    id INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL,
                    ts_nanos INTEGER NOT NULL, level TEXT NOT NULL, target TEXT NOT NULL,
                    feedback_log_body TEXT, thread_id TEXT, process_uuid TEXT,
                    estimated_bytes INTEGER NOT NULL DEFAULT 0
                 );",
        )
        .unwrap();
        insert_log(
            &logs,
            first_at + chrono::Duration::milliseconds(4),
            "turn-1",
        );

        let mut store = LedgerStore::open_in_memory().unwrap();
        let first = ingest_post_sampling(&mut store, codex_home, "machine").unwrap();
        assert_eq!(
            (first.observations, first.matched, first.unmatched),
            (1, 1, 0)
        );

        let second_at = Utc::now() - chrono::Duration::minutes(1);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(second_at, 200)
        )
        .unwrap();
        insert_log(
            &logs,
            second_at + chrono::Duration::milliseconds(3),
            "turn-2",
        );
        let second = ingest_post_sampling(&mut store, codex_home, "machine").unwrap();
        assert_eq!(
            (second.observations, second.matched, second.unmatched),
            (1, 1, 0)
        );
        assert!(second.bytes_read < first.bytes_read + 1_024);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            300
        );

        let third = ingest_post_sampling(&mut store, codex_home, "machine").unwrap();
        assert_eq!(third.observations, 0);
        assert_eq!(third.bytes_read, 0);

        // Codex may compact a rollout in place. The physical identity remains
        // stable while the byte length moves behind the durable candidate
        // cursor; this must be an explicit verified reset, not a daemon-killing
        // cursor regression.
        let fourth_at = Utc::now() - chrono::Duration::seconds(30);
        fs::write(&rollout, format!("{}\n", token_line(fourth_at, 400))).unwrap();
        insert_log(
            &logs,
            fourth_at + chrono::Duration::milliseconds(2),
            "turn-3",
        );
        let fourth = ingest_post_sampling(&mut store, codex_home, "machine").unwrap();
        assert_eq!(
            (fourth.observations, fourth.matched, fourth.unmatched),
            (1, 1, 0)
        );
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            700
        );
        let ambiguous_at = Utc::now() - chrono::Duration::seconds(10);
        let mut append = OpenOptions::new().append(true).open(&rollout).unwrap();
        writeln!(append, "{}", token_line(ambiguous_at, 500)).unwrap();
        writeln!(append, "{}", token_line(ambiguous_at, 900)).unwrap();
        insert_log(&logs, ambiguous_at, "ambiguous-turn");
        let ambiguous = ingest_post_sampling(&mut store, codex_home, "machine").unwrap();
        assert_eq!(
            (
                ambiguous.observations,
                ambiguous.matched,
                ambiguous.unmatched
            ),
            (1, 0, 1)
        );
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            700
        );
        let reason: String = store
            .connection()
            .query_row(
                "SELECT quality_reason FROM usage_events WHERE quality='unknown'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reason, "post_sampling_ambiguous_nearby_last_token_usage");
        let linked: i64 = store.connection().query_row(
            "SELECT COUNT(*) FROM sampling_candidate_links WHERE method='unique_nearest_timestamp'",
            [], |row| row.get(0)
        ).unwrap();
        assert_eq!(
            linked, 3,
            "the ambiguous fourth observation must not get a candidate link"
        );
    }

    #[test]
    fn receipt_keys_need_process_identity_and_distinguish_requests() {
        let key =
            source_receipt_key("machine", Some("process"), 1, 100, 5, "thread", "body").unwrap();
        assert_eq!(
            Some(key.clone()),
            source_receipt_key("machine", Some("process"), 1, 100, 5, "thread", "body")
        );
        assert_ne!(
            Some(key.clone()),
            source_receipt_key("machine", Some("process"), 2, 100, 5, "thread", "body")
        );
        assert_ne!(
            Some(key),
            source_receipt_key(
                "machine",
                Some("other-process"),
                1,
                100,
                5,
                "thread",
                "body"
            )
        );
        assert!(source_receipt_key("machine", None, 1, 100, 5, "thread", "body").is_none());
    }

    fn copied_source_fixture() -> (tempfile::TempDir, LedgerStore, DateTime<Utc>, PathBuf) {
        let temporary = tempdir().unwrap();
        let home = temporary.path();
        let rollout = home.join("rollout.jsonl");
        let at = Utc::now() - chrono::Duration::minutes(2);
        fs::write(&rollout, format!("{}\n", token_line(at, 100))).unwrap();
        {
            let state = Connection::open(home.join("state_5.sqlite")).unwrap();
            state.execute_batch(
                "CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT NOT NULL);
                 CREATE TABLE project_roots(project_id TEXT,path TEXT,position INTEGER);
                 CREATE TABLE threads(id TEXT PRIMARY KEY,rollout_path TEXT,source TEXT,model TEXT,cwd TEXT,project_id TEXT);
                 INSERT INTO projects VALUES ('project','Synthetic');"
            ).unwrap();
            state.execute("INSERT INTO threads VALUES ('thread-1',?1,'vscode','gpt-5.6-sol','/work','project')",
                [rollout.to_string_lossy().as_ref()]).unwrap();
            let logs = Connection::open(home.join("logs_2.sqlite")).unwrap();
            logs.execute_batch(
                "CREATE TABLE logs(id INTEGER PRIMARY KEY AUTOINCREMENT,ts INTEGER,ts_nanos INTEGER,
                 level TEXT,target TEXT,feedback_log_body TEXT,thread_id TEXT,process_uuid TEXT,estimated_bytes INTEGER);"
            ).unwrap();
            insert_log(&logs, at, "shared-turn");
        }
        let mut store = LedgerStore::open_in_memory().unwrap();
        ingest_post_sampling(&mut store, home, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100
        );
        fs::create_dir(home.join("sqlite")).unwrap();
        fs::copy(
            home.join("logs_2.sqlite"),
            home.join("sqlite/logs_2.sqlite"),
        )
        .unwrap();
        fs::copy(
            home.join("state_5.sqlite"),
            home.join("sqlite/state_5.sqlite"),
        )
        .unwrap();
        ingest_post_sampling(&mut store, home, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100,
            "a copied source must not become a second model call"
        );
        let next_at = at + chrono::Duration::seconds(1);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 150)
        )
        .unwrap();
        {
            let migrated = Connection::open(home.join("sqlite/logs_2.sqlite")).unwrap();
            insert_log(&migrated, next_at, "new-migrated-turn");
        }
        let report = ingest_post_sampling(&mut store, home, "machine").unwrap();
        assert_eq!(report.inserted_events, 1);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        let idle = ingest_post_sampling(&mut store, home, "machine").unwrap();
        assert_eq!(idle.observations, 0);
        assert_eq!(idle.bytes_read, 0);
        (temporary, store, at, rollout)
    }

    #[test]
    fn copied_log_sources_must_not_duplicate_sampling() {
        let _fixture = copied_source_fixture();
    }

    #[test]
    fn migrated_source_keeps_its_cursor_after_primary_is_removed() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let root = temporary.path();
        let primary_at = at + chrono::Duration::seconds(3);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(primary_at, 200)
        )
        .unwrap();
        {
            let primary = Connection::open(root.join("logs_2.sqlite")).unwrap();
            primary
                .execute("UPDATE sqlite_sequence SET seq=99 WHERE name='logs'", [])
                .unwrap();
            insert_log(&primary, primary_at, "primary-high-watermark");
        }
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            450
        );
        fs::rename(root.join("logs_2.sqlite"), root.join("paused-logs.sqlite")).unwrap();
        let next_at = at + chrono::Duration::seconds(4);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 180)
        )
        .unwrap();
        {
            let migrated = Connection::open(root.join("sqlite/logs_2.sqlite")).unwrap();
            insert_log(&migrated, next_at, "migrated-after-removal");
        }
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            630
        );
    }

    #[test]
    fn initially_migrated_source_keeps_binding_when_primary_appears() {
        let (temporary, _previous_store, at, rollout) = copied_source_fixture();
        let root = temporary.path();
        fs::rename(
            root.join("logs_2.sqlite"),
            root.join("paused-primary.sqlite"),
        )
        .unwrap();
        let mut store = LedgerStore::open_in_memory().unwrap();
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        fs::rename(
            root.join("paused-primary.sqlite"),
            root.join("logs_2.sqlite"),
        )
        .unwrap();
        let next_at = at + chrono::Duration::seconds(3);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 200)
        )
        .unwrap();
        {
            let primary = Connection::open(root.join("logs_2.sqlite")).unwrap();
            insert_log(&primary, next_at, "new-primary-request");
        }
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            450
        );
        assert_eq!(
            ingest_post_sampling(&mut store, root, "machine")
                .unwrap()
                .observations,
            0
        );
    }

    #[test]
    fn same_file_log_reset_must_not_skip_reused_row_ids() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let root = temporary.path();
        let primary = root.join("logs_2.sqlite");
        let identity = physical_file_identity(&primary, &primary.metadata().unwrap()).unwrap();
        let old_checkpoint = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        let next_at = at + chrono::Duration::seconds(6);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 180)
        )
        .unwrap();
        {
            let reset = Connection::open(&primary).unwrap();
            reset
                .execute_batch(
                    "DELETE FROM logs; UPDATE sqlite_sequence SET seq=0 WHERE name='logs';",
                )
                .unwrap();
            assert_eq!(
                ingest_post_sampling(&mut store, root, "machine")
                    .unwrap()
                    .observations,
                0
            );
            assert_eq!(
                store
                    .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
                    .unwrap()
                    .unwrap()
                    .parser_state_json,
                old_checkpoint.parser_state_json
            );
            insert_log(&reset, next_at, "shared-turn");
        }
        assert_eq!(
            physical_file_identity(&primary, &primary.metadata().unwrap()).unwrap(),
            identity
        );
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            430
        );
        assert_eq!(
            ingest_post_sampling(&mut store, root, "machine")
                .unwrap()
                .observations,
            0
        );
        let checkpoint = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        let state: Value =
            serde_json::from_str(checkpoint.parser_state_json.as_deref().unwrap()).unwrap();
        assert_eq!(state["version"], 4);
        assert_eq!(state["generation"], 1);
        assert_eq!(state["anchorKey"].as_str().unwrap().len(), 64);
        Connection::open(&primary)
            .unwrap()
            .execute("UPDATE logs SET process_uuid=NULL", [])
            .unwrap();
        assert!(ingest_post_sampling(&mut store, root, "machine").is_err());
        assert_eq!(
            store
                .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
                .unwrap()
                .unwrap()
                .parser_state_json,
            checkpoint.parser_state_json
        );
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            430
        );
    }

    #[test]
    fn removed_anchor_replays_remaining_receipts_without_recounting() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let root = temporary.path();
        let next_at = at + chrono::Duration::seconds(6);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 210)
        )
        .unwrap();
        {
            let migrated = Connection::open(root.join("sqlite/logs_2.sqlite")).unwrap();
            migrated.execute("DELETE FROM logs WHERE id=2", []).unwrap();
            insert_log(&migrated, next_at, "after-pruned-anchor");
        }
        let report = ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(report.observations, 2);
        assert_eq!(report.unchanged_events, 1);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            460
        );
        let idle = ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(idle.observations, 0);
        assert_eq!(idle.bytes_read, 0);
    }

    #[test]
    fn physical_log_replacement_replays_copies_once_and_accepts_reset_ids() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let root = temporary.path();
        let primary = root.join("logs_2.sqlite");
        let replacement = root.join("replacement.sqlite");
        fs::copy(&primary, &replacement).unwrap();
        fs::rename(&primary, root.join("old-primary.sqlite")).unwrap();
        fs::rename(&replacement, &primary).unwrap();
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        let next_at = at + chrono::Duration::seconds(6);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 180)
        )
        .unwrap();
        fs::copy(&primary, &replacement).unwrap();
        {
            let reset = Connection::open(&replacement).unwrap();
            reset
                .execute_batch(
                    "DELETE FROM logs; UPDATE sqlite_sequence SET seq=0 WHERE name='logs';",
                )
                .unwrap();
            insert_log(&reset, next_at, "shared-turn");
        }
        fs::rename(&primary, root.join("second-old-primary.sqlite")).unwrap();
        fs::rename(&replacement, &primary).unwrap();
        ingest_post_sampling(&mut store, root, "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            430
        );
        let checkpoint = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        let state: Value =
            serde_json::from_str(checkpoint.parser_state_json.as_deref().unwrap()).unwrap();
        assert_eq!(state["generation"], 2);
        assert_eq!(checkpoint.byte_offset, 1);
        assert_eq!(
            ingest_post_sampling(&mut store, root, "machine")
                .unwrap()
                .observations,
            0
        );
        fs::copy(&primary, &replacement).unwrap();
        Connection::open(&replacement)
            .unwrap()
            .execute("UPDATE logs SET process_uuid=NULL", [])
            .unwrap();
        fs::rename(&primary, root.join("third-old-primary.sqlite")).unwrap();
        fs::rename(&replacement, &primary).unwrap();
        assert!(ingest_post_sampling(&mut store, root, "machine").is_err());
        let preserved = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        assert_eq!(preserved.parser_state_json, checkpoint.parser_state_json);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            430
        );
    }

    #[test]
    fn verified_account_epoch_wins_over_overlapping_inferred_history() {
        let at = Utc::now();
        let epochs = vec![
            AccountEpoch {
                observed_from: at - chrono::Duration::days(1),
                observed_to: None,
                account_fingerprint: "verified".to_owned(),
                confidence: AttributionConfidence::Verified,
            },
            AccountEpoch {
                observed_from: at - chrono::Duration::hours(1),
                observed_to: None,
                account_fingerprint: "newer-inferred".to_owned(),
                confidence: AttributionConfidence::Inferred,
            },
        ];
        assert_eq!(
            account_epoch_at(&epochs, at).map(|epoch| epoch.account_fingerprint.as_str()),
            Some("verified")
        );
    }
}
