use super::*;

struct Point {
    at: String,
    thread: Option<String>,
    model: Option<String>,
    usage: TokenUsage,
}

fn usage_at(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<TokenUsage> {
    Ok(TokenUsage {
        input_tokens: u64_from_sql(row.get(offset)?, offset)?,
        cached_input_tokens: u64_from_sql(row.get(offset + 1)?, offset + 1)?,
        cache_write_input_tokens: u64_from_sql(row.get(offset + 2)?, offset + 2)?,
        cache_write_observed_input_tokens: u64_from_sql(row.get(offset + 3)?, offset + 3)?,
        output_tokens: u64_from_sql(row.get(offset + 4)?, offset + 4)?,
        reasoning_output_tokens: u64_from_sql(row.get(offset + 5)?, offset + 5)?,
        total_tokens: u64_from_sql(row.get(offset + 6)?, offset + 6)?,
    })
}

impl LedgerStore {
    pub(super) fn candidate_comparison(
        &self,
        event_id: &str,
    ) -> StoreResult<Option<CandidateComparison>> {
        let evidence = self.connection.query_row(
            "SELECT kept.effective_at,kept.thread_id,kept.model,kept.quality='confirmed',
                kept.input_tokens,kept.cached_input_tokens,kept.cache_write_input_tokens,
                kept.cache_write_observed_input_tokens,kept.output_tokens,kept.reasoning_output_tokens,kept.total_tokens,
                link.reconstruction_event_id,rebuilt.event_id,COALESCE(rebuilt.source_timestamp,rebuilt.observed_at),
                rebuilt.thread_id,rebuilt.model,
                rebuilt.input_tokens,rebuilt.cached_input_tokens,rebuilt.cache_write_input_tokens,
                rebuilt.cache_write_observed_input_tokens,rebuilt.output_tokens,rebuilt.reasoning_output_tokens,rebuilt.total_tokens,
                (SELECT COUNT(*) FROM sampling_candidate_links other WHERE other.reconstruction_event_id=link.reconstruction_event_id)
             FROM retained_request_evidence kept
             LEFT JOIN sampling_candidate_links link ON link.event_id=kept.event_id
             LEFT JOIN reconstruction_usage_events rebuilt ON rebuilt.event_id=link.reconstruction_event_id
             WHERE kept.event_id=?1",
            params![event_id], |row| {
                let source = Point { at: row.get(0)?, thread: row.get(1)?, model: row.get(2)?, usage: usage_at(row, 4)? };
                let confirmed: bool = row.get(3)?;
                let candidate_id: Option<String> = row.get(11)?;
                let target = if row.get::<_, Option<String>>(12)?.is_some() {
                    Some(Point { at: row.get(13)?, thread: row.get(14)?, model: row.get(15)?, usage: usage_at(row, 16)? })
                } else { None };
                Ok((source, confirmed, candidate_id, target, u64_from_sql(row.get(23)?, 23)?))
            },
        ).optional()?;
        let Some((source, confirmed, candidate_id, candidate, linked_records)) = evidence else {
            return Ok(None);
        };
        Ok(Some(compare_points(
            source,
            confirmed,
            candidate_id,
            candidate,
            linked_records,
        )))
    }
}

fn compare_points(
    source: Point,
    confirmed: bool,
    candidate_id: Option<String>,
    candidate: Option<Point>,
    linked_records: u64,
) -> CandidateComparison {
    let mut comparison = CandidateComparison {
        status: CandidateOverlapStatus::NotLinked,
        candidate_id,
        candidate_at: candidate.as_ref().map(|point| point.at.clone()),
        candidate_thread_id: candidate.as_ref().and_then(|point| point.thread.clone()),
        candidate_model: candidate.as_ref().and_then(|point| point.model.clone()),
        candidate_usage: candidate.as_ref().map(|point| point.usage),
        candidate_usage_valid: candidate
            .as_ref()
            .map(|point| point.usage.validate().is_ok()),
        linked_records,
        candidate_minus_source_nanoseconds: None,
        mismatches: Vec::new(),
    };
    if let Some(candidate) = candidate.as_ref() {
        if !confirmed {
            comparison
                .mismatches
                .push(CandidateMismatch::SourceUnconfirmed);
        }
        if source.thread != candidate.thread {
            comparison.mismatches.push(CandidateMismatch::Thread);
        }
        if source.model != candidate.model {
            comparison.mismatches.push(CandidateMismatch::Model);
        }
        if confirmed {
            for (field, left, right) in [
                (
                    CandidateMismatch::Input,
                    source.usage.input_tokens,
                    candidate.usage.input_tokens,
                ),
                (
                    CandidateMismatch::CacheRead,
                    source.usage.cached_input_tokens,
                    candidate.usage.cached_input_tokens,
                ),
                (
                    CandidateMismatch::CacheWrite,
                    source.usage.cache_write_input_tokens,
                    candidate.usage.cache_write_input_tokens,
                ),
                (
                    CandidateMismatch::CacheWriteCoverage,
                    source.usage.cache_write_observed_input_tokens,
                    candidate.usage.cache_write_observed_input_tokens,
                ),
                (
                    CandidateMismatch::Output,
                    source.usage.output_tokens,
                    candidate.usage.output_tokens,
                ),
                (
                    CandidateMismatch::Reasoning,
                    source.usage.reasoning_output_tokens,
                    candidate.usage.reasoning_output_tokens,
                ),
                (
                    CandidateMismatch::Total,
                    source.usage.total_tokens,
                    candidate.usage.total_tokens,
                ),
            ] {
                if left != right {
                    comparison.mismatches.push(field);
                }
            }
        }
        if comparison.candidate_usage_valid == Some(false) {
            comparison
                .mismatches
                .push(CandidateMismatch::InvalidCandidateUsage);
        }
        comparison.candidate_minus_source_nanoseconds = DateTime::parse_from_rfc3339(&source.at)
            .ok()
            .zip(DateTime::parse_from_rfc3339(&candidate.at).ok())
            .and_then(|(source, candidate)| (candidate - source).num_nanoseconds());
        match comparison.candidate_minus_source_nanoseconds {
            Some(delta) if delta.unsigned_abs() > 250_000_000 => comparison
                .mismatches
                .push(CandidateMismatch::TimeOutsideTolerance),
            None => comparison
                .mismatches
                .push(CandidateMismatch::UnverifiableTime),
            _ => {}
        }
    }
    comparison.status = if comparison.candidate_id.is_none() {
        CandidateOverlapStatus::NotLinked
    } else if linked_records > 1 {
        CandidateOverlapStatus::SharedCandidate
    } else if candidate.is_none() {
        CandidateOverlapStatus::TargetUnavailable
    } else if comparison.candidate_minus_source_nanoseconds.is_none() {
        CandidateOverlapStatus::UnverifiableTime
    } else if comparison.mismatches.is_empty() {
        CandidateOverlapStatus::ConsistentCandidate
    } else {
        CandidateOverlapStatus::DifferentEvidence
    };
    comparison
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_counterpart_is_diagnostic_without_weakening_storage_guards() {
        let point = |usage| Point {
            at: "2026-01-01T00:00:00Z".into(),
            thread: Some("synthetic".into()),
            model: None,
            usage,
        };
        let invalid = TokenUsage {
            total_tokens: 999,
            ..TokenUsage::default()
        };
        let comparison = compare_points(
            point(TokenUsage::default()),
            true,
            Some("candidate".into()),
            Some(point(invalid)),
            1,
        );
        assert_eq!(comparison.candidate_usage_valid, Some(false));
        assert_eq!(comparison.status, CandidateOverlapStatus::DifferentEvidence);
        assert!(comparison.mismatches.contains(&CandidateMismatch::Total));
        assert!(
            comparison
                .mismatches
                .contains(&CandidateMismatch::InvalidCandidateUsage)
        );
    }
}
