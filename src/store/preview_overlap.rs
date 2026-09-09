//! Bounded consistency comparison; proximity never creates record identity.
use super::*;
use crate::source_union::{EvidenceSide, Measurement, same_token_amounts};
use anyhow::{Result, anyhow};

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlapGroup {
    records: u64,
    sampling_usage: Option<TokenUsage>,
    write_coverage_differences: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlapRow {
    sampling_event: String,
    candidate_event: Option<String>,
    sampling_at: DateTime<Utc>,
    candidate_at: Option<DateTime<Utc>>,
    status: &'static str,
    nearby_candidates: usize,
    reverse_sampling_neighbors: usize,
    key_agreement: Option<bool>,
    write_coverage_differs: bool,
    mismatches: Vec<&'static str>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOverlap {
    scope: &'static str,
    thread: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    manifest_body_sha256: String,
    loaded_sampling: usize,
    loaded_candidates: usize,
    sampling_in_scope: usize,
    candidates_in_scope: usize,
    candidates_without_sampling_neighbors: usize,
    groups: BTreeMap<&'static str, OverlapGroup>,
    rows: Option<Vec<OverlapRow>>,
    identity_proven: bool,
    production_policy_changed: bool,
    preview_revalidated: bool,
}

pub fn compare_preview_sampling(
    preview: &Path,
    db: &Path,
    thread: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: usize,
    include_rows: bool,
) -> Result<PreviewOverlap> {
    if thread.is_empty() || start >= end || !(1..=10000).contains(&limit) {
        return Err(anyhow!(
            "require thread, ordered dates and combined limit 1..10000"
        ));
    }
    let before = start
        .checked_sub_signed(ChronoDuration::milliseconds(500))
        .ok_or_else(|| anyhow!("date outside supported range"))?;
    let after = end
        .checked_add_signed(ChronoDuration::milliseconds(500))
        .ok_or_else(|| anyhow!("date outside supported range"))?;
    let candidate_start = start
        .checked_sub_signed(ChronoDuration::milliseconds(250))
        .ok_or_else(|| anyhow!("date outside supported range"))?;
    let candidate_end = end
        .checked_add_signed(ChronoDuration::milliseconds(250))
        .ok_or_else(|| anyhow!("date outside supported range"))?;
    let store = LedgerStore::open_reconstruction_audit(db)?;
    let input = store.connection.unchecked_transaction()?;
    let preview = correction_preview::open_preview_read_only(preview)?;
    let candidate_snapshot = preview.unchecked_transaction()?;
    let hash = correction_preview::preview_hash(&candidate_snapshot)?;
    let ids=input.prepare_cached("SELECT event_id FROM retained_request_evidence WHERE thread_id=?1 AND effective_at>=?2 AND effective_at<?3 ORDER BY effective_at,event_id LIMIT ?4")?
        .query_map(params![thread,timestamp(before),timestamp(after),(limit+1) as i64],|row|row.get::<_,String>(0))?
        .collect::<Result<Vec<_>,_>>()?;
    if ids.len() > limit {
        return Err(anyhow!(
            "overlap context exceeds budget; narrow the interval"
        ));
    }
    let samples = ids
        .iter()
        .map(|id| union_repository::load_measurement(&input, EvidenceSide::Sampling, id))
        .collect::<StoreResult<Vec<_>>>()?;
    let candidates=candidate_snapshot.prepare_cached("SELECT event_id,at,thread_id,model,account_id,project_id,record_key,
        input_tokens,cached_input_tokens,cache_write_input_tokens,cache_write_observed_input_tokens,output_tokens,reasoning_output_tokens,total_tokens
        FROM preview_facts WHERE side='candidate' AND thread_id=?1 AND at>=?2 AND at<?3 ORDER BY at,event_id LIMIT ?4")?
        .query_map(params![thread,timestamp(candidate_start),timestamp(candidate_end),(limit-samples.len()+1) as i64],|row|Ok(Measurement{
            id:row.get(0)?,at:parse_timestamp_column(row.get(1)?,1)?,thread:row.get(2)?,model:row.get(3)?,account:row.get(4)?,project:row.get(5)?,
            record_key:row.get(6)?,side:EvidenceSide::Reconstruction,assignment_available:true,usage:Some(TokenUsage{
                input_tokens:u64_from_sql(row.get(7)?,7)?,cached_input_tokens:u64_from_sql(row.get(8)?,8)?,
                cache_write_input_tokens:u64_from_sql(row.get(9)?,9)?,cache_write_observed_input_tokens:u64_from_sql(row.get(10)?,10)?,
                output_tokens:u64_from_sql(row.get(11)?,11)?,reasoning_output_tokens:u64_from_sql(row.get(12)?,12)?,total_tokens:u64_from_sql(row.get(13)?,13)?,
            })
        }))?.collect::<Result<Vec<_>,_>>()?;
    if samples.len() + candidates.len() > limit {
        return Err(anyhow!(
            "overlap context exceeds budget; narrow the interval"
        ));
    }
    let report = compare(samples, candidates, thread, start, end, hash, include_rows)?;
    candidate_snapshot.commit()?;
    input.commit()?;
    Ok(report)
}

fn neighbors(points: &[Measurement], at: DateTime<Utc>) -> Result<std::ops::Range<usize>> {
    let low = at
        .checked_sub_signed(ChronoDuration::milliseconds(250))
        .ok_or_else(|| anyhow!("unsupported time"))?;
    let high = at
        .checked_add_signed(ChronoDuration::milliseconds(250))
        .ok_or_else(|| anyhow!("unsupported time"))?;
    Ok(points.partition_point(|p| p.at < low)..points.partition_point(|p| p.at <= high))
}

fn compare(
    mut samples: Vec<Measurement>,
    mut candidates: Vec<Measurement>,
    thread: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    hash: String,
    include_rows: bool,
) -> Result<PreviewOverlap> {
    samples.sort_by_key(|p| p.at);
    candidates.sort_by_key(|p| p.at);
    let reverse = candidates
        .iter()
        .map(|p| neighbors(&samples, p.at).map(|r| r.len()))
        .collect::<Result<Vec<_>>>()?;
    let mut result = PreviewOverlap {
        scope: "sampling_vs_candidate_consistency_not_a_union",
        thread: thread.into(),
        start,
        end,
        manifest_body_sha256: hash,
        loaded_sampling: samples.len(),
        loaded_candidates: candidates.len(),
        sampling_in_scope: 0,
        candidates_in_scope: 0,
        candidates_without_sampling_neighbors: 0,
        groups: BTreeMap::new(),
        rows: include_rows.then(Vec::new),
        identity_proven: false,
        production_policy_changed: false,
        preview_revalidated: false,
    };
    for (candidate, count) in candidates.iter().zip(&reverse) {
        if candidate.at >= start && candidate.at < end {
            result.candidates_in_scope += 1;
            result.candidates_without_sampling_neighbors += usize::from(*count == 0);
        }
    }
    for sample in samples.iter().filter(|p| p.at >= start && p.at < end) {
        result.sampling_in_scope += 1;
        let range = neighbors(&candidates, sample.at)?;
        let candidate = (range.len() == 1).then(|| &candidates[range.start]);
        let reverse_count = if candidate.is_some() {
            reverse[range.start]
        } else {
            0
        };
        let key_agreement = sample
            .record_key
            .as_ref()
            .filter(|key| !key.is_empty())
            .zip(
                candidate
                    .and_then(|p| p.record_key.as_ref())
                    .filter(|key| !key.is_empty()),
            )
            .map(|(a, b)| a == b);
        let mut mismatches = Vec::new();
        let mut coverage_diff = false;
        if let Some(candidate) = candidate {
            for (field, different) in [
                ("model", sample.model != candidate.model),
                ("account", sample.account != candidate.account),
                ("project", sample.project != candidate.project),
            ] {
                if different {
                    mismatches.push(field);
                }
            }
            if let (Some(a), Some(b)) = (sample.usage, candidate.usage) {
                coverage_diff =
                    a.cache_write_observed_input_tokens != b.cache_write_observed_input_tokens;
                if !same_token_amounts(a, b) {
                    mismatches.push("token_amounts");
                }
            }
        }
        let status = if sample.usage.is_none() {
            "unconfirmed_sampling"
        } else if sample.usage.is_some_and(|u| u.validate().is_err())
            || candidate.is_some_and(|p| p.usage.is_some_and(|u| u.validate().is_err()))
        {
            "invalid_usage"
        } else if !sample.assignment_available {
            "missing_assignment"
        } else if range.is_empty() {
            "no_nearby_candidate"
        } else if range.len() != 1 || reverse_count != 1 {
            "ambiguous_neighbors"
        } else if !mismatches.is_empty() {
            "conflicting_values"
        } else if key_agreement == Some(false) {
            "different_record_keys"
        } else if key_agreement == Some(true) {
            "compatible_shared_key"
        } else {
            "compatible_without_identity"
        };
        let group = result.groups.entry(status).or_default();
        group.records += 1;
        group.write_coverage_differences += u64::from(coverage_diff);
        if let Some(usage) = sample.usage.filter(|u| u.validate().is_ok()) {
            crate::source_union::add(group.sampling_usage.get_or_insert_default(), usage)?;
        }
        if let Some(rows) = &mut result.rows {
            rows.push(OverlapRow {
                sampling_event: sample.id.clone(),
                candidate_event: candidate.map(|p| p.id.clone()),
                sampling_at: sample.at,
                candidate_at: candidate.map(|p| p.at),
                status,
                nearby_candidates: range.len(),
                reverse_sampling_neighbors: reverse_count,
                key_agreement,
                write_coverage_differs: coverage_diff,
                mismatches,
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(id: &str, nanos: i64, coverage: u64) -> Measurement {
        Measurement {
            id: id.into(),
            side: EvidenceSide::Sampling,
            record_key: None,
            at: "2026-01-01T00:00:01Z".parse::<DateTime<Utc>>().unwrap()
                + ChronoDuration::nanoseconds(nanos),
            thread: "thread".into(),
            model: Some("model".into()),
            account: None,
            project: None,
            assignment_available: true,
            usage: Some(TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 40,
                cache_write_observed_input_tokens: coverage,
                output_tokens: 20,
                reasoning_output_tokens: 5,
                total_tokens: 120,
                ..Default::default()
            }),
        }
    }
    fn run(samples: Vec<Measurement>, candidates: Vec<Measurement>) -> PreviewOverlap {
        compare(
            samples,
            candidates,
            "thread",
            "2026-01-01T00:00:01Z".parse().unwrap(),
            "2026-01-01T00:00:02Z".parse().unwrap(),
            "digest".into(),
            true,
        )
        .unwrap()
    }
    #[test]
    fn matching_amounts_with_different_coverage_do_not_create_identity() {
        let report = run(
            vec![point("sample", 0, 0)],
            vec![point("candidate", 100_000_000, 100)],
        );
        assert_eq!(report.groups["compatible_without_identity"].records, 1);
        assert_eq!(
            report.groups["compatible_without_identity"].write_coverage_differences,
            1
        );
        assert_eq!(
            report.groups["compatible_without_identity"]
                .sampling_usage
                .unwrap()
                .total_tokens,
            120
        );
        assert!(report.rows.unwrap()[0].mismatches.is_empty());
        assert!(!report.identity_proven && !report.production_policy_changed);
    }
    #[test]
    fn exact_tolerance_and_reverse_neighbors_outside_scope_prevent_false_uniqueness() {
        let report = run(
            vec![point("outside", -450_000_000, 0), point("seed", 0, 0)],
            vec![point("candidate", -200_000_000, 0)],
        );
        assert_eq!(report.sampling_in_scope, 1);
        assert_eq!(report.groups["ambiguous_neighbors"].records, 1);
        assert_eq!(report.rows.unwrap()[0].reverse_sampling_neighbors, 2);
        assert!(
            run(
                vec![point("seed", 0, 0)],
                vec![point("edge", 250_000_000, 0)]
            )
            .groups
            .contains_key("compatible_without_identity")
        );
        assert!(
            run(
                vec![point("seed", 0, 0)],
                vec![point("outside", 250_000_001, 0)]
            )
            .groups
            .contains_key("no_nearby_candidate")
        );
    }
    #[test]
    fn unknown_amounts_real_value_conflicts_and_key_conflicts_stay_visible() {
        let mut sample = point("sample", 0, 0);
        sample.usage = None;
        let report = run(vec![sample], vec![point("candidate", 0, 100)]);
        assert!(
            report.groups["unconfirmed_sampling"]
                .sampling_usage
                .is_none()
        );
        let mut candidate = point("candidate", 0, 0);
        candidate.usage.as_mut().unwrap().input_tokens += 1;
        candidate.usage.as_mut().unwrap().total_tokens += 1;
        assert!(
            run(vec![point("sample", 0, 0)], vec![candidate])
                .groups
                .contains_key("conflicting_values")
        );
        let mut sample = point("sample", 0, 0);
        sample.record_key = Some("key-a".into());
        let mut candidate = point("candidate", 0, 0);
        candidate.record_key = Some("key-b".into());
        assert!(
            run(vec![sample.clone()], vec![candidate.clone()])
                .groups
                .contains_key("different_record_keys")
        );
        candidate.record_key = sample.record_key.clone();
        let report = run(vec![sample], vec![candidate]);
        assert!(report.groups.contains_key("compatible_shared_key"));
        assert!(!report.identity_proven);
    }
}
