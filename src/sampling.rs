use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ingest::physical_file_identity,
    project::{ProjectRecord, ProjectResolutionInput, resolve_project},
    store::{BatchOutcome, FileCursor, LedgerStore},
    stream_boundary::{BoundaryAction, StreamBoundary, StreamPhase, record_timestamp},
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
    record_digest: String,
    byte_offset: u64,
    observed_at: DateTime<Utc>,
    usage: Option<TokenUsage>,
    unavailable_reason: Option<&'static str>,
}

/// Paired with the candidate byte cursor, never restored independently of it.
#[derive(Debug, Serialize, Deserialize)]
struct CandidateCounterCheckpoint {
    version: u32,
    previous_total: Option<TokenUsage>,
    cumulative_seen: bool,
    allow_initial_sample: bool,
    #[serde(default = "unknown_stream_boundary")]
    boundary: StreamBoundary,
    #[serde(default)]
    last_token_at: Option<DateTime<Utc>>,
    #[serde(default)]
    allow_unframed: bool,
    #[serde(default)]
    canonical_header_checked: bool,
}

fn unknown_stream_boundary() -> StreamBoundary {
    StreamBoundary {
        foreign_replay: true,
        ..StreamBoundary::default()
    }
}

impl CandidateCounterCheckpoint {
    fn new(start_offset: u64) -> Self {
        Self {
            version: 3,
            previous_total: None,
            cumulative_seen: start_offset > 0,
            allow_initial_sample: start_offset == 0,
            boundary: if start_offset == 0 {
                StreamBoundary::default()
            } else {
                unknown_stream_boundary()
            },
            last_token_at: None,
            allow_unframed: start_offset == 0,
            canonical_header_checked: start_offset == 0,
        }
    }

    fn observe_record(
        &mut self,
        record: &Value,
        thread_id: &str,
        is_child: bool,
    ) -> (Option<TokenUsage>, Option<&'static str>) {
        let total = record
            .pointer("/payload/info/total_token_usage")
            .and_then(parse_candidate_usage);
        if record.get("type").and_then(Value::as_str) == Some("session_meta") {
            self.allow_unframed = false;
        }
        let action = self.boundary.classify(
            record,
            thread_id,
            is_child,
            self.last_token_at,
            self.previous_total.is_some(),
            total.is_some(),
        );
        let unframed_root = self.allow_unframed
            && !is_child
            && self.boundary.phase == StreamPhase::AwaitingCanonical
            && record.get("type").and_then(Value::as_str) == Some("event_msg")
            && record.pointer("/payload/type").and_then(Value::as_str) == Some("token_count");
        if action == BoundaryAction::Usage || unframed_root {
            let result = self.normalize(record);
            if total.is_some() {
                self.last_token_at = record_timestamp(record);
            }
            return result;
        }
        if action == BoundaryAction::Baseline {
            self.previous_total = total;
            self.cumulative_seen = true;
            self.allow_initial_sample = false;
            self.last_token_at = record_timestamp(record);
            return (None, Some("post_sampling_inherited_history"));
        }
        (None, Some("post_sampling_unestablished_or_replayed_stream"))
    }

    fn normalize(&mut self, record: &Value) -> (Option<TokenUsage>, Option<&'static str>) {
        let last = record
            .pointer("/payload/info/last_token_usage")
            .and_then(parse_candidate_usage);
        let Some(raw_total) = record.pointer("/payload/info/total_token_usage") else {
            if !self.cumulative_seen {
                return (
                    last,
                    last.is_none()
                        .then_some("post_sampling_invalid_nearby_last_token_usage"),
                );
            }
            // Never bridge an unobserved counter interval and assign its whole
            // increment to the following marker's timestamp/model/account.
            self.previous_total = None;
            self.allow_initial_sample = false;
            return (None, Some("post_sampling_missing_counter_continuity"));
        };
        self.cumulative_seen = true;
        let Some(total) = parse_candidate_usage(raw_total) else {
            self.previous_total = None;
            self.allow_initial_sample = false;
            return (None, Some("post_sampling_invalid_cumulative_usage"));
        };
        let allowed_last = if self.previous_total.is_some() || self.allow_initial_sample {
            last
        } else {
            None
        };
        let step = crate::counter::normalize_counter(self.previous_total, total, allowed_last);
        self.previous_total = Some(total);
        self.allow_initial_sample = false;
        let reason = if step.unchanged {
            Some("post_sampling_counter_unchanged")
        } else if step.usage.is_none() {
            Some("post_sampling_counter_baseline_only")
        } else {
            None
        };
        (step.usage, reason)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nearest {
    Missing,
    Ambiguous,
    Unique(usize),
}

fn nearest(times: &[i128], at: i128) -> Nearest {
    let right = times.partition_point(|value| *value < at);
    let mut best: Option<(u128, usize)> = None;
    let mut tied = false;
    for index in [right.checked_sub(1), (right < times.len()).then_some(right)]
        .into_iter()
        .flatten()
    {
        let distance = times[index].abs_diff(at);
        if distance > MATCH_TOLERANCE_NANOS as u128 {
            continue;
        }
        if best.is_none_or(|(previous, _)| distance < previous) {
            best = Some((distance, index));
            tied = false;
        } else if best.is_some_and(|(previous, _)| distance == previous) {
            tied = true;
        }
    }
    let Some((_, index)) = best else {
        return Nearest::Missing;
    };
    if tied
        || (index > 0 && times[index - 1] == times[index])
        || (index + 1 < times.len() && times[index + 1] == times[index])
    {
        Nearest::Ambiguous
    } else {
        Nearest::Unique(index)
    }
}

/// Associations within the supplied mature cohort, not proof of model calls.
/// A used nearest candidate must never force a farther replacement match.
fn mutual_matches(observations: &[i128], candidates: &[i128]) -> Vec<Nearest> {
    observations
        .iter()
        .enumerate()
        .map(|(index, at)| match nearest(candidates, *at) {
            Nearest::Unique(candidate)
                if nearest(observations, candidates[candidate]) == Nearest::Unique(index) =>
            {
                Nearest::Unique(candidate)
            }
            Nearest::Unique(_) => Nearest::Ambiguous,
            other => other,
        })
        .collect()
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
    let first_observed_at = observations.iter().map(|value| value.observed_at).min();
    let last_observed_at = observations.iter().map(|value| value.observed_at).max();
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
    for (thread_id, mut observations) in by_thread {
        // Candidate search uses a monotonic timestamp pointer, but log IDs may
        // arrive out of timestamp order. Commit order is restored by log ID below.
        observations.sort_by_key(|observation| (observation.observed_at, observation.log_id));
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
                let mut counter = if can_resume && !bootstrap {
                    existing
                        .as_ref()
                        .and_then(|cursor| cursor.parser_state_json.as_deref())
                        .and_then(|value| {
                            serde_json::from_str::<CandidateCounterCheckpoint>(value).ok()
                        })
                        .filter(|state| {
                            matches!(state.version, 2 | 3)
                                && state
                                    .previous_total
                                    .is_none_or(|usage| usage.validate().is_ok())
                                && (state.previous_total.is_none()
                                    || (state.cumulative_seen && !state.allow_initial_sample))
                        })
                        .unwrap_or_else(|| CandidateCounterCheckpoint::new(stored_offset))
                } else {
                    CandidateCounterCheckpoint::new(stored_offset)
                };
                // Version 2 had numeric state only. Preserve its baseline but
                // do not pretend its unknown stream/replay phase was live.
                counter.version = 3;
                if !counter.canonical_header_checked {
                    if counter.boundary.canonical_at.is_none() {
                        counter.boundary.canonical_at =
                            read_canonical_header(path, &thread_id, &mut report.bytes_read)?;
                    }
                    counter.canonical_header_checked = true;
                }
                let (candidates, next_offset) = read_usage_candidates(
                    path,
                    stored_offset,
                    &mut report.bytes_read,
                    safe_before,
                    &mut counter,
                    &thread_id,
                    thread.parent_thread_id.is_some(),
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
                        parser_state_json: Some(serde_json::to_string(&counter)?),
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
        let matches = mutual_matches(
            &observations
                .iter()
                .map(|value| timestamp_nanos(value.observed_at))
                .collect::<Vec<_>>(),
            &candidates
                .iter()
                .map(|value| timestamp_nanos(value.observed_at))
                .collect::<Vec<_>>(),
        );
        for (observation, association) in observations.into_iter().zip(matches) {
            let invalid =
                matches!(association, Nearest::Unique(index) if candidates[index].usage.is_none());
            let matched = match association {
                Nearest::Unique(index) if !invalid => Some(index),
                _ => None,
            };
            let (usage, quality, reason) = if let Some(index) = matched {
                report.matched = report.matched.saturating_add(1);
                (
                    candidates[index].usage.expect("validated candidate"),
                    DataQuality::Confirmed,
                    None,
                )
            } else {
                report.unmatched = report.unmatched.saturating_add(1);
                (
                    TokenUsage::default(),
                    DataQuality::Unknown,
                    Some(
                        if invalid {
                            match association {
                                Nearest::Unique(index) => candidates[index]
                                    .unavailable_reason
                                    .unwrap_or("post_sampling_invalid_nearby_last_token_usage"),
                                _ => unreachable!(),
                            }
                        } else if association == Nearest::Ambiguous {
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
            if let Some(index) = matched {
                event.provenance.source_record_key = rollout_identity.as_deref().map(|identity| {
                    crate::reconstruction::source_record_key(
                        machine_id,
                        identity,
                        &thread_id,
                        candidates[index].byte_offset,
                        &candidates[index].record_digest,
                    )
                });
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
    let outcome = store.upsert_sampling_events_and_cursors(
            &events,
            &FileCursor {
                machine_id: machine_id.to_owned(),
                source_id: source_id.to_owned(),
                file_identity: file_identity.clone(),
                byte_offset: max_log_id,
                line_number: max_log_id,
                parser_state_json: Some(
                    serde_json::json!({"source":"logs_2_post_sampling","version":4,
                        "associationPolicy":"mutual_unique_nearest_v2",
                        "counterPolicy":"shared_numeric_counter_v1",
                        "boundaryPolicy":"shared_stream_boundary_v1",
                        "relativePath":relative_source(codex_home,logs_path),"physicalIdentity":physical,
                        "anchorKey":anchor_keys.get(&max_log_id),
                        "generation":generation,"eventNamespace":namespace})
                    .to_string(),
                ),
                updated_at: Utc::now(),
            },
            &candidate_cursors,
        )?;
    observe_batch(&mut report, outcome);
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
           AND id <= ?2
           AND target = 'codex_core::session::turn'
           AND instr(feedback_log_body, ' post sampling token usage ') > 0
           AND thread_id IS NOT NULL
         ORDER BY id"
    ))?;
    let rows = statement.query_map(
        params![
            i64::try_from(after_id).unwrap_or(i64::MAX),
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
            let observed_at = if (0..1_000_000_000).contains(&nanos) {
                DateTime::<Utc>::from_timestamp(seconds, nanos as u32)
            } else {
                None
            }
            .ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Integer,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid sampling timestamp; source cursor must not advance",
                    )),
                )
            })?;
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
                observed_at,
                thread_id,
                turn_id: extract_field(&body, "turn.id="),
                model: extract_field(&body, " model="),
            })
        },
    )?;
    let mut ready = Vec::new();
    for row in rows {
        let observation = row?;
        // Filtering timestamps in SQL can skip a lower row ID and permanently
        // lose it after a later (but older-timestamped) row advances the cursor.
        if observation.observed_at > safe_before {
            break;
        }
        ready.push(observation);
    }
    Ok(ready)
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

/// One bounded header read when upgrading a numeric-only cursor. It recovers
/// the creation timestamp, not a live/replay phase, and is never a full rescan.
fn read_canonical_header(
    path: &Path,
    thread_id: &str,
    bytes_read: &mut u64,
) -> Result<Option<DateTime<Utc>>> {
    let mut reader = BufReader::new(File::open(path)?.take(64 * 1024));
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = reader.read_until(b'\n', &mut line)?;
        *bytes_read = bytes_read.saturating_add(count as u64);
        if count == 0 || !line.ends_with(b"\n") {
            return Ok(None);
        }
        let raw = line.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&line);
        let Ok(record) = serde_json::from_slice::<Value>(raw) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) == Some("session_meta")
            && record.pointer("/payload/id").and_then(Value::as_str) == Some(thread_id)
        {
            return Ok(record_timestamp(&record));
        }
    }
}

fn read_usage_candidates(
    path: &Path,
    start_offset: u64,
    bytes_read: &mut u64,
    safe_before: DateTime<Utc>,
    counter: &mut CandidateCounterCheckpoint,
    thread_id: &str,
    is_child: bool,
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
        if extract_json_timestamp(&line)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|timestamp| timestamp > safe_before)
        {
            durable_offset = line_start;
            break;
        }
        durable_offset = reader.stream_position()?;
        if ![
            "token_count",
            "session_meta",
            "task_started",
            "turn_context",
        ]
        .iter()
        .any(|kind| line.contains(kind))
        {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if record_timestamp(&value).is_some_and(|timestamp| timestamp > safe_before) {
            durable_offset = line_start;
            break;
        }
        let (usage, unavailable_reason) = counter.observe_record(&value, thread_id, is_child);
        if value.get("type").and_then(Value::as_str) != Some("event_msg")
            || value.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
        {
            continue;
        }
        let Some(observed_at) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        else {
            continue;
        };
        candidates.push(UsageCandidate {
            record_digest: crate::reconstruction::source_record_digest(&value),
            byte_offset: line_start,
            observed_at,
            usage,
            unavailable_reason,
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
            source_record_key: None,
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

fn parse_candidate_usage(value: &Value) -> Option<TokenUsage> {
    let input_tokens = value.get("input_tokens")?.as_u64()?;
    let cache_write = match value.get("cache_write_input_tokens") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64()?),
    };
    let usage = TokenUsage {
        input_tokens,
        cached_input_tokens: value.get("cached_input_tokens")?.as_u64()?,
        cache_write_input_tokens: cache_write.unwrap_or_default(),
        cache_write_observed_input_tokens: cache_write.map_or(0, |_| input_tokens),
        output_tokens: value.get("output_tokens")?.as_u64()?,
        reasoning_output_tokens: value.get("reasoning_output_tokens")?.as_u64()?,
        total_tokens: value.get("total_tokens")?.as_u64()?,
    };
    usage.validate().ok()?;
    Some(usage)
}

fn timestamp_nanos(value: DateTime<Utc>) -> i128 {
    i128::from(value.timestamp()) * 1_000_000_000 + i128::from(value.timestamp_subsec_nanos())
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

    #[test]
    fn bounded_header_recovery_does_not_assume_a_live_stream() {
        let temporary = tempdir().unwrap();
        let path = temporary.path().join("legacy.jsonl");
        let at = Utc::now() - chrono::Duration::minutes(1);
        fs::write(&path,format!("{}\n",
            serde_json::json!({"timestamp":at.to_rfc3339(),"type":"session_meta","payload":{"id":"legacy"}}))).unwrap();
        let mut state = CandidateCounterCheckpoint::new(500);
        let mut bytes = 0;
        state.boundary.canonical_at = read_canonical_header(&path, "legacy", &mut bytes).unwrap();
        assert_eq!(state.boundary.canonical_at, Some(at));
        assert!(state.boundary.foreign_replay);
        let start = serde_json::json!({"type":"event_msg","payload":{"type":"task_started",
            "turn_id":"f1234567-89ab-4cde-8abc-0123456789ab","started_at":at.timestamp()}});
        state.observe_record(&start, "legacy", false);
        assert_eq!(state.boundary.phase, StreamPhase::Live);
        fs::write(&path, vec![b'x'; 100_000]).unwrap();
        bytes = 0;
        assert_eq!(
            read_canonical_header(&path, "legacy", &mut bytes).unwrap(),
            None
        );
        assert_eq!(bytes, 64 * 1024);
    }

    #[test]
    fn numeric_only_checkpoint_requires_a_proven_stream_resume() {
        let at = Utc::now();
        let mut value: Value = serde_json::from_str(&token_line(at, 100)).unwrap();
        let mut state: CandidateCounterCheckpoint = serde_json::from_value(serde_json::json!({
            "version":2,"previous_total":value["payload"]["info"]["last_token_usage"],
            "cumulative_seen":true,"allow_initial_sample":false
        }))
        .unwrap();
        let thread = "019b76da-a800-7000-8000-000000000000";
        value["payload"]["info"]["total_token_usage"] = serde_json::json!({
            "input_tokens":140,"cached_input_tokens":120,"output_tokens":10,
            "reasoning_output_tokens":3,"total_tokens":150
        });
        assert!(state.observe_record(&value, thread, true).0.is_none());
        let fake = serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"f1234567-89ab-4cde-8abc-0123456789ab"}});
        state.observe_record(&fake, thread, true);
        assert!(state.observe_record(&value, thread, true).0.is_none());
        let own = serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"019b76da-a900-7000-8000-000000000000"}});
        state.observe_record(&own, thread, true);
        // Unknown pre-resume observations advance only the baseline. They
        // cannot be assigned to the first identifiable post-resume request.
        for field in ["input_tokens", "cached_input_tokens", "total_tokens"] {
            value["payload"]["info"]["total_token_usage"][field] = serde_json::json!(
                value["payload"]["info"]["total_token_usage"][field]
                    .as_u64()
                    .unwrap()
                    + 10
            );
        }
        assert_eq!(
            state
                .observe_record(&value, thread, true)
                .0
                .unwrap()
                .total_tokens,
            10
        );
    }

    #[test]
    fn future_or_partial_boundary_records_do_not_advance_parser_state() {
        let temporary = tempdir().unwrap();
        let path = temporary.path().join("root.jsonl");
        let now = Utc::now();
        let meta = serde_json::json!({"timestamp":(now-chrono::Duration::seconds(60)).to_rfc3339(),"type":"session_meta","payload":{"id":"root"}});
        let future = serde_json::json!({"timestamp":(now+chrono::Duration::seconds(60)).to_rfc3339(),"type":"session_meta","payload":{"id":"parent"}}).to_string().replace("\":", "\": ");
        fs::write(&path, format!("{meta}\n{future}\n")).unwrap();
        let mut state = CandidateCounterCheckpoint::new(0);
        let mut bytes = 0;
        let (records, offset) =
            read_usage_candidates(&path, 0, &mut bytes, now, &mut state, "root", false).unwrap();
        assert!(records.is_empty());
        assert_eq!(offset, meta.to_string().len() as u64 + 1);
        assert_eq!(state.boundary.phase, StreamPhase::Live);
        assert!(!state.boundary.foreign_replay);
        fs::write(&path, format!("{meta}\n{future}")).unwrap();
        let (_, next) = read_usage_candidates(
            &path,
            offset,
            &mut bytes,
            now + chrono::Duration::seconds(120),
            &mut state,
            "root",
            false,
        )
        .unwrap();
        assert_eq!(next, offset);
        assert!(!state.boundary.foreign_replay);
    }

    #[test]
    fn inherited_candidates_remain_blocked_after_gap_and_parser_restart() {
        let temporary = tempdir().unwrap();
        let path = temporary.path().join("child.jsonl");
        let at = Utc::now() - chrono::Duration::minutes(2);
        let mut inherited: Value = serde_json::from_str(&token_line(at, 100)).unwrap();
        inherited["payload"]["info"]["total_token_usage"] =
            inherited["payload"]["info"]["last_token_usage"].clone();
        fs::write(&path, format!("{}\n{}\n{inherited}\n",
            serde_json::json!({"timestamp":at.to_rfc3339(),"type":"session_meta","payload":{"id":"child"}}),
            serde_json::json!({"timestamp":at.to_rfc3339(),"type":"session_meta","payload":{"id":"parent"}}),
        )).unwrap();
        let mut counter = CandidateCounterCheckpoint::new(0);
        let mut bytes = 0;
        let (first, offset) = read_usage_candidates(
            &path,
            0,
            &mut bytes,
            Utc::now(),
            &mut counter,
            "child",
            true,
        )
        .unwrap();
        assert_eq!(first.len(), 1);
        assert!(
            first[0].usage.is_none(),
            "copied ancestor quantity is not a child sample"
        );
        counter = serde_json::from_str(&serde_json::to_string(&counter).unwrap()).unwrap();
        let later = at + chrono::Duration::seconds(10);
        inherited["timestamp"] = serde_json::json!(later.to_rfc3339());
        inherited["payload"]["info"]["total_token_usage"]["input_tokens"] = serde_json::json!(190);
        inherited["payload"]["info"]["total_token_usage"]["cached_input_tokens"] =
            serde_json::json!(170);
        inherited["payload"]["info"]["total_token_usage"]["total_tokens"] = serde_json::json!(200);
        let mut own = inherited.clone();
        own["timestamp"] =
            serde_json::json!((later + chrono::Duration::milliseconds(100)).to_rfc3339());
        for field in ["input_tokens", "cached_input_tokens", "total_tokens"] {
            own["payload"]["info"]["total_token_usage"][field] = serde_json::json!(
                own["payload"]["info"]["total_token_usage"][field]
                    .as_u64()
                    .unwrap()
                    + 50
            );
        }
        {
            let mut append = OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(append,"{}\n{inherited}\n{}\n{own}",
                serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"f1234567-89ab-4cde-8abc-0123456789ab","started_at":1}}),
                serde_json::json!({"type":"event_msg","payload":{"type":"task_started","started_at":later.timestamp()}}),
            ).unwrap();
        }
        let (next, _) = read_usage_candidates(
            &path,
            offset,
            &mut bytes,
            Utc::now(),
            &mut counter,
            "child",
            true,
        )
        .unwrap();
        assert_eq!(next.len(), 2);
        assert!(next[0].usage.is_none());
        assert_eq!(next[1].usage.unwrap().total_tokens, 50);
    }

    #[test]
    fn counter_gaps_and_legacy_cursor_do_not_invent_continuity() {
        let at = Utc::now();
        let mut line: Value = serde_json::from_str(&token_line(at, 100)).unwrap();
        let mut state = CandidateCounterCheckpoint::new(500);
        assert!(
            state.normalize(&line).0.is_none(),
            "tail without a baseline is not a legacy whole stream"
        );
        line["payload"]["info"]["total_token_usage"] =
            line["payload"]["info"]["last_token_usage"].clone();
        assert!(
            state.normalize(&line).0.is_none(),
            "first cumulative value establishes only a baseline"
        );
        line["payload"]["info"]["total_token_usage"] = Value::Null;
        assert!(
            state.normalize(&line).0.is_none(),
            "malformed total cannot fall back to a valid last"
        );
        line["payload"]["info"]["total_token_usage"] =
            line["payload"]["info"]["last_token_usage"].clone();
        assert!(
            state.normalize(&line).0.is_none(),
            "must not bridge a broken interval"
        );
        assert_eq!(
            state.normalize(&line).1,
            Some("post_sampling_counter_unchanged")
        );
    }

    #[test]
    fn sampling_counter_cursor_failure_rolls_back_events_and_log_cursor() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let prior_log = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        let prior_candidate = store
            .get_cursor("machine", "sampling-rollout:thread-1")
            .unwrap()
            .unwrap();
        let next_at = at + chrono::Duration::seconds(10);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(next_at, 100)
        )
        .unwrap();
        let logs = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&logs, next_at, "atomic-counter");
        store.connection().execute_batch("CREATE TEMP TRIGGER fail_candidate BEFORE UPDATE ON file_cursors
            WHEN NEW.source_id='sampling-rollout:thread-1' BEGIN SELECT RAISE(ABORT, 'synthetic cursor failure'); END;").unwrap();
        assert!(ingest_post_sampling(&mut store, temporary.path(), "machine").is_err());
        assert_eq!(
            store
                .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
                .unwrap()
                .unwrap()
                .byte_offset,
            prior_log.byte_offset
        );
        let candidate = store
            .get_cursor("machine", "sampling-rollout:thread-1")
            .unwrap()
            .unwrap();
        assert_eq!(candidate.byte_offset, prior_candidate.byte_offset);
        assert_eq!(
            candidate.parser_state_json,
            prior_candidate.parser_state_json
        );
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        let count: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE event_id='logs2-post-sampling:2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        store
            .connection()
            .execute_batch("DROP TRIGGER fail_candidate")
            .unwrap();
        ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            350
        );
        assert_eq!(
            ingest_post_sampling(&mut store, temporary.path(), "machine")
                .unwrap()
                .bytes_read,
            0
        );
    }

    #[test]
    fn repeated_counters_and_stale_last_usage_do_not_add_new_consumption() {
        let (temporary, _old_store, at, rollout) = copied_source_fixture();
        let ledger = temporary.path().join("normalized.sqlite3");
        let mut store = LedgerStore::open(&ledger).unwrap();
        ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        let base = at + chrono::Duration::seconds(10);
        let mut first: Value = serde_json::from_str(&token_line(base, 100)).unwrap();
        first["payload"]["info"]["total_token_usage"] =
            first["payload"]["info"]["last_token_usage"].clone();
        let mut repeat = first.clone();
        repeat["timestamp"] =
            serde_json::json!((base + chrono::Duration::milliseconds(10)).to_rfc3339());
        {
            let mut append = OpenOptions::new().append(true).open(&rollout).unwrap();
            writeln!(append, "{first}\n{repeat}").unwrap();
        }
        let logs = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&logs, base + chrono::Duration::milliseconds(11), "repeat");
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((report.matched, report.unmatched), (0, 1));
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        drop(store);

        // Restart must restore the previous cumulative snapshot. A stale last
        // value of 900 cannot override the actual 50-token counter increment.
        let mut store = LedgerStore::open(&ledger).unwrap();
        let next_at = base + chrono::Duration::seconds(1);
        let mut next: Value = serde_json::from_str(&token_line(next_at, 900)).unwrap();
        next["payload"]["info"]["total_token_usage"] = serde_json::json!({
            "input_tokens":130,"cached_input_tokens":90,"output_tokens":20,
            "reasoning_output_tokens":6,"total_tokens":150
        });
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{next}"
        )
        .unwrap();
        insert_log(&logs, next_at, "increment");
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((report.matched, report.unmatched), (1, 0));
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            300
        );

        next["timestamp"] =
            serde_json::json!((next_at + chrono::Duration::seconds(1)).to_rfc3339());
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{next}"
        )
        .unwrap();
        insert_log(
            &logs,
            next_at + chrono::Duration::seconds(1),
            "repeat-after-restart",
        );
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((report.matched, report.unmatched), (0, 1));
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            300
        );
        let idle = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((idle.observations, idle.bytes_read), (0, 0));
    }

    #[test]
    fn missing_or_malformed_usage_is_not_a_confirmed_zero() {
        assert!(parse_candidate_usage(&Value::Null).is_none());
        assert!(parse_candidate_usage(&serde_json::json!({})).is_none());
        let good = serde_json::json!({"input_tokens":0,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":0});
        assert_eq!(parse_candidate_usage(&good), Some(TokenUsage::default()));
        for field in [
            "input_tokens",
            "cached_input_tokens",
            "output_tokens",
            "reasoning_output_tokens",
            "total_tokens",
        ] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(parse_candidate_usage(&bad).is_none());
            bad = good.clone();
            bad[field] = serde_json::json!(-1);
            assert!(parse_candidate_usage(&bad).is_none());
        }
        let mut bad = good;
        bad["cache_write_input_tokens"] = serde_json::json!("invalid");
        assert!(parse_candidate_usage(&bad).is_none());
    }

    #[test]
    fn invalid_nearest_snapshot_does_not_fall_back_to_an_older_amount() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let base = at + chrono::Duration::seconds(30);
        let mut append = OpenOptions::new().append(true).open(&rollout).unwrap();
        writeln!(append, "{}", token_line(base, 900)).unwrap();
        let mut bad: Value =
            serde_json::from_str(&token_line(base + chrono::Duration::milliseconds(10), 100))
                .unwrap();
        bad["payload"]["info"]["last_token_usage"]["total_tokens"] = serde_json::json!(999);
        writeln!(append, "{bad}").unwrap();
        drop(append);
        let logs = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(
            &logs,
            base + chrono::Duration::milliseconds(11),
            "invalid-neighbor",
        );
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((report.matched, report.unmatched), (0, 1));
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        let quality: String = store
            .connection()
            .query_row(
                "SELECT quality FROM retained_request_evidence WHERE turn_id='invalid-neighbor'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(quality, "unknown");
    }

    #[test]
    fn matching_is_mutual_and_never_uses_a_farther_candidate_after_consumption() {
        let ms = 1_000_000;
        assert_eq!(
            mutual_matches(&[0, 20 * ms], &[15 * ms, 150 * ms]),
            vec![Nearest::Ambiguous, Nearest::Unique(0)]
        );
        assert_eq!(
            mutual_matches(&[0, 10 * ms], &[5 * ms]),
            vec![Nearest::Ambiguous, Nearest::Ambiguous]
        );
        assert_eq!(
            mutual_matches(&[0, 0], &[0]),
            vec![Nearest::Ambiguous, Nearest::Ambiguous]
        );
        assert_eq!(mutual_matches(&[0], &[0, 0]), vec![Nearest::Ambiguous]);
        assert_eq!(mutual_matches(&[0], &[250 * ms]), vec![Nearest::Unique(0)]);
        assert_eq!(
            mutual_matches(&[0], &[250 * ms + 1]),
            vec![Nearest::Missing]
        );
        assert_eq!(nearest(&[i128::MAX], i128::MIN), Nearest::Missing);
        assert_eq!(nearest(&[], 0), Nearest::Missing);
        let distant = "2400-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            nearest(
                &[timestamp_nanos(distant + chrono::Duration::seconds(1))],
                timestamp_nanos(distant)
            ),
            Nearest::Missing
        );
        assert_eq!(
            nearest(
                &[timestamp_nanos(
                    distant + chrono::Duration::milliseconds(250)
                )],
                timestamp_nanos(distant)
            ),
            Nearest::Unique(0)
        );
    }

    #[test]
    fn ambiguous_anchor_does_not_import_an_unrelated_amount_or_a_context_counter() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let base = at + chrono::Duration::seconds(30);
        let mut append = OpenOptions::new().append(true).open(&rollout).unwrap();
        writeln!(
            append,
            "{}",
            token_line(base + chrono::Duration::milliseconds(15), 300)
        )
        .unwrap();
        writeln!(
            append,
            "{}",
            token_line(base + chrono::Duration::milliseconds(150), 900)
        )
        .unwrap();
        drop(append);
        let logs = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&logs, base, "earlier-anchor");
        insert_log(
            &logs,
            base + chrono::Duration::milliseconds(20),
            "nearer-anchor",
        );
        logs.execute("UPDATE logs SET feedback_log_body=replace(feedback_log_body,'total_usage_tokens=100','total_usage_tokens=9000000000000') WHERE id>1",[]).unwrap();
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!(
            (report.observations, report.matched, report.unmatched),
            (2, 1, 1)
        );
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            550
        );
        let rows:Vec<(Option<String>,String,i64)>=store.connection().prepare("SELECT turn_id,quality,total_tokens FROM retained_request_evidence WHERE turn_id IN ('earlier-anchor','nearer-anchor') ORDER BY turn_id")
            .unwrap().query_map([],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        assert_eq!(
            rows,
            vec![
                (Some("earlier-anchor".into()), "unknown".into(), 0),
                (Some("nearer-anchor".into()), "confirmed".into(), 300)
            ]
        );
        let unknown_link:i64=store.connection().query_row("SELECT COUNT(*) FROM sampling_candidate_links l JOIN retained_request_evidence r USING(event_id) WHERE r.turn_id='earlier-anchor'",[],|r|r.get(0)).unwrap();
        assert_eq!(unknown_link, 0);
        let next = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!((next.observations, next.bytes_read), (0, 0));
    }

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
    fn maturity_watermark_keeps_out_of_order_rows_pending() {
        let (temporary, _store, at, _rollout) = copied_source_fixture();
        let connection = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&connection, at + chrono::Duration::seconds(10), "pending");
        insert_log(
            &connection,
            at + chrono::Duration::seconds(2),
            "later-id-older-time",
        );
        let rows = read_observation_rows(
            &connection,
            0,
            None,
            at + chrono::Duration::seconds(5),
            "machine",
        )
        .unwrap();
        assert_eq!(rows.iter().map(|row| row.log_id).collect::<Vec<_>>(), [1]);
        let resumed = read_observation_rows(
            &connection,
            1,
            None,
            at + chrono::Duration::seconds(11),
            "machine",
        )
        .unwrap();
        assert_eq!(
            resumed.iter().map(|row| row.log_id).collect::<Vec<_>>(),
            [2, 3]
        );
    }

    #[test]
    fn mature_rows_with_reversed_timestamps_match_without_skipping_candidates() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let connection = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        for (seconds, tokens) in [(6, 180), (4, 200)] {
            let time = at + chrono::Duration::seconds(seconds);
            writeln!(
                OpenOptions::new().append(true).open(&rollout).unwrap(),
                "{}",
                token_line(time, tokens)
            )
            .unwrap();
            insert_log(&connection, time, &format!("out-of-order-{seconds}"));
        }
        let report = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        assert_eq!(report.matched, 2);
        assert_eq!(report.unmatched, 0);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            630
        );
        assert_eq!(
            store
                .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
                .unwrap()
                .unwrap()
                .byte_offset,
            3
        );
    }

    #[test]
    fn invalid_appended_timestamp_keeps_committed_cursor_and_usage() {
        let (temporary, mut store, at, rollout) = copied_source_fixture();
        let checkpoint = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        let valid_at = at + chrono::Duration::seconds(5);
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{}",
            token_line(valid_at, 180)
        )
        .unwrap();
        let connection = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&connection, valid_at, "valid-before-invalid");
        insert_log(
            &connection,
            at + chrono::Duration::seconds(6),
            "invalid-appended",
        );
        connection
            .execute("UPDATE logs SET ts_nanos=-1 WHERE id=3", [])
            .unwrap();
        assert!(ingest_post_sampling(&mut store, temporary.path(), "machine").is_err());
        let preserved = store
            .get_cursor("machine", POST_SAMPLING_SOURCE_ID)
            .unwrap()
            .unwrap();
        assert_eq!(preserved, checkpoint);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
    }

    #[test]
    fn source_timestamp_is_exact_and_invalid_time_does_not_become_now() {
        let (temporary, _store, at, _rollout) = copied_source_fixture();
        let connection = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        let rows = read_observation_rows(
            &connection,
            0,
            None,
            at - chrono::Duration::nanoseconds(1),
            "machine",
        )
        .unwrap();
        assert!(rows.is_empty());
        for (seconds, nanos) in [
            (at.timestamp(), -1),
            (at.timestamp(), 1_000_000_000),
            (i64::MIN, 0),
        ] {
            connection
                .execute("UPDATE logs SET ts=?1,ts_nanos=?2", params![seconds, nanos])
                .unwrap();
            assert!(read_observation_rows(&connection, 0, None, Utc::now(), "machine").is_err());
        }
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
    fn sampling_and_reconstruction_share_the_same_source_record_evidence() {
        let (temporary, _old_store, at, old_rollout) = copied_source_fixture();
        let rollout = temporary.path().join("sessions/rollout.jsonl");
        fs::create_dir_all(rollout.parent().unwrap()).unwrap();
        fs::rename(old_rollout, &rollout).unwrap();
        for state in ["state_5.sqlite", "sqlite/state_5.sqlite"] {
            let state = Connection::open(temporary.path().join(state)).unwrap();
            state
                .execute_batch(
                    "ALTER TABLE threads ADD COLUMN title TEXT NOT NULL DEFAULT 'Synthetic';
                ALTER TABLE threads ADD COLUMN created_at INTEGER NOT NULL DEFAULT 1788220800;
                ALTER TABLE threads ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 1788220800;
                ALTER TABLE threads ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE threads ADD COLUMN has_user_event INTEGER NOT NULL DEFAULT 1;
                ALTER TABLE threads ADD COLUMN git_origin_url TEXT;",
                )
                .unwrap();
            state
                .execute(
                    "UPDATE threads SET rollout_path=?1",
                    [rollout.to_string_lossy().as_ref()],
                )
                .unwrap();
        }
        let mut first: Value = serde_json::from_str(&token_line(at, 100)).unwrap();
        first["payload"]["info"]["total_token_usage"] =
            first["payload"]["info"]["last_token_usage"].clone();
        // The retained file starts after an older counter prefix. Both sources
        // must still pair only the identifiable sample, not the initial total.
        for field in ["input_tokens", "cached_input_tokens", "total_tokens"] {
            first["payload"]["info"]["total_token_usage"][field] = serde_json::json!(
                first["payload"]["info"]["total_token_usage"][field]
                    .as_u64()
                    .unwrap()
                    + 1000
            );
        }
        let mut second: Value =
            serde_json::from_str(&token_line(at + chrono::Duration::seconds(1), 150)).unwrap();
        second["payload"]["info"]["total_token_usage"] = serde_json::json!({
            "input_tokens":1230,"cached_input_tokens":1190,"output_tokens":20,"reasoning_output_tokens":6,"total_tokens":1250
        });
        // Numeric lineage must agree even when the last-usage snapshot is stale.
        let stale: Value = serde_json::from_str(&token_line(at, 900)).unwrap();
        second["payload"]["info"]["last_token_usage"] =
            stale["payload"]["info"]["last_token_usage"].clone();
        let inherited_usage = serde_json::json!({"input_tokens":1000,"cached_input_tokens":1000,
            "output_tokens":0,"reasoning_output_tokens":0,"total_tokens":1000});
        let inherited = serde_json::json!({"timestamp":(at-chrono::Duration::milliseconds(500)).to_rfc3339(),
            "type":"event_msg","payload":{"type":"token_count","info":{
                "total_token_usage":inherited_usage,"last_token_usage":inherited_usage}}});
        fs::write(&rollout, format!("{}\n{}\n{inherited}\n{}\n{}\n{}\n{}\n",
            serde_json::json!({"timestamp":(at-chrono::Duration::seconds(1)).to_rfc3339(),"type":"session_meta","payload":{"id":"thread-1"}}),
            serde_json::json!({"type":"session_meta","payload":{"id":"ancestor"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"task_started","started_at":at.timestamp()}}),
            serde_json::json!({"type":"turn_context","payload":{"model":"gpt-5.6-sol"}}), first, second)).unwrap();
        let ledger_path = temporary.path().join("ledger.sqlite3");
        let mut store = LedgerStore::open(&ledger_path).unwrap();
        crate::runtime::sync_native_catalog(&mut store, temporary.path()).unwrap();
        ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        let reconstructed = crate::reconstruction::ingest_reconstruction_batch(
            &mut store,
            temporary.path(),
            "machine",
            8,
        )
        .unwrap();
        assert_eq!(reconstructed.inserted_events, 2, "{reconstructed:?}");
        let matched: i64 = store.connection().query_row(
            "SELECT COUNT(*) FROM (SELECT record_key FROM source_record_evidence GROUP BY record_key HAVING COUNT(DISTINCT evidence_source)=2)",
            [], |row| row.get(0)).unwrap();
        assert_eq!(matched, 2);
        let shadow = store
            .shadow_source_union("thread-1", at, at + chrono::Duration::seconds(10), 100)
            .unwrap();
        assert!(
            shadow.complete_for_supplied_records,
            "{:?}",
            shadow.unresolved
        );
        assert_eq!(shadow.shared_records_collapsed, 2);
        assert_eq!(shadow.usage.unwrap().total_tokens, 250);
        drop(store);
        let mut store = LedgerStore::open(&ledger_path).unwrap();
        let idle_sampling = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        let idle_reconstruction = crate::reconstruction::ingest_reconstruction_batch(
            &mut store,
            temporary.path(),
            "machine",
            8,
        )
        .unwrap();
        assert_eq!(idle_sampling.observations, 0);
        assert_eq!(idle_sampling.bytes_read, 0);
        assert_eq!(idle_reconstruction.inserted_events, 0);
        assert_eq!(idle_reconstruction.bytes_read, 0);
        assert_eq!(
            store
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            250
        );
        let third_at = at + chrono::Duration::seconds(2);
        let mut third: Value = serde_json::from_str(&token_line(third_at, 180)).unwrap();
        let mut cumulative = second["payload"]["info"]["total_token_usage"].clone();
        for field in [
            "input_tokens",
            "cached_input_tokens",
            "output_tokens",
            "reasoning_output_tokens",
            "total_tokens",
        ] {
            cumulative[field] = serde_json::json!(
                cumulative[field].as_u64().unwrap()
                    + third["payload"]["info"]["last_token_usage"][field]
                        .as_u64()
                        .unwrap()
            );
        }
        third["payload"]["info"]["total_token_usage"] = cumulative;
        writeln!(
            OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{third}"
        )
        .unwrap();
        let logs = Connection::open(temporary.path().join("logs_2.sqlite")).unwrap();
        insert_log(&logs, third_at, "after-ledger-reopen");
        drop(logs);
        let appended = ingest_post_sampling(&mut store, temporary.path(), "machine").unwrap();
        let rebuilt = crate::reconstruction::ingest_reconstruction_batch(
            &mut store,
            temporary.path(),
            "machine",
            8,
        )
        .unwrap();
        assert_eq!(appended.inserted_events, 1);
        assert_eq!(rebuilt.inserted_events, 1);
        let shadow = store
            .shadow_source_union("thread-1", at, at + chrono::Duration::seconds(10), 100)
            .unwrap();
        assert!(
            shadow.complete_for_supplied_records,
            "{:?}",
            shadow.unresolved
        );
        assert_eq!(shadow.shared_records_collapsed, 3);
        assert_eq!(shadow.usage.unwrap().total_tokens, 430);
        let expected = shadow.usage.unwrap();
        for dimension in [
            crate::store::AggregateDimension::Account,
            crate::store::AggregateDimension::Project,
            crate::store::AggregateDimension::Model,
            crate::store::AggregateDimension::Thread,
            crate::store::AggregateDimension::Day,
        ] {
            let buckets = store
                .aggregate_exact_time_series(
                    crate::store::TimeGrain::Day,
                    Some(dimension),
                    &AggregateFilter::default(),
                    "Asia/Shanghai",
                )
                .unwrap();
            let expected = serde_json::to_value(expected).unwrap();
            for field in [
                "input_tokens",
                "cached_input_tokens",
                "cache_write_input_tokens",
                "cache_write_observed_input_tokens",
                "output_tokens",
                "reasoning_output_tokens",
                "total_tokens",
            ] {
                let sum: u64 = buckets
                    .iter()
                    .map(|bucket| {
                        serde_json::to_value(bucket.usage).unwrap()[field]
                            .as_u64()
                            .unwrap_or(0)
                    })
                    .sum();
                assert_eq!(
                    sum,
                    expected[field].as_u64().unwrap_or(0),
                    "{dimension:?}/{field}"
                );
            }
        }
        let digest = crate::reconstruction::source_record_digest(&first);
        second = first.clone();
        second["timestamp"] = serde_json::json!("2020-01-01T00:00:00Z");
        assert_ne!(digest, crate::reconstruction::source_record_digest(&second));
        let key = crate::reconstruction::source_record_key("machine", "file", "thread", 1, &digest);
        assert_ne!(
            key,
            crate::reconstruction::source_record_key("machine", "file", "thread", 2, &digest)
        );
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
