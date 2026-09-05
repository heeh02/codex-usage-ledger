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
          AND (?7 IS NULL OR EXISTS(
              SELECT 1 FROM retained_request_assignments assigned
              WHERE assigned.event_id=retained_request_evidence.event_id
                AND assigned.account_fingerprint=?7))
          AND (?8 IS NULL OR model=?8)
        ORDER BY effective_at, event_id LIMIT ?6"
    )
}

impl LedgerStore {
    /// Scoped retained observations, not proof of a complete turn or total ledger.
    /// Unidentified requests stay independent; aggregation precedes pagination.
    pub fn retained_turn_page(
        &self,
        scope: RetainedRequestScope<'_>,
        offset: usize,
        limit: usize,
    ) -> StoreResult<RetainedTurnPage> {
        if scope.start >= scope.end || scope.thread_id.is_empty() || !(1..=500).contains(&limit) {
            return Err(StoreError::InvalidRequestQuery(
                "invalid turn scope or page size",
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT turn_id,MIN(effective_at),MAX(effective_at),COUNT(*),
                SUM(quality='confirmed'),
                SUM(CASE WHEN quality='confirmed' THEN input_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN cached_input_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN cache_write_input_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN cache_write_observed_input_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN output_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN reasoning_output_tokens ELSE 0 END),
                SUM(CASE WHEN quality='confirmed' THEN total_tokens ELSE 0 END),
                CASE WHEN turn_id IS NULL THEN 'request:'||event_id ELSE 'turn:'||turn_id END
             FROM retained_request_evidence kept
             WHERE thread_id=?1 AND effective_at>=?2 AND effective_at<?3
               AND (?4 IS NULL OR EXISTS(SELECT 1 FROM retained_request_assignments assigned
                   WHERE assigned.event_id=kept.event_id AND assigned.account_fingerprint=?4))
               AND (?5 IS NULL OR model=?5)
             GROUP BY turn_id,CASE WHEN turn_id IS NULL THEN event_id ELSE '' END
             ORDER BY MIN(effective_at),MIN(event_id)
             LIMIT ?6 OFFSET ?7"
        )?;
        let rows = statement.query_map(
            params![
                scope.thread_id,
                timestamp(scope.start),
                timestamp(scope.end),
                scope.account,
                scope.model,
                (limit + 1) as i64,
                sql_u64(offset as u64, "turn_offset")?
            ],
            |row| {
                Ok(RetainedTurnObservation {
                    group_id: row.get(12)?,
                    turn_id: row.get(0)?,
                    first_at: row.get(1)?,
                    last_at: row.get(2)?,
                    request_count: u64_from_sql(row.get(3)?, 3)?,
                    confirmed_request_count: u64_from_sql(row.get(4)?, 4)?,
                    usage: TokenUsage {
                        input_tokens: u64_from_sql(row.get(5)?, 5)?,
                        cached_input_tokens: u64_from_sql(row.get(6)?, 6)?,
                        cache_write_input_tokens: u64_from_sql(row.get(7)?, 7)?,
                        cache_write_observed_input_tokens: u64_from_sql(row.get(8)?, 8)?,
                        output_tokens: u64_from_sql(row.get(9)?, 9)?,
                        reasoning_output_tokens: u64_from_sql(row.get(10)?, 10)?,
                        total_tokens: u64_from_sql(row.get(11)?, 11)?,
                    },
                })
            },
        )?;
        let mut observations = rows.collect::<Result<Vec<_>, _>>()?;
        let next_offset = (observations.len() > limit).then(|| offset.saturating_add(limit));
        observations.truncate(limit);
        Ok(RetainedTurnPage {
            observations,
            next_offset,
        })
    }

    /// Checks one source candidate without rewriting source choices or totals.
    pub fn request_candidate_overlap(&self, event_id: &str) -> StoreResult<CandidateOverlapStatus> {
        Ok(self
            .candidate_comparison(event_id)?
            .map(|comparison| comparison.status)
            .unwrap_or(CandidateOverlapStatus::RequestUnavailable))
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
        self.retained_request_page_for_scope(
            RetainedRequestScope {
                thread_id,
                start,
                end,
                account: None,
                model: None,
            },
            after,
            limit,
        )
    }

    pub fn retained_request_page_for_scope(
        &self,
        scope: RetainedRequestScope<'_>,
        after: Option<&RetainedRequestCursor>,
        limit: usize,
    ) -> StoreResult<RetainedRequestPage> {
        let RetainedRequestScope {
            thread_id,
            start,
            end,
            account,
            model,
        } = scope;
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
                account,
                model,
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
