use super::*;

impl LedgerStore {
    /// One read snapshot per page. Category amounts describe retained confirmed
    /// observations, not a de-duplicated union or replacement accounting policy.
    pub fn audit_candidate_page(
        &self,
        scope: RetainedRequestScope<'_>,
        after: Option<&RetainedRequestCursor>,
        limit: usize,
    ) -> StoreResult<CandidateAuditPage> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut report = CandidateAuditPage {
            scope: "own_retained_request_page",
            group_totals_scope: "this_page_confirmed_observations_only",
            schema_version: self.schema_version()?,
            audit_version: 3,
            read_only: true,
            request_equality_proven: false,
            history_complete: false,
            thread_id: scope.thread_id.to_owned(),
            start: scope.start,
            end: scope.end,
            selected_account: scope.account.map(str::to_owned),
            selected_model: scope.model.map(str::to_owned),
            rows: Vec::new(),
            groups: Vec::new(),
            day_policy_contexts: Vec::new(),
            next: None,
        };
        let page = self.retained_request_page_for_scope(scope, after, limit)?;
        let mut groups = BTreeMap::<CandidateOverlapStatus, CandidateAuditGroup>::new();
        let mut policies = BTreeMap::<String, DayPolicyContext>::new();
        for observation in page.observations {
            let policy_day = DateTime::parse_from_rfc3339(&observation.cursor.effective_at)
                .ok()
                .map(|at| {
                    at.with_timezone(&chrono_tz::Asia::Shanghai)
                        .date_naive()
                        .to_string()
                });
            let selected_source = if let Some(day) = policy_day.as_ref() {
                if !policies.contains_key(day) {
                    policies.insert(day.clone(), self.audit_day_policy(&report.thread_id, day)?);
                }
                policies.get(day).and_then(|policy| policy.selected_source)
            } else {
                None
            };
            let comparison = self
                .candidate_comparison(&observation.cursor.event_id)?
                .ok_or(StoreError::InvalidRequestQuery(
                    "retained observation disappeared inside audit snapshot",
                ))?;
            let status = comparison.status;
            let confirmed_usage =
                (observation.quality == DataQuality::Confirmed).then_some(observation.usage);
            let group = groups.entry(status).or_insert(CandidateAuditGroup {
                status,
                records: 0,
                confirmed_records: 0,
                confirmed_usage: None,
            });
            group.records += 1;
            if let Some(usage) = confirmed_usage {
                usage
                    .validate()
                    .map_err(StoreError::InvalidConfirmedUsage)?;
                group.confirmed_records += 1;
                checked_add_usage(
                    group
                        .confirmed_usage
                        .get_or_insert_with(TokenUsage::default),
                    usage,
                )?;
            }
            report.rows.push(CandidateAuditRow {
                cursor: observation.cursor,
                source_model: observation.model,
                status,
                quality: observation.quality,
                confirmed_usage,
                comparison,
                policy_day,
                retained_side_selected_by_day_policy: if observation.quality
                    == DataQuality::Confirmed
                {
                    selected_source.map(|source| source == DayPolicySource::Sampling)
                } else {
                    None
                },
            });
        }
        report.groups = groups.into_values().collect();
        report.day_policy_contexts = policies.into_values().collect();
        report.next = page.next;
        transaction.commit()?;
        Ok(report)
    }

    /// Mirrors the current full-thread/day rule without refreshing its cached
    /// projection. These are NOT account/model/window-filtered usage totals.
    fn audit_day_policy(&self, thread: &str, day: &str) -> StoreResult<DayPolicyContext> {
        let (sampling_records, sampling_tokens, reconstruction_records, reconstruction_tokens) = self.connection.query_row(
            "WITH sampling AS (
                SELECT COALESCE(SUM(event_count),0) records,COALESCE(SUM(total_tokens),0) tokens
                FROM daily_usage_rollups WHERE local_day=?1 AND thread_key=?2 AND quality='confirmed'
             ), rebuilt AS (
                SELECT COALESCE(SUM(event_count),0) records,COALESCE(SUM(total_tokens),0) tokens
                FROM reconstruction_daily_rollups WHERE local_day=?1 AND thread_key=?2
             ) SELECT sampling.records,sampling.tokens,rebuilt.records,rebuilt.tokens FROM sampling,rebuilt",
            params![day, thread], |row| Ok((u64_from_sql(row.get(0)?,0)?,u64_from_sql(row.get(1)?,1)?,u64_from_sql(row.get(2)?,2)?,u64_from_sql(row.get(3)?,3)?)),
        )?;
        let selected_source = if sampling_records == 0 && reconstruction_records == 0 {
            None
        } else if reconstruction_tokens > sampling_tokens {
            Some(DayPolicySource::Reconstruction)
        } else {
            Some(DayPolicySource::Sampling)
        };
        Ok(DayPolicyContext {
            policy: "max_thread_day_v1",
            scope: "full_storage_day_all_accounts_and_models",
            day: day.to_owned(),
            sampling_records,
            sampling_tokens,
            reconstruction_records,
            reconstruction_tokens,
            selected_source,
        })
    }
}
