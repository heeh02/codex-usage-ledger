//! Shared source-format boundary decisions, separate from numeric normalization.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StreamPhase {
    #[default]
    AwaitingCanonical,
    ChildPrefix,
    Live,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StreamBoundary {
    pub phase: StreamPhase,
    pub foreign_replay: bool,
    pub canonical_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryAction {
    Ignore,
    Canonical,
    Resume,
    Context,
    Baseline,
    Usage,
}

impl StreamBoundary {
    pub fn classify(
        &mut self,
        record: &Value,
        thread_id: &str,
        is_child: bool,
        last_token_at: Option<DateTime<Utc>>,
        has_counter: bool,
        valid_total: bool,
    ) -> BoundaryAction {
        let kind = record.get("type").and_then(Value::as_str);
        let payload_kind = record.pointer("/payload/type").and_then(Value::as_str);
        if kind == Some("session_meta") {
            let id = record.pointer("/payload/id").and_then(Value::as_str);
            if self.phase == StreamPhase::AwaitingCanonical {
                if id != Some(thread_id) {
                    return BoundaryAction::Ignore;
                }
                self.canonical_at = record_timestamp(record);
                self.phase = if is_child {
                    StreamPhase::ChildPrefix
                } else {
                    StreamPhase::Live
                };
                return BoundaryAction::Canonical;
            }
            if id.is_some_and(|id| id != thread_id) {
                self.foreign_replay = true;
            }
            // Seeing the child's metadata again is not proof that replay ended.
            return BoundaryAction::Ignore;
        }
        if kind == Some("event_msg")
            && payload_kind == Some("task_started")
            && (self.foreign_replay || self.phase == StreamPhase::ChildPrefix)
            && crate::replay::task_belongs_to_canonical_stream(record, thread_id, self.canonical_at)
        {
            self.foreign_replay = false;
            self.phase = StreamPhase::Live;
            return BoundaryAction::Resume;
        }
        if self.foreign_replay {
            return if kind == Some("event_msg")
                && payload_kind == Some("token_count")
                && valid_total
            {
                BoundaryAction::Baseline
            } else {
                BoundaryAction::Ignore
            };
        }
        if kind == Some("turn_context") && self.phase == StreamPhase::Live {
            return BoundaryAction::Context;
        }
        if kind != Some("event_msg")
            || payload_kind != Some("token_count")
            || self.phase == StreamPhase::AwaitingCanonical
        {
            return BoundaryAction::Ignore;
        }
        let Some(at) = record_timestamp(record) else {
            return BoundaryAction::Ignore;
        };
        if self.phase == StreamPhase::ChildPrefix {
            if !valid_total {
                return BoundaryAction::Ignore;
            }
            let near = |earlier: DateTime<Utc>| {
                at.signed_duration_since(earlier).num_milliseconds().max(0) <= 2_000
            };
            let inherited = last_token_at
                .filter(|_| has_counter)
                .map(near)
                .unwrap_or_else(|| self.canonical_at.is_some_and(near));
            if inherited {
                return BoundaryAction::Baseline;
            }
            self.phase = StreamPhase::Live;
        }
        BoundaryAction::Usage
    }
}

pub(crate) fn record_timestamp(record: &Value) -> Option<DateTime<Utc>> {
    record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}
