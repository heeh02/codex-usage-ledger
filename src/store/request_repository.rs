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
