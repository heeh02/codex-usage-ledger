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
            audit_version: 2,
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
            next: None,
        };
        let page = self.retained_request_page_for_scope(scope, after, limit)?;
        let mut groups = BTreeMap::<CandidateOverlapStatus, CandidateAuditGroup>::new();
        for observation in page.observations {
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
            });
        }
        report.groups = groups.into_values().collect();
        report.next = page.next;
        transaction.commit()?;
        Ok(report)
    }
}
