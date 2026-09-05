use super::*;

pub(super) fn request_page_sql(has_cursor: bool) -> String {
    let lower_bound = if has_cursor {
        "(effective_at, event_id) > (?4, ?5)"
    } else {
        "effective_at >= ?2"
    };
    format!(
        "SELECT effective_at, event_id, turn_id, model, account_fingerprint,
        project_id, quality, input_tokens, cached_input_tokens,
        cache_write_input_tokens, cache_write_observed_input_tokens,
        output_tokens, reasoning_output_tokens, total_tokens,
        account_confidence, project_confidence
        FROM retained_request_evidence
        WHERE thread_id = ?1 AND effective_at < ?3 AND {lower_bound}
        ORDER BY effective_at, event_id LIMIT ?6"
    )
}

impl LedgerStore {
    /// Checks one source candidate without rewriting source choices or totals.
    pub fn request_candidate_overlap(&self, event_id: &str) -> StoreResult<CandidateOverlapStatus> {
        let evidence = self.connection.query_row(
            "SELECT kept.effective_at, link.reconstruction_event_id,
                COALESCE(rebuilt.source_timestamp, rebuilt.observed_at),
                kept.quality='confirmed' AND kept.thread_id IS rebuilt.thread_id
                AND kept.model IS rebuilt.model
                AND kept.input_tokens=rebuilt.input_tokens
                AND kept.cached_input_tokens=rebuilt.cached_input_tokens
                AND kept.cache_write_input_tokens=rebuilt.cache_write_input_tokens
                AND kept.cache_write_observed_input_tokens=rebuilt.cache_write_observed_input_tokens
                AND kept.output_tokens=rebuilt.output_tokens
                AND kept.reasoning_output_tokens=rebuilt.reasoning_output_tokens
                AND kept.total_tokens=rebuilt.total_tokens,
                EXISTS(SELECT 1 FROM sampling_candidate_links other
                    WHERE other.reconstruction_event_id=link.reconstruction_event_id
                    AND other.event_id != link.event_id)
             FROM retained_request_evidence kept
             LEFT JOIN sampling_candidate_links link ON link.event_id=kept.event_id
             LEFT JOIN reconstruction_usage_events rebuilt ON rebuilt.event_id=link.reconstruction_event_id
             WHERE kept.event_id=?1",
            params![event_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?, row.get::<_, Option<bool>>(3)?, row.get::<_, bool>(4)?)),
        ).optional()?;
        let Some((sample_time, link, target_time, components_equal, shared)) = evidence else {
            return Ok(CandidateOverlapStatus::RequestUnavailable);
        };
        if link.is_none() {
            return Ok(CandidateOverlapStatus::NotLinked);
        }
        if shared {
            return Ok(CandidateOverlapStatus::SharedCandidate);
        }
        let Some(target_time) = target_time else {
            return Ok(CandidateOverlapStatus::TargetUnavailable);
        };
        let (Ok(sample), Ok(target)) = (
            DateTime::parse_from_rfc3339(&sample_time),
            DateTime::parse_from_rfc3339(&target_time),
        ) else {
            return Ok(CandidateOverlapStatus::UnverifiableTime);
        };
        let near = (sample - target)
            .num_nanoseconds()
            .is_some_and(|delta| delta.unsigned_abs() <= 250_000_000);
        Ok(if near && components_equal == Some(true) {
            CandidateOverlapStatus::ConsistentCandidate
        } else {
            CandidateOverlapStatus::DifferentEvidence
        })
    }

    /// Retained local observations only, not a completeness claim or an
    /// effective-accounting source. Attribution is ingest-observed, not current.
    /// The half-open window and compound cursor prevent equal-time rows from
    /// being skipped. Late arrivals before the cursor require a fresh traversal.
    pub fn retained_request_page(
        &self,
        thread_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        after: Option<&RetainedRequestCursor>,
        limit: usize,
    ) -> StoreResult<RetainedRequestPage> {
        if start >= end || thread_id.is_empty() || !(1..=500).contains(&limit) {
            return Err(StoreError::InvalidRequestQuery(
                "require thread, start < end and limit 1..500",
            ));
        }
        if let Some(cursor) = after {
            let at = DateTime::parse_from_rfc3339(&cursor.effective_at)
                .map_err(|_| StoreError::InvalidRequestQuery("invalid cursor timestamp"))?
                .with_timezone(&Utc);
            if at < start
                || at >= end
                || cursor.event_id.is_empty()
                || timestamp(at) != cursor.effective_at
            {
                return Err(StoreError::InvalidRequestQuery(
                    "cursor must use canonical time within the query window",
                ));
            }
        }
        let mut statement = self
            .connection
            .prepare(&request_page_sql(after.is_some()))?;
        let rows = statement.query_map(
            params![
                thread_id,
                timestamp(start),
                timestamp(end),
                after.map(|cursor| cursor.effective_at.as_str()),
                after.map(|cursor| cursor.event_id.as_str()),
                (limit + 1) as i64,
            ],
            |row| {
                Ok(RetainedRequestObservation {
                    cursor: RetainedRequestCursor {
                        effective_at: row.get(0)?,
                        event_id: row.get(1)?,
                    },
                    turn_id: row.get(2)?,
                    model: row.get(3)?,
                    observed_account: row.get(4)?,
                    observed_project: row.get(5)?,
                    quality: parse_quality_column(&row.get::<_, String>(6)?, 6)?,
                    observed_account_confidence: parse_confidence_column(
                        &row.get::<_, String>(14)?,
                        14,
                    )?,
                    observed_project_confidence: parse_confidence_column(
                        &row.get::<_, String>(15)?,
                        15,
                    )?,
                    usage: TokenUsage {
                        input_tokens: u64_from_sql(row.get(7)?, 7)?,
                        cached_input_tokens: u64_from_sql(row.get(8)?, 8)?,
                        cache_write_input_tokens: u64_from_sql(row.get(9)?, 9)?,
                        cache_write_observed_input_tokens: u64_from_sql(row.get(10)?, 10)?,
                        output_tokens: u64_from_sql(row.get(11)?, 11)?,
                        reasoning_output_tokens: u64_from_sql(row.get(12)?, 12)?,
                        total_tokens: u64_from_sql(row.get(13)?, 13)?,
                    },
                })
            },
        )?;
        let mut observations = rows.collect::<Result<Vec<_>, _>>()?;
        let more = observations.len() > limit;
        observations.truncate(limit);
        let next = more.then(|| {
            observations
                .last()
                .expect("positive page limit")
                .cursor
                .clone()
        });
        Ok(RetainedRequestPage { observations, next })
    }
}
