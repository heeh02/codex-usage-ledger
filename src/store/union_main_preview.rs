//! Read-only integration of a resolved union through existing product queries.
//! Temporary views exist only on this connection; no stored policy is promoted.
use super::*;

impl LedgerStore {
    /// Select the request union on a live collector connection after its initial
    /// projection is ready. This changes query routing only, not retained facts.
    /// Subsequent writes invalidate readiness until incremental staging catches
    /// up; queries must never fall back to the legacy daily maximum.
    pub fn enable_source_union_queries(&mut self) -> StoreResult<()> {
        if self.union_main_preview {
            return self.refresh_effective_source_selection();
        }
        let transaction = self.connection.unchecked_transaction()?;
        if !self.union_main_projection_ready()? {
            return Err(StoreError::SnapshotUnavailable);
        }
        transaction.execute_batch(PREVIEW_VIEWS)?;
        transaction.commit()?;
        self.union_main_preview = true;
        self.exact_series_memo = Default::default();
        Ok(())
    }

    /// Open a current-schema ledger read-only and exercise the normal query
    /// services using selected request facts, including retained-only requests.
    /// This is an acceptance lane, not authorization to migrate historical facts.
    pub fn open_source_union_main_preview(path: impl AsRef<Path>) -> StoreResult<Self> {
        let store = Self::open_source_union_diagnostics(path)?;
        store.refresh_effective_source_selection()?;
        Ok(store)
    }

    /// Permit scoped diagnostic endpoints while ordinary aggregate queries keep
    /// their global readiness guard. No partial total is silently exposed.
    pub fn open_source_union_diagnostics(path: impl AsRef<Path>) -> StoreResult<Self> {
        let mut store = Self::open_read_only(path)?;
        // SQLITE_OPEN_READ_ONLY still protects the main file while TEMP DDL is
        // installed. Restore query_only before exposing the connection.
        store.connection.pragma_update(None, "query_only", "OFF")?;
        store.connection.execute_batch(PREVIEW_VIEWS)?;
        store.connection.pragma_update(None, "query_only", "ON")?;
        store.union_main_preview = true;
        Ok(store)
    }

    pub(super) fn union_main_projection_ready(&self) -> StoreResult<bool> {
        Ok(self.connection.query_row(
            "SELECT policy_version=2 AND pending=0 AND unresolved=0
             AND NOT EXISTS(SELECT 1 FROM measurement_union_backfill WHERE complete=0)
             FROM measurement_union_counts WHERE id=1",
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn is_source_union_main_preview(&self) -> bool {
        self.union_main_preview
    }
}

// Main queries already classify projectless threads and apply account/model/
// own/tree filters. Do not reimplement those product semantics in an audit DTO.
// Rollup local keys preserve the established Shanghai storage convention;
// exact-time consumers apply their requested timezone to source_timestamp.
const PREVIEW_VIEWS: &str = r#"
CREATE TEMP VIEW effective_usage_events AS
SELECT evidence_source || ':' || event_id AS event_id,
       effective_at AS observed_at, effective_at AS source_timestamp,
       thread_id, NULL AS parent_thread_id, model, NULL AS cwd,
       account_fingerprint, project_id,
       input_tokens, cached_input_tokens, cache_write_input_tokens,
       cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
       total_tokens, 'confirmed' AS quality
FROM main.measurement_union_selected;

CREATE TEMP VIEW effective_daily_usage_rollups AS
SELECT date(source_timestamp, '+8 hours') AS local_day,
       COALESCE(thread_id,'') AS thread_key,
       COALESCE(account_fingerprint,'') AS account_key,
       COALESCE(project_id,'') AS project_key, COALESCE(model,'') AS model_key,
       quality, COUNT(*) AS event_count,
       SUM(input_tokens) AS input_tokens, SUM(cached_input_tokens) AS cached_input_tokens,
       SUM(cache_write_input_tokens) AS cache_write_input_tokens,
       SUM(cache_write_observed_input_tokens) AS cache_write_observed_input_tokens,
       SUM(output_tokens) AS output_tokens, SUM(reasoning_output_tokens) AS reasoning_output_tokens,
       SUM(total_tokens) AS total_tokens, 'request_union' AS evidence_source
FROM temp.effective_usage_events
GROUP BY local_day,thread_key,account_key,project_key,model_key,quality;

CREATE TEMP VIEW effective_hourly_usage_rollups AS
SELECT strftime('%Y-%m-%dT%H:00', source_timestamp, '+8 hours') AS local_hour,
       COALESCE(thread_id,'') AS thread_key,
       COALESCE(account_fingerprint,'') AS account_key,
       COALESCE(project_id,'') AS project_key, COALESCE(model,'') AS model_key,
       quality, COUNT(*) AS event_count,
       SUM(input_tokens) AS input_tokens, SUM(cached_input_tokens) AS cached_input_tokens,
       SUM(cache_write_input_tokens) AS cache_write_input_tokens,
       SUM(cache_write_observed_input_tokens) AS cache_write_observed_input_tokens,
       SUM(output_tokens) AS output_tokens, SUM(reasoning_output_tokens) AS reasoning_output_tokens,
       SUM(total_tokens) AS total_tokens, 'request_union' AS evidence_source
FROM temp.effective_usage_events
GROUP BY local_hour,thread_key,account_key,project_key,model_key,quality;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::event;

    /// Optional private GUI fixture export; never reads a real Codex home.
    #[test]
    fn populated_ui_review_fixture() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("synthetic.sqlite3");
        let mut store = LedgerStore::open(&path).unwrap();
        let now = Utc::now();
        for (id, name) in [("demo-alpha", "Demo Alpha"), ("demo-beta", "Demo Beta")] {
            store
                .upsert_project(
                    &ProjectRecord {
                        project_id: id.into(),
                        project_name: name.into(),
                        roots: vec![],
                        git_identities: vec![],
                    },
                    now,
                )
                .unwrap();
        }
        let nodes = [
            ("alpha", None, Some("demo-alpha")),
            ("alpha-worker", Some("alpha"), Some("demo-alpha")),
            ("beta", None, Some("demo-beta")),
            ("beta-worker", Some("beta"), Some("demo-beta")),
            ("chat", None, None),
            ("chat-worker", Some("chat"), None),
        ];
        store
            .upsert_thread_catalog_batch(
                &nodes
                    .iter()
                    .map(|(id, parent, project)| ThreadCatalogRecord {
                        thread_id: (*id).into(),
                        parent_thread_id: parent.map(str::to_owned),
                        project_id: project.map(str::to_owned),
                        project_name: project.map(str::to_owned),
                        title: Some(format!("Demo · {id}")),
                        model: None,
                        agent_nickname: None,
                        agent_role: None,
                        agent_path: None,
                        depth: Some(u32::from(parent.is_some())),
                        created_at: now - ChronoDuration::days(400),
                        updated_at: now,
                        archived: false,
                        has_user_event: parent.is_none(),
                        source_kind: "state_5".into(),
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut expected = 0;
        for day in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 20, 40, 90, 400] {
            for (node, (thread, parent, project)) in nodes.iter().enumerate() {
                for occurrence in 0..3 {
                    let id = format!("demo-{day}-{node}-{occurrence}");
                    let mut row = event(
                        &id,
                        DataQuality::Confirmed,
                        (day * 100 + node as i64 * 10 + occurrence + 1) as u64,
                    );
                    let at = now
                        - ChronoDuration::days(day)
                        - ChronoDuration::minutes(node as i64 * 3 + occurrence + 1);
                    row.observed_at = at;
                    row.source_timestamp = Some(at);
                    row.thread_id = Some((*thread).into());
                    row.parent_thread_id = parent.map(str::to_owned);
                    row.model = Some(
                        ["gpt-5.6-sol", "gpt-5.6-luna", "gpt-6-astra"]
                            [(node + occurrence as usize) % 3]
                            .into(),
                    );
                    row.account_fingerprint = Some(
                        if (day + node as i64) % 2 == 0 {
                            "demoA000-account"
                        } else {
                            "demoB000-account"
                        }
                        .into(),
                    );
                    row.project.project_id = project.map(str::to_owned);
                    row.project.project_name = project.map(str::to_owned);
                    let input = (1_000_000 * (1 + day % 5)
                        + node as i64 * 111_000
                        + occurrence * 333_000) as u64;
                    row.usage = TokenUsage {
                        input_tokens: input,
                        cached_input_tokens: input * 9 / 10,
                        cache_write_input_tokens: input / 100,
                        cache_write_observed_input_tokens: if day % 2 == 0 { input } else { 0 },
                        output_tokens: 20_000,
                        reasoning_output_tokens: 8_000,
                        total_tokens: input + 20_000,
                    };
                    row.provenance.source_record_key = Some(id.clone());
                    expected += row.usage.total_tokens;
                    if occurrence != 1 {
                        store.upsert_event(&row).unwrap();
                    }
                    if occurrence != 0 {
                        row.event_id = format!("r-{id}");
                        let tx = store.connection.unchecked_transaction().unwrap();
                        upsert_reconstruction_event_in(
                            &tx,
                            &ReconstructionEvent {
                                event: row,
                                counter_epoch: 0,
                            },
                        )
                        .unwrap();
                        tx.commit().unwrap();
                    }
                }
            }
        }
        store.set_user_confirmed_account_count(Some(2)).unwrap();
        for _ in 0..100 {
            if store
                .stage_source_union_batch(1000, 1000, 10000)
                .unwrap()
                .projection_ready
            {
                break;
            }
        }
        drop(store);
        let preview = LedgerStore::open_source_union_main_preview(&path).unwrap();
        assert_eq!(
            preview
                .aggregate_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            expected
        );
        drop(preview);
        if let Some(output) = std::env::var_os("LEDGER_UI_REVIEW_FIXTURE_PATH") {
            create_review_shadow(&path, Path::new(&output)).unwrap();
        }
    }

    fn fixture(path: &Path) {
        let mut store = LedgerStore::open(path).unwrap();
        let at = "2026-04-01T00:00:00Z".parse().unwrap();
        store
            .upsert_thread_catalog_batch(&[ThreadCatalogRecord {
                thread_id: "thread".into(),
                parent_thread_id: None,
                project_id: Some("project-1".into()),
                project_name: Some("Demo".into()),
                title: Some("Synthetic conversation".into()),
                model: None,
                agent_nickname: None,
                agent_role: None,
                agent_path: None,
                depth: Some(0),
                created_at: at,
                updated_at: at,
                archived: false,
                has_user_event: true,
                source_kind: "state_5".into(),
            }])
            .unwrap();
        for (id, total, offset) in [("a", 100, 1), ("b", 200, 2), ("c", 300, 3)] {
            let mut row = event(id, DataQuality::Confirmed, offset);
            row.source_timestamp = Some("2026-04-01T00:00:00Z".parse().unwrap());
            row.provenance.source_record_key = Some(id.into());
            row.usage.input_tokens = total - 20;
            row.usage.cache_write_observed_input_tokens = total - 20;
            row.usage.total_tokens = total;
            row.model = Some(if id == "a" { "sampling-only" } else { "shared" }.into());
            if id != "c" {
                store.upsert_event(&row).unwrap();
            }
            if id != "a" {
                row.event_id = format!("r-{id}");
                let tx = store.connection.unchecked_transaction().unwrap();
                upsert_reconstruction_event_in(
                    &tx,
                    &ReconstructionEvent {
                        event: row,
                        counter_epoch: 0,
                    },
                )
                .unwrap();
                tx.commit().unwrap();
            }
        }
        // Compacted sampling remains in the retained ledger, not usage_events.
        store
            .connection
            .execute("DELETE FROM usage_events WHERE event_id='a'", [])
            .unwrap();
        for _ in 0..100 {
            if store
                .stage_source_union_batch(10, 10, 100)
                .unwrap()
                .projection_ready
            {
                break;
            }
        }
        store.refresh_effective_source_selection().unwrap();
    }

    #[test]
    fn live_union_keeps_ingesting_without_daily_max_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.sqlite3");
        fixture(&path);
        let mut store = LedgerStore::open(&path).unwrap();
        store.enable_source_union_queries().unwrap();
        store.enable_source_union_queries().unwrap();
        let filter = AggregateFilter::default();
        assert_eq!(
            store.aggregate_usage(&filter).unwrap().usage.total_tokens,
            600
        );
        let mut row = event("live-new", DataQuality::Confirmed, 4);
        row.provenance.source_record_key = Some("live-new".into());
        let added = row.usage.total_tokens;
        store.upsert_event(&row).unwrap();
        assert!(matches!(
            store.aggregate_usage(&filter),
            Err(StoreError::SnapshotUnavailable)
        ));
        for _ in 0..100 {
            if store
                .stage_source_union_batch(10, 10, 100)
                .unwrap()
                .projection_ready
            {
                break;
            }
        }
        assert_eq!(
            store.aggregate_usage(&filter).unwrap().usage.total_tokens,
            600 + added
        );
    }

    #[test]
    fn live_union_activation_rejects_unprepared_projection() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store
            .upsert_event(&event("pending", DataQuality::Confirmed, 1))
            .unwrap();
        assert!(matches!(
            store.enable_source_union_queries(),
            Err(StoreError::SnapshotUnavailable)
        ));
        assert!(!store.is_source_union_main_preview());
        let count: i64 = store
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_temp_master WHERE name='effective_usage_events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn existing_product_queries_share_union_without_retained_double_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.sqlite3");
        fixture(&path);
        let before = std::fs::read(&path).unwrap();
        let reader = LedgerStore::open_source_union_main_preview(&path).unwrap();
        reader
            .with_usage_snapshot(|store| {
                let filter = AggregateFilter::default();
                let aggregate = store.aggregate_usage(&filter)?;
                assert_eq!(aggregate.usage.total_tokens, 600);
                assert_eq!(aggregate.event_count, 3);
                assert_eq!(
                    store.aggregate_rollup_usage(&filter)?.usage,
                    aggregate.usage
                );
                for dimension in [
                    AggregateDimension::Account,
                    AggregateDimension::Project,
                    AggregateDimension::Model,
                    AggregateDimension::Thread,
                ] {
                    for rows in [
                        store.aggregate_by(dimension, &filter)?,
                        store.aggregate_rollup_by(dimension, &filter)?,
                    ] {
                        let mut sum = TokenUsage::default();
                        for row in rows {
                            checked_add_usage(&mut sum, row.usage)?;
                        }
                        assert_eq!(sum, aggregate.usage);
                    }
                }
                for grain in [TimeGrain::Hour, TimeGrain::Day] {
                    for rows in [
                        store.aggregate_time_series(grain, None, &filter)?,
                        store.aggregate_exact_time_series(grain, None, &filter, "Asia/Shanghai")?,
                    ] {
                        assert_eq!(
                            rows.iter().map(|row| row.usage.total_tokens).sum::<u64>(),
                            600
                        );
                    }
                }
                assert_eq!(
                    store
                        .aggregate_usage(&AggregateFilter {
                            model: Some("sampling-only".into()),
                            ..filter
                        })?
                        .usage
                        .total_tokens,
                    100
                );
                Ok(())
            })
            .unwrap();
        assert!(
            reader
                .connection
                .execute("DELETE FROM measurement_union_selected", [])
                .is_err()
        );
        drop(reader);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let legacy = LedgerStore::open_read_only(&path).unwrap();
        assert_eq!(
            legacy
                .aggregate_rollup_usage(&AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            500
        );
    }

    #[test]
    fn rejects_pending_unresolved_and_changes_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.sqlite3");
        fixture(&path);
        let reader = LedgerStore::open_source_union_main_preview(&path).unwrap();
        let mut writer = LedgerStore::open(&path).unwrap();
        writer
            .upsert_event(&event("missing-key", DataQuality::Confirmed, 20))
            .unwrap();
        assert!(
            reader
                .with_usage_snapshot(|store| store.aggregate_usage(&AggregateFilter::default()))
                .is_err()
        );
        assert!(LedgerStore::open_source_union_main_preview(&path).is_err());
        for _ in 0..100 {
            if writer
                .stage_source_union_batch(10, 10, 100)
                .unwrap()
                .projection_ready
            {
                break;
            }
        }
        assert!(LedgerStore::open_source_union_main_preview(&path).is_err());
    }

    #[tokio::test]
    async fn actual_dashboard_bundle_keeps_preview_through_async_query_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.sqlite3");
        fixture(&path);
        let state = crate::api::ApiState::with_store(
            LedgerStore::open_source_union_main_preview(&path).unwrap(),
        );
        for (model, total) in [(None, 600), (Some("sampling-only"), 100)] {
            for timezone in ["UTC", "Asia/Shanghai", "America/New_York"] {
                let query = crate::api::UsageQuery {
                    period: Some("custom".into()),
                    start_date: Some("2026-03-31".into()),
                    end_date: Some("2026-04-02".into()),
                    timezone: Some(timezone.into()),
                    session: Some("thread".into()),
                    model: model.map(str::to_owned),
                    account: Some("acct-fp".into()),
                    project: Some("project-1".into()),
                    ..Default::default()
                };
                let bundle = state.bundle_json(query).await.unwrap();
                assert_eq!(bundle["collection"]["usagePolicy"], "request_union_v2");
                assert_eq!(bundle["collection"]["mode"], "union-preview");
                assert_eq!(bundle["summary"]["usage"]["confirmed"]["total"], total);
                assert_eq!(
                    bundle["explorer"]["selectedSession"]["treeUsage"]["total"],
                    total
                );
                assert_eq!(
                    bundle["explorer"]["selectedSession"]["ownUsage"]["total"],
                    total
                );
                for dimension in ["model", "account", "project"] {
                    let sum: u64 = bundle["breakdowns"][dimension]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| row["usage"]["confirmed"]["total"].as_u64().unwrap())
                        .sum();
                    assert_eq!(sum, total);
                }
                let sum: u64 = bundle["timeseries"]["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| row["confirmed"]["total"].as_u64().unwrap())
                    .sum();
                assert_eq!(sum, total);
            }
        }
    }
}
