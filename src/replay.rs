//! Replay-safe rollout interpretation.
//!
//! Modern subagent rollouts can contain a canonical child `session_meta`
//! followed by a replay of the parent's full JSONL history. The replayed
//! records are written with fresh outer timestamps, so blindly accepting each
//! `session_meta`, `turn_context`, and `token_count` both changes ownership and
//! counts the parent's tokens again. This module fixes the first canonical
//! identity and quarantines that replay segment until the child's own turn
//! begins.

use crate::ingest::JsonlLine;
use crate::types::{
    AttributionConfidence, DataQuality, EventProvenance, ProjectAttribution, TokenUsage, UsageEvent,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const REPLAY_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayConfig {
    pub machine_id: String,
    pub source_id: String,
    pub file_identity: String,
    pub account_fingerprint: Option<String>,
    pub account_confidence: AttributionConfidence,
    pub project: ProjectAttribution,
}

impl ReplayConfig {
    pub fn new(
        machine_id: impl Into<String>,
        source_id: impl Into<String>,
        file_identity: impl Into<String>,
    ) -> Self {
        Self {
            machine_id: machine_id.into(),
            source_id: source_id.into(),
            file_identity: file_identity.into(),
            account_fingerprint: None,
            account_confidence: AttributionConfidence::Unknown,
            project: ProjectAttribution {
                project_id: None,
                project_name: None,
                confidence: AttributionConfidence::Unknown,
                method: "unassigned".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalRollout {
    pub rollout_id: String,
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub initial_cwd: Option<String>,
    pub source_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPhase {
    #[default]
    AwaitingCanonical,
    Live,
    ReplayingForeignHistory,
}

/// Durable replay/accounting state paired with a JSONL tail checkpoint.
///
/// Callers must commit this checkpoint atomically with the corresponding
/// `TailCheckpoint`: advancing only the byte offset would lose canonical
/// identity, replay phase, context, and cumulative-counter history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCheckpoint {
    #[serde(default = "current_replay_checkpoint_schema_version")]
    pub schema_version: u32,
    /// Binds state to one physical JSONL file. Missing is accepted for v1
    /// compatibility with early checkpoints, but newly emitted values set it.
    #[serde(default)]
    pub file_identity: Option<String>,
    #[serde(default)]
    pub canonical: Option<CanonicalRollout>,
    #[serde(default)]
    pub phase: ReplayPhase,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub previous_total: Option<TokenUsage>,
    #[serde(default)]
    pub counter_epoch: u64,
}

impl Default for ReplayCheckpoint {
    fn default() -> Self {
        Self {
            schema_version: REPLAY_CHECKPOINT_SCHEMA_VERSION,
            file_identity: None,
            canonical: None,
            phase: ReplayPhase::AwaitingCanonical,
            model: None,
            cwd: None,
            previous_total: None,
            counter_epoch: 0,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReplayCheckpointError {
    #[error("unsupported replay checkpoint schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("replay checkpoint belongs to file {checkpoint}, not {configured}")]
    FileIdentityMismatch {
        checkpoint: String,
        configured: String,
    },
    #[error("replay checkpoint phase requires a canonical rollout")]
    MissingCanonicalRollout,
}

const fn current_replay_checkpoint_schema_version() -> u32 {
    REPLAY_CHECKPOINT_SCHEMA_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordDisposition {
    Ignored,
    Malformed,
    CanonicalSessionEstablished,
    ForeignSessionQuarantined,
    DuplicateSessionQuarantined,
    ReplayRecordQuarantined,
    ReplayEnded,
    ContextUpdated,
    ConfirmedUsage,
    QuarantinedUsage,
    CounterUnchanged,
    CounterReset,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenSample {
    pub total: Option<TokenUsage>,
    pub last: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayOutcome {
    pub disposition: RecordDisposition,
    pub event: Option<UsageEvent>,
    pub sample: Option<TokenSample>,
    pub reason: Option<String>,
}

impl ReplayOutcome {
    fn plain(disposition: RecordDisposition) -> Self {
        Self {
            disposition,
            event: None,
            sample: None,
            reason: None,
        }
    }

    fn reason(disposition: RecordDisposition, reason: impl Into<String>) -> Self {
        Self {
            disposition,
            event: None,
            sample: None,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplayGuard {
    config: ReplayConfig,
    canonical: Option<CanonicalRollout>,
    phase: ReplayPhase,
    model: Option<String>,
    cwd: Option<String>,
    previous_total: Option<TokenUsage>,
    counter_epoch: u64,
}

impl ReplayGuard {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            config,
            canonical: None,
            phase: ReplayPhase::AwaitingCanonical,
            model: None,
            cwd: None,
            previous_total: None,
            counter_epoch: 0,
        }
    }

    pub fn from_checkpoint(
        config: ReplayConfig,
        checkpoint: ReplayCheckpoint,
    ) -> Result<Self, ReplayCheckpointError> {
        if checkpoint.schema_version == 0
            || checkpoint.schema_version > REPLAY_CHECKPOINT_SCHEMA_VERSION
        {
            return Err(ReplayCheckpointError::UnsupportedSchemaVersion(
                checkpoint.schema_version,
            ));
        }
        if let Some(file_identity) = checkpoint.file_identity.as_deref()
            && file_identity != config.file_identity
        {
            return Err(ReplayCheckpointError::FileIdentityMismatch {
                checkpoint: file_identity.to_owned(),
                configured: config.file_identity,
            });
        }
        if checkpoint.canonical.is_none() && checkpoint.phase != ReplayPhase::AwaitingCanonical {
            return Err(ReplayCheckpointError::MissingCanonicalRollout);
        }
        Ok(Self {
            config,
            canonical: checkpoint.canonical,
            phase: checkpoint.phase,
            model: checkpoint.model,
            cwd: checkpoint.cwd,
            previous_total: checkpoint.previous_total,
            counter_epoch: checkpoint.counter_epoch,
        })
    }

    pub fn checkpoint(&self) -> ReplayCheckpoint {
        ReplayCheckpoint {
            schema_version: REPLAY_CHECKPOINT_SCHEMA_VERSION,
            file_identity: Some(self.config.file_identity.clone()),
            canonical: self.canonical.clone(),
            phase: self.phase,
            model: self.model.clone(),
            cwd: self.cwd.clone(),
            previous_total: self.previous_total,
            counter_epoch: self.counter_epoch,
        }
    }

    pub fn canonical(&self) -> Option<&CanonicalRollout> {
        self.canonical.as_ref()
    }

    pub fn phase(&self) -> ReplayPhase {
        self.phase
    }

    pub fn counter_epoch(&self) -> u64 {
        self.counter_epoch
    }

    pub fn current_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn current_cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub fn process_line(&mut self, line: &JsonlLine, observed_at: DateTime<Utc>) -> ReplayOutcome {
        if line.is_blank() {
            return ReplayOutcome::plain(RecordDisposition::Ignored);
        }
        let record = match line.parse_json() {
            Ok(record) => record,
            Err(error) => {
                return ReplayOutcome::reason(
                    RecordDisposition::Malformed,
                    format!("invalid JSONL record: {error}"),
                );
            }
        };
        self.process_value(&record, line, observed_at)
    }

    pub fn process_value(
        &mut self,
        record: &Value,
        line: &JsonlLine,
        observed_at: DateTime<Utc>,
    ) -> ReplayOutcome {
        let record_type = record
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match record_type {
            "session_meta" => self.process_session_meta(record),
            "turn_context" => self.process_turn_context(record),
            "event_msg"
                if record.pointer("/payload/type").and_then(Value::as_str)
                    == Some("task_started") =>
            {
                self.process_task_started(record)
            }
            "event_msg"
                if record.pointer("/payload/type").and_then(Value::as_str)
                    == Some("token_count") =>
            {
                self.process_token_count(record, line, observed_at)
            }
            _ if self.phase == ReplayPhase::ReplayingForeignHistory => ReplayOutcome::reason(
                RecordDisposition::ReplayRecordQuarantined,
                "record belongs to a foreign replay segment",
            ),
            _ => ReplayOutcome::plain(RecordDisposition::Ignored),
        }
    }

    fn process_session_meta(&mut self, record: &Value) -> ReplayOutcome {
        let payload = record.get("payload").and_then(Value::as_object);
        let Some(session_id) = payload
            .and_then(|payload| payload.get("id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            return ReplayOutcome::reason(
                RecordDisposition::Malformed,
                "session_meta is missing payload.id",
            );
        };

        if self.canonical.is_none() {
            let source_timestamp = parse_source_timestamp(record);
            let initial_cwd = payload
                .and_then(|payload| payload.get("cwd"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let parent_thread_id = record
                .pointer("/payload/source/subagent/thread_spawn/parent_thread_id")
                .and_then(Value::as_str)
                .or_else(|| {
                    record
                        .pointer("/payload/parent_thread_id")
                        .and_then(Value::as_str)
                })
                .map(str::to_owned);
            self.cwd = initial_cwd.clone();
            self.canonical = Some(CanonicalRollout {
                rollout_id: session_id.to_owned(),
                thread_id: session_id.to_owned(),
                parent_thread_id,
                initial_cwd,
                source_timestamp,
            });
            self.phase = ReplayPhase::Live;
            return ReplayOutcome::plain(RecordDisposition::CanonicalSessionEstablished);
        }

        let canonical_id = self
            .canonical
            .as_ref()
            .map(|canonical| canonical.rollout_id.as_str())
            .unwrap_or_default();
        if session_id == canonical_id {
            ReplayOutcome::reason(
                RecordDisposition::DuplicateSessionQuarantined,
                format!("duplicate canonical session_meta for {session_id}"),
            )
        } else {
            self.phase = ReplayPhase::ReplayingForeignHistory;
            ReplayOutcome::reason(
                RecordDisposition::ForeignSessionQuarantined,
                format!(
                    "foreign session_meta {session_id} cannot replace canonical {canonical_id}"
                ),
            )
        }
    }

    fn process_task_started(&mut self, record: &Value) -> ReplayOutcome {
        if self.phase != ReplayPhase::ReplayingForeignHistory {
            return ReplayOutcome::plain(RecordDisposition::Ignored);
        }
        if !self.task_belongs_to_canonical_stream(record) {
            return ReplayOutcome::reason(
                RecordDisposition::ReplayRecordQuarantined,
                "task_started predates the canonical rollout",
            );
        }

        self.phase = ReplayPhase::Live;
        self.model = None;
        self.cwd = self
            .canonical
            .as_ref()
            .and_then(|canonical| canonical.initial_cwd.clone());
        ReplayOutcome::plain(RecordDisposition::ReplayEnded)
    }

    fn task_belongs_to_canonical_stream(&self, record: &Value) -> bool {
        let Some(canonical) = self.canonical.as_ref() else {
            return false;
        };
        let turn_id = record
            .pointer("/payload/turn_id")
            .and_then(Value::as_str)
            .and_then(uuid_v7_millis_prefix);
        let rollout_id = uuid_v7_millis_prefix(&canonical.rollout_id);
        if matches!((turn_id, rollout_id), (Some(turn), Some(rollout)) if turn >= rollout) {
            return true;
        }

        let started_at_ms = record
            .pointer("/payload/started_at")
            .and_then(json_u64)
            .map(|seconds| seconds.saturating_mul(1_000));
        let canonical_ms = canonical
            .source_timestamp
            .as_ref()
            .and_then(|timestamp| u64::try_from(timestamp.timestamp_millis()).ok());
        matches!(
            (started_at_ms, canonical_ms),
            (Some(started), Some(canonical)) if started.saturating_add(2_000) >= canonical
        )
    }

    fn process_turn_context(&mut self, record: &Value) -> ReplayOutcome {
        if self.phase != ReplayPhase::Live {
            return ReplayOutcome::reason(
                RecordDisposition::ReplayRecordQuarantined,
                "turn_context occurred outside the canonical live stream",
            );
        }
        let payload = record.get("payload").and_then(Value::as_object);
        if let Some(model) = payload
            .and_then(|payload| payload.get("model"))
            .and_then(Value::as_str)
            .filter(|model| !model.is_empty())
        {
            self.model = Some(model.to_owned());
        }
        if let Some(cwd) = payload
            .and_then(|payload| payload.get("cwd"))
            .and_then(Value::as_str)
            .filter(|cwd| !cwd.is_empty())
        {
            self.cwd = Some(cwd.to_owned());
        }
        ReplayOutcome::plain(RecordDisposition::ContextUpdated)
    }

    fn process_token_count(
        &mut self,
        record: &Value,
        line: &JsonlLine,
        observed_at: DateTime<Utc>,
    ) -> ReplayOutcome {
        let sample = parse_token_sample(record);
        if sample.total.is_none() && sample.last.is_none() {
            return ReplayOutcome::reason(
                RecordDisposition::Malformed,
                "token_count has neither total_token_usage nor last_token_usage",
            );
        }

        if self.phase != ReplayPhase::Live {
            let usage = sample.last.or(sample.total).unwrap_or_default();
            let reason = if self.phase == ReplayPhase::ReplayingForeignHistory {
                "token_count belongs to replayed foreign history"
            } else {
                "token_count was observed before canonical session_meta"
            };
            let event = self.make_event(
                record,
                line,
                observed_at,
                usage,
                DataQuality::Quarantined,
                Some(reason.to_string()),
            );
            return ReplayOutcome {
                disposition: RecordDisposition::ReplayRecordQuarantined,
                event: Some(event),
                sample: Some(sample),
                reason: Some(reason.to_string()),
            };
        }

        let previous = self.previous_total;
        let reset = matches!(
            (sample.total, previous),
            (Some(total), Some(previous)) if total.checked_delta(previous).is_none()
        );
        if matches!((sample.total, previous), (Some(total), Some(previous)) if total == previous) {
            return ReplayOutcome {
                disposition: RecordDisposition::CounterUnchanged,
                event: None,
                sample: Some(sample),
                reason: Some(
                    "cumulative counter is unchanged; repeated last usage is not counted".into(),
                ),
            };
        }

        let cumulative_delta = match (sample.total, previous) {
            (Some(total), Some(previous)) if !reset => total.checked_delta(previous),
            _ => None,
        };
        if let Some(total) = sample.total {
            self.previous_total = Some(total);
        }
        if reset {
            self.counter_epoch += 1;
        }

        let (usage, quality, quality_reason) = match sample.last {
            Some(last) if last.validate().is_ok() => {
                let reason = if reset {
                    Some(format!(
                        "cumulative counter reset; explicit last_token_usage starts epoch {}",
                        self.counter_epoch
                    ))
                } else if cumulative_delta.is_some_and(|delta| delta != last) {
                    Some(
                        "cumulative delta differs from explicit last_token_usage; explicit delta retained"
                            .to_string(),
                    )
                } else {
                    None
                };
                (last, DataQuality::Confirmed, reason)
            }
            Some(last) => {
                let reason = last
                    .validate()
                    .expect_err("invalid usage must have an invariant error");
                (
                    last,
                    DataQuality::Quarantined,
                    Some(format!("invalid last_token_usage: {reason}")),
                )
            }
            None => {
                let derived = cumulative_delta.or(sample.total).unwrap_or_default();
                (
                    derived,
                    DataQuality::Quarantined,
                    Some(
                        "last_token_usage is missing; cumulative value is not a trusted event delta"
                            .to_string(),
                    ),
                )
            }
        };

        if usage.is_zero() && quality == DataQuality::Confirmed {
            return ReplayOutcome {
                disposition: if reset {
                    RecordDisposition::CounterReset
                } else {
                    RecordDisposition::CounterUnchanged
                },
                event: None,
                sample: Some(sample),
                reason: quality_reason,
            };
        }

        let disposition = if reset {
            RecordDisposition::CounterReset
        } else if quality == DataQuality::Confirmed {
            RecordDisposition::ConfirmedUsage
        } else {
            RecordDisposition::QuarantinedUsage
        };
        let event = self.make_event(
            record,
            line,
            observed_at,
            usage,
            quality,
            quality_reason.clone(),
        );
        ReplayOutcome {
            disposition,
            event: Some(event),
            sample: Some(sample),
            reason: quality_reason,
        }
    }

    fn make_event(
        &self,
        record: &Value,
        line: &JsonlLine,
        observed_at: DateTime<Utc>,
        usage: TokenUsage,
        quality: DataQuality,
        quality_reason: Option<String>,
    ) -> UsageEvent {
        let rollout_id = self
            .canonical
            .as_ref()
            .map(|canonical| canonical.rollout_id.clone())
            .unwrap_or_else(|| format!("unknown:{{{}}}", self.config.file_identity));
        let event_id = stable_event_id(
            &self.config.machine_id,
            &self.config.file_identity,
            &rollout_id,
            line.byte_offset,
        );
        UsageEvent {
            event_id,
            observed_at,
            source_timestamp: parse_source_timestamp(record),
            thread_id: self
                .canonical
                .as_ref()
                .map(|canonical| canonical.thread_id.clone()),
            parent_thread_id: self
                .canonical
                .as_ref()
                .and_then(|canonical| canonical.parent_thread_id.clone()),
            model: self.model.clone(),
            cwd: self.cwd.clone(),
            account_fingerprint: self.config.account_fingerprint.clone(),
            account_confidence: self.config.account_confidence,
            project: self.config.project.clone(),
            usage,
            quality,
            quality_reason,
            provenance: EventProvenance {
                machine_id: self.config.machine_id.clone(),
                source_id: self.config.source_id.clone(),
                rollout_id,
                file_identity: self.config.file_identity.clone(),
                byte_offset: line.byte_offset,
                line_number: line.line_number,
            },
        }
    }
}

/// Extract both cumulative context and the actual per-sampling delta.
pub fn parse_token_sample(record: &Value) -> TokenSample {
    let info = record.pointer("/payload/info");
    TokenSample {
        total: info
            .and_then(|info| info.get("total_token_usage"))
            .and_then(parse_usage),
        last: info
            .and_then(|info| info.get("last_token_usage"))
            .and_then(parse_usage),
    }
}

fn parse_usage(value: &Value) -> Option<TokenUsage> {
    let object = value.as_object()?;
    let input_tokens = object.get("input_tokens").and_then(json_u64).unwrap_or(0);
    let cached_input_tokens = object
        .get("cached_input_tokens")
        .and_then(json_u64)
        .unwrap_or(0);
    let cache_write_value = object
        .get("cache_write_input_tokens")
        .or_else(|| object.get("cache_write_tokens"))
        .or_else(|| object.get("input_cache_write_tokens"));
    let cache_write_input_tokens = cache_write_value.and_then(json_u64).unwrap_or(0);
    let cache_write_observed_input_tokens = if cache_write_value.is_some() {
        input_tokens
    } else {
        0
    };
    let output_tokens = object.get("output_tokens").and_then(json_u64).unwrap_or(0);
    let reasoning_output_tokens = object
        .get("reasoning_output_tokens")
        .and_then(json_u64)
        .unwrap_or(0);
    let total_tokens = object
        .get("total_tokens")
        .and_then(json_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    Some(TokenUsage {
        input_tokens,
        cached_input_tokens,
        cache_write_input_tokens,
        cache_write_observed_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
    })
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn parse_source_timestamp(record: &Value) -> Option<DateTime<Utc>> {
    record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn uuid_v7_millis_prefix(value: &str) -> Option<u64> {
    let prefix: String = value
        .chars()
        .filter(|character| *character != '-')
        .take(12)
        .collect();
    (prefix.len() == 12)
        .then(|| u64::from_str_radix(&prefix, 16).ok())
        .flatten()
}

fn stable_event_id(machine_id: &str, file_identity: &str, rollout_id: &str, offset: u64) -> String {
    let mut digest = Sha256::new();
    for part in [machine_id, file_identity, rollout_id] {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    digest.update(offset.to_le_bytes());
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::IncrementalJsonlTailer;
    use std::io::Cursor;

    fn fixture_lines(contents: &str) -> Vec<JsonlLine> {
        let bytes = contents.as_bytes().to_vec();
        let mut reader = Cursor::new(bytes.clone());
        IncrementalJsonlTailer::new()
            .poll_reader(&mut reader, bytes.len() as u64, "fixture")
            .unwrap()
            .lines
    }

    fn guard() -> ReplayGuard {
        ReplayGuard::new(ReplayConfig::new("machine-a", "codex-home-a", "fixture"))
    }

    fn observed_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-31T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn canonical_identity_survives_foreign_parent_replay() {
        let lines = fixture_lines(include_str!("../tests/fixtures/subagent-replay.jsonl"));
        let mut guard = guard();
        let outcomes: Vec<_> = lines
            .iter()
            .map(|line| guard.process_line(line, observed_at()))
            .collect();

        assert_eq!(
            outcomes[0].disposition,
            RecordDisposition::CanonicalSessionEstablished
        );
        assert_eq!(
            outcomes[1].disposition,
            RecordDisposition::ForeignSessionQuarantined
        );
        assert_eq!(
            outcomes[4].disposition,
            RecordDisposition::ReplayRecordQuarantined
        );
        assert_eq!(
            outcomes[4].event.as_ref().unwrap().quality,
            DataQuality::Quarantined
        );
        assert_eq!(outcomes[6].disposition, RecordDisposition::ReplayEnded);
        assert_eq!(outcomes[7].disposition, RecordDisposition::ContextUpdated);
        assert_eq!(outcomes[8].disposition, RecordDisposition::ConfirmedUsage);

        let event = outcomes[8].event.as_ref().unwrap();
        assert_eq!(
            event.thread_id.as_deref(),
            Some("01a05549-db09-7e23-a8fa-4591323280d0")
        );
        assert_eq!(
            event.parent_thread_id.as_deref(),
            Some("019f9350-7caf-7362-8d71-de313b383325")
        );
        assert_eq!(event.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(event.cwd.as_deref(), Some("/child/project"));
        assert_eq!(event.usage.total_tokens, 105);
        assert_eq!(event.quality, DataQuality::Confirmed);
        assert_eq!(
            guard.canonical().unwrap().rollout_id,
            "01a05549-db09-7e23-a8fa-4591323280d0"
        );
    }

    #[test]
    fn unchanged_counters_are_zero_and_regressions_start_an_epoch() {
        let lines = fixture_lines(include_str!("../tests/fixtures/counter-epochs.jsonl"));
        let mut guard = guard();
        let outcomes: Vec<_> = lines
            .iter()
            .map(|line| guard.process_line(line, observed_at()))
            .collect();

        assert_eq!(outcomes[2].disposition, RecordDisposition::ConfirmedUsage);
        assert_eq!(outcomes[2].event.as_ref().unwrap().usage.total_tokens, 10);
        assert_eq!(outcomes[3].disposition, RecordDisposition::CounterUnchanged);
        assert!(outcomes[3].event.is_none());
        assert_eq!(outcomes[4].disposition, RecordDisposition::ConfirmedUsage);
        assert_eq!(outcomes[4].event.as_ref().unwrap().usage.total_tokens, 8);
        assert_eq!(outcomes[5].disposition, RecordDisposition::CounterReset);
        assert_eq!(outcomes[5].event.as_ref().unwrap().usage.total_tokens, 3);
        assert_eq!(
            outcomes[5].event.as_ref().unwrap().quality,
            DataQuality::Confirmed
        );
        assert_eq!(guard.counter_epoch(), 1);
        assert_eq!(outcomes[6].disposition, RecordDisposition::CounterUnchanged);
    }

    #[test]
    fn missing_last_usage_never_becomes_confirmed_usage() {
        let lines = fixture_lines(
            "{\"type\":\"session_meta\",\"timestamp\":\"2026-08-31T00:00:00Z\",\"payload\":{\"id\":\"01a05549-db09-7e23-a8fa-4591323280d0\"}}\n\
             {\"type\":\"event_msg\",\"timestamp\":\"2026-08-31T00:00:01Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":9,\"output_tokens\":1,\"total_tokens\":10}}}}\n",
        );
        let mut guard = guard();
        guard.process_line(&lines[0], observed_at());
        let outcome = guard.process_line(&lines[1], observed_at());
        assert_eq!(outcome.disposition, RecordDisposition::QuarantinedUsage);
        assert_eq!(outcome.event.unwrap().quality, DataQuality::Quarantined);
    }

    #[test]
    fn non_conserving_last_usage_is_quarantined() {
        let lines = fixture_lines(
            "{\"type\":\"session_meta\",\"timestamp\":\"2026-08-31T00:00:00Z\",\"payload\":{\"id\":\"01a05549-db09-7e23-a8fa-4591323280d0\"}}\n\
             {\"type\":\"event_msg\",\"timestamp\":\"2026-08-31T00:00:01Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":90,\"output_tokens\":10,\"reasoning_output_tokens\":11,\"total_tokens\":101}}}}\n",
        );
        let mut guard = guard();
        guard.process_line(&lines[0], observed_at());
        let outcome = guard.process_line(&lines[1], observed_at());
        assert_eq!(outcome.disposition, RecordDisposition::QuarantinedUsage);
        let event = outcome.event.unwrap();
        assert_eq!(event.quality, DataQuality::Quarantined);
        assert!(event.quality_reason.as_deref().is_some_and(|reason| {
            reason.contains("accounting invariant") || reason.contains("total tokens")
        }));
    }

    #[test]
    fn cache_write_is_an_explicit_input_bucket_with_legacy_coverage() {
        let current = serde_json::json!({
            "payload": {"info": {"last_token_usage": {
                "input_tokens": 100,
                "cached_input_tokens": 70,
                "cache_write_input_tokens": 10,
                "output_tokens": 20,
                "total_tokens": 120
            }}}
        });
        let usage = parse_token_sample(&current).last.unwrap();
        assert_eq!(usage.cached_input_tokens, 70);
        assert_eq!(usage.cache_write_input_tokens, 10);
        assert_eq!(usage.uncached_input_tokens(), 20);
        assert_eq!(usage.cache_write_observed_input_tokens, 100);
        assert_eq!(usage.cache_write_coverage(), 1.0);

        let legacy = serde_json::json!({
            "payload": {"info": {"last_token_usage": {
                "input_tokens": 100,
                "cached_input_tokens": 70,
                "output_tokens": 20,
                "total_tokens": 120
            }}}
        });
        let legacy_usage = parse_token_sample(&legacy).last.unwrap();
        assert_eq!(legacy_usage.cache_write_input_tokens, 0);
        assert_eq!(legacy_usage.cache_write_observed_input_tokens, 0);
        assert_eq!(legacy_usage.uncached_input_tokens(), 30);
    }

    #[test]
    fn event_ids_are_stable_for_the_same_provenance() {
        let lines = fixture_lines(include_str!("../tests/fixtures/counter-epochs.jsonl"));
        let mut left = guard();
        let mut right = guard();
        let left_events: Vec<_> = lines
            .iter()
            .filter_map(|line| left.process_line(line, observed_at()).event)
            .collect();
        let right_events: Vec<_> = lines
            .iter()
            .filter_map(|line| right.process_line(line, observed_at()).event)
            .collect();
        assert_eq!(left_events, right_events);
    }

    #[test]
    fn checkpoint_restores_replay_phase_context_and_counter_state() {
        let replay_lines = fixture_lines(include_str!("../tests/fixtures/subagent-replay.jsonl"));
        let mut before_restart = guard();
        for line in &replay_lines[..5] {
            before_restart.process_line(line, observed_at());
        }
        assert_eq!(before_restart.phase(), ReplayPhase::ReplayingForeignHistory);

        let encoded = serde_json::to_string(&before_restart.checkpoint()).unwrap();
        let decoded: ReplayCheckpoint = serde_json::from_str(&encoded).unwrap();
        let mut restored = ReplayGuard::from_checkpoint(
            ReplayConfig::new("machine-a", "codex-home-a", "fixture"),
            decoded,
        )
        .unwrap();
        let outcomes: Vec<_> = replay_lines[5..]
            .iter()
            .map(|line| restored.process_line(line, observed_at()))
            .collect();
        assert_eq!(
            outcomes[0].disposition,
            RecordDisposition::ReplayRecordQuarantined
        );
        assert_eq!(outcomes[1].disposition, RecordDisposition::ReplayEnded);
        assert_eq!(outcomes[2].disposition, RecordDisposition::ContextUpdated);
        assert_eq!(outcomes[3].disposition, RecordDisposition::ConfirmedUsage);
        assert_eq!(outcomes[3].event.as_ref().unwrap().usage.total_tokens, 105);

        let counter_lines = fixture_lines(include_str!("../tests/fixtures/counter-epochs.jsonl"));
        let mut counter_guard = guard();
        for line in &counter_lines[..5] {
            counter_guard.process_line(line, observed_at());
        }
        let checkpoint = counter_guard.checkpoint();
        let mut restored = ReplayGuard::from_checkpoint(
            ReplayConfig::new("machine-a", "codex-home-a", "fixture"),
            checkpoint,
        )
        .unwrap();
        let reset = restored.process_line(&counter_lines[5], observed_at());
        assert_eq!(reset.disposition, RecordDisposition::CounterReset);
        assert_eq!(restored.counter_epoch(), 1);
        let unchanged = restored.process_line(&counter_lines[6], observed_at());
        assert_eq!(unchanged.disposition, RecordDisposition::CounterUnchanged);
    }

    #[test]
    fn checkpoint_rejects_future_schema_and_wrong_file() {
        let future = ReplayCheckpoint {
            schema_version: REPLAY_CHECKPOINT_SCHEMA_VERSION + 1,
            ..ReplayCheckpoint::default()
        };
        assert_eq!(
            ReplayGuard::from_checkpoint(
                ReplayConfig::new("machine-a", "codex-home-a", "fixture"),
                future,
            )
            .unwrap_err(),
            ReplayCheckpointError::UnsupportedSchemaVersion(REPLAY_CHECKPOINT_SCHEMA_VERSION + 1)
        );

        let wrong_file = ReplayCheckpoint {
            file_identity: Some("another-file".into()),
            ..ReplayCheckpoint::default()
        };
        assert!(matches!(
            ReplayGuard::from_checkpoint(
                ReplayConfig::new("machine-a", "codex-home-a", "fixture"),
                wrong_file,
            ),
            Err(ReplayCheckpointError::FileIdentityMismatch { .. })
        ));
    }
}
