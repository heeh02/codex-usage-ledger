//! Shadow selection of local measurements, not inference/billing identity.
use crate::types::TokenUsage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSide {
    Sampling,
    Reconstruction,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub id: String,
    pub side: EvidenceSide,
    pub record_key: Option<String>,
    pub at: DateTime<Utc>,
    pub thread: String,
    pub model: Option<String>,
    pub account: Option<String>,
    pub project: Option<String>,
    pub assignment_available: bool,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    MissingRecordKey,
    UnconfirmedUsage,
    InvalidUsage,
    MissingAssignment,
    MultipleRecordsOnOneSide,
    ConflictingDimensions,
    ConflictingUsage,
    TimeMismatch,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedGroup {
    pub record_key: Option<String>,
    pub records: Vec<Measurement>,
    pub reason: UnresolvedReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnionShadow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub scope: &'static str,
    pub production_policy_changed: bool,
    pub version: u32,
    pub input_records: usize,
    pub selected: Vec<Measurement>,
    pub unresolved: Vec<UnresolvedGroup>,
    pub shared_records_collapsed: usize,
    pub canonical_records_outside_window: usize,
    pub complete_for_supplied_records: bool,
    pub history_complete: bool,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, thiserror::Error)]
pub enum UnionError {
    #[error("source union contains duplicate event identities")]
    DuplicateIdentity,
    #[error("source union total overflow")]
    Overflow,
    #[error("source union needs start before end")]
    InvalidWindow,
}

pub fn plan(
    records: Vec<Measurement>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<UnionShadow, UnionError> {
    if start >= end {
        return Err(UnionError::InvalidWindow);
    }
    let mut identities = BTreeSet::new();
    let mut keyed = BTreeMap::<String, Vec<Measurement>>::new();
    let mut report = UnionShadow {
        start,
        end,
        scope: "supplied_local_measurements_not_inference_usage",
        production_policy_changed: false,
        version: 1,
        input_records: records.len(),
        selected: Vec::new(),
        unresolved: Vec::new(),
        shared_records_collapsed: 0,
        canonical_records_outside_window: 0,
        complete_for_supplied_records: false,
        history_complete: false,
        usage: None,
    };
    for record in records {
        if !identities.insert((record.side, record.id.clone())) {
            return Err(UnionError::DuplicateIdentity);
        }
        if let Some(key) = record.record_key.as_ref().filter(|key| !key.is_empty()) {
            keyed.entry(key.clone()).or_default().push(record);
        } else {
            report.unresolved.push(UnresolvedGroup {
                record_key: None,
                records: vec![record],
                reason: UnresolvedReason::MissingRecordKey,
            });
        }
    }
    for (key, mut group) in keyed {
        group.sort_by_key(|record| (record.side, record.id.clone()));
        let first = &group[0];
        let reason = if group.iter().any(|record| record.usage.is_none()) {
            Some(UnresolvedReason::UnconfirmedUsage)
        } else if group
            .iter()
            .any(|record| record.usage.is_some_and(|usage| usage.validate().is_err()))
        {
            Some(UnresolvedReason::InvalidUsage)
        } else if group.iter().any(|record| !record.assignment_available) {
            Some(UnresolvedReason::MissingAssignment)
        } else if group.len() > 2 || group.windows(2).any(|pair| pair[0].side == pair[1].side) {
            Some(UnresolvedReason::MultipleRecordsOnOneSide)
        } else if group.iter().any(|record| {
            record.thread != first.thread
                || record.model != first.model
                || record.account != first.account
                || record.project != first.project
        }) {
            Some(UnresolvedReason::ConflictingDimensions)
        } else if group.iter().any(|record| record.usage != first.usage) {
            Some(UnresolvedReason::ConflictingUsage)
        } else if group.iter().any(|record| {
            (record.at - first.at)
                .num_nanoseconds()
                .is_none_or(|delta| delta.unsigned_abs() > 250_000_000)
        }) {
            Some(UnresolvedReason::TimeMismatch)
        } else {
            None
        };
        if let Some(reason) = reason {
            report.unresolved.push(UnresolvedGroup {
                record_key: Some(key),
                records: group,
                reason,
            });
            continue;
        }
        // Preserve the sampling observation's current attribution/time when the
        // same source measurement exists on both sides. Never filter before this.
        report.shared_records_collapsed += group.len() - 1;
        let selected = group.remove(0);
        if selected.at >= start && selected.at < end {
            report.selected.push(selected);
        } else {
            report.canonical_records_outside_window += 1;
        }
    }
    report
        .selected
        .sort_by_key(|record| (record.at, record.side, record.id.clone()));
    report.complete_for_supplied_records = report.unresolved.is_empty();
    if report.complete_for_supplied_records && !report.selected.is_empty() {
        let mut total = TokenUsage::default();
        for record in &report.selected {
            add(&mut total, record.usage.expect("validated measurement"))?;
        }
        report.usage = Some(total);
    }
    Ok(report)
}

fn add(total: &mut TokenUsage, next: TokenUsage) -> Result<(), UnionError> {
    for (target, value) in [
        (&mut total.input_tokens, next.input_tokens),
        (&mut total.cached_input_tokens, next.cached_input_tokens),
        (
            &mut total.cache_write_input_tokens,
            next.cache_write_input_tokens,
        ),
        (
            &mut total.cache_write_observed_input_tokens,
            next.cache_write_observed_input_tokens,
        ),
        (&mut total.output_tokens, next.output_tokens),
        (
            &mut total.reasoning_output_tokens,
            next.reasoning_output_tokens,
        ),
        (&mut total.total_tokens, next.total_tokens),
    ] {
        *target = target.checked_add(value).ok_or(UnionError::Overflow)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, side: EvidenceSide, key: &str) -> Measurement {
        Measurement {
            id: id.into(),
            side,
            record_key: Some(key.into()),
            at: "2026-01-01T00:00:00Z".parse().unwrap(),
            thread: "thread".into(),
            model: Some("model".into()),
            account: Some("account".into()),
            project: Some("project".into()),
            assignment_available: true,
            usage: Some(TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 40,
                cache_write_input_tokens: 10,
                cache_write_observed_input_tokens: 100,
                output_tokens: 20,
                reasoning_output_tokens: 5,
                total_tokens: 120,
            }),
        }
    }
    fn run(records: Vec<Measurement>) -> UnionShadow {
        plan(
            records,
            "2025-12-31T00:00:00Z".parse().unwrap(),
            "2026-01-02T00:00:00Z".parse().unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn same_record_collapses_once_and_all_components_conserve() {
        let a = record("a", EvidenceSide::Sampling, "shared");
        let b = record("b", EvidenceSide::Reconstruction, "shared");
        let c = record("c", EvidenceSide::Reconstruction, "independent");
        let report = run(vec![c, b, a]);
        assert_eq!(report.shared_records_collapsed, 1);
        assert_eq!(report.selected.len(), 2);
        let usage = report.usage.unwrap();
        assert_eq!(
            (
                usage.input_tokens,
                usage.cached_input_tokens,
                usage.cache_write_input_tokens,
                usage.output_tokens,
                usage.reasoning_output_tokens,
                usage.total_tokens
            ),
            (200, 80, 20, 40, 10, 240)
        );
        assert!(usage.validate().is_ok());
        assert_eq!(report.selected[0].id, "a");
    }
    #[test]
    fn ambiguities_never_become_a_partial_total_disguised_as_complete() {
        type Mutate = fn(&mut Measurement);
        let cases: &[(UnresolvedReason, Mutate)] = &[
            (UnresolvedReason::MissingRecordKey, |r| r.record_key = None),
            (UnresolvedReason::UnconfirmedUsage, |r| r.usage = None),
            (UnresolvedReason::MissingAssignment, |r| {
                r.assignment_available = false
            }),
            (UnresolvedReason::ConflictingDimensions, |r| {
                r.account = None
            }),
            (UnresolvedReason::ConflictingUsage, |r| {
                r.usage.as_mut().unwrap().cached_input_tokens += 1
            }),
            (UnresolvedReason::TimeMismatch, |r| {
                r.at += chrono::Duration::seconds(1)
            }),
            (UnresolvedReason::InvalidUsage, |r| {
                r.usage.as_mut().unwrap().total_tokens = 999
            }),
            (UnresolvedReason::MultipleRecordsOnOneSide, |r| {
                r.side = EvidenceSide::Sampling
            }),
        ];
        for (reason, mutate) in cases {
            let a = record("a", EvidenceSide::Sampling, "shared");
            let mut b = record("b", EvidenceSide::Reconstruction, "shared");
            mutate(&mut b);
            let report = run(vec![a, b]);
            assert!(!report.complete_for_supplied_records);
            assert!(report.usage.is_none());
            assert!(
                report
                    .unresolved
                    .iter()
                    .any(|group| group.reason == *reason)
            );
        }
    }
    #[test]
    fn empty_zero_and_overflow_are_not_conflated() {
        assert!(run(Vec::new()).usage.is_none());
        let mut zero = record("zero", EvidenceSide::Sampling, "zero");
        zero.usage = Some(TokenUsage::default());
        assert_eq!(run(vec![zero]).usage, Some(TokenUsage::default()));
        let mut a = record("a", EvidenceSide::Sampling, "a");
        a.usage = Some(TokenUsage {
            input_tokens: u64::MAX,
            total_tokens: u64::MAX,
            ..TokenUsage::default()
        });
        let b = record("b", EvidenceSide::Sampling, "b");
        assert!(matches!(
            plan(
                vec![a.clone(), b],
                a.at,
                a.at + chrono::Duration::seconds(1)
            ),
            Err(UnionError::Overflow)
        ));
    }

    #[test]
    fn canonical_time_is_selected_before_boundary_filtering() {
        let a = record("a", EvidenceSide::Sampling, "shared");
        let mut b = record("b", EvidenceSide::Reconstruction, "shared");
        b.at += chrono::Duration::milliseconds(100);
        let report = plan(
            vec![a.clone(), b],
            a.at + chrono::Duration::milliseconds(50),
            a.at + chrono::Duration::seconds(1),
        )
        .unwrap();
        assert!(report.selected.is_empty());
        assert_eq!(report.canonical_records_outside_window, 1);
        assert!(report.usage.is_none());
        assert!(matches!(
            plan(
                vec![a.clone(), a.clone()],
                a.at,
                a.at + chrono::Duration::seconds(1)
            ),
            Err(UnionError::DuplicateIdentity)
        ));
    }
}
