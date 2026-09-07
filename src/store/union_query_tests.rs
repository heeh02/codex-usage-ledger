use super::*;
use crate::store::tests::event;

fn query() -> SourceUnionQuery {
    SourceUnionQuery {
        start: "2026-01-01T00:00:00Z".parse().unwrap(),
        end: "2027-01-01T00:00:00Z".parse().unwrap(),
        timezone: "UTC".into(),
        grain: SourceUnionGrain::Day,
        account: None,
        project: None,
        model: None,
        thread: None,
    }
}
fn sample(id: &str, key: &str, at: &str, offset: u64) -> UsageEvent {
    let mut row = event(id, DataQuality::Confirmed, offset);
    row.source_timestamp = Some(at.parse().unwrap());
    row.provenance.source_record_key = Some(key.into());
    row
}
fn reconstructed(store: &LedgerStore, row: UsageEvent) {
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
fn drain(store: &mut LedgerStore) {
    for _ in 0..100 {
        if store
            .stage_source_union_batch(2, 2, 20)
            .unwrap()
            .projection_ready
        {
            return;
        }
    }
    panic!("fixture did not finish");
}
fn conserves(data: &SourceUnionData) {
    for groups in [
        &data.by_time,
        &data.by_account,
        &data.by_model,
        &data.by_project,
        &data.by_thread,
    ] {
        assert_eq!(groups.iter().map(|g| g.records).sum::<u64>(), data.records);
        let mut sum = None;
        for bucket in groups {
            checked_add_usage(sum.get_or_insert_default(), bucket.usage).unwrap();
        }
        assert_eq!(sum, data.usage);
    }
}

fn fact_rows(connection: &Connection) -> Vec<Vec<String>> {
    [
        "retained_request_evidence",
        "retained_request_assignments",
        "source_record_evidence",
        "measurement_union_selected",
        "measurement_union_groups",
        "reconstruction_usage_events",
    ]
    .iter()
    .map(|table| {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
        let columns = statement.column_count();
        let mut rows = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|i| row.get_ref(i).map(|value| format!("{value:?}")))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|values| values.join("|"))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.sort();
        rows
    })
    .collect()
}

#[test]
fn new_reader_recovers_the_600_token_counterexample_and_sampling_only_model() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut observations = Vec::new();
    for (id, total, offset) in [("a", 100, 1), ("b", 200, 2), ("c", 300, 3)] {
        let mut row = sample(id, id, "2026-04-01T00:00:00Z", offset);
        row.usage.input_tokens = total - 20;
        row.usage.cache_write_observed_input_tokens = total - 20;
        row.usage.total_tokens = total;
        row.model = Some(
            if id == "a" {
                "sampling-only"
            } else {
                "shared-model"
            }
            .into(),
        );
        observations.push(row);
    }
    store.upsert_event(&observations[0]).unwrap();
    store.upsert_event(&observations[1]).unwrap();
    let mut peer = observations[1].clone();
    peer.event_id = "b-peer".into();
    reconstructed(&store, peer);
    reconstructed(&store, observations[2].clone());
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap()
            .usage
            .total_tokens,
        500
    );
    drain(&mut store);
    let data = store
        .read_source_union_projection(&query())
        .unwrap()
        .data
        .unwrap();
    assert_eq!(data.usage.unwrap().total_tokens, 600);
    conserves(&data);
    let model = store
        .read_source_union_projection(&SourceUnionQuery {
            model: Some("sampling-only".into()),
            ..query()
        })
        .unwrap()
        .data
        .unwrap();
    assert_eq!(model.usage.unwrap().total_tokens, 100);
    assert_eq!(model.records, 1);
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap()
            .usage
            .total_tokens,
        500,
        "diagnostic read must not promote itself"
    );
}

#[test]
fn candidate_query_conserves_all_components_and_filters_after_shared_selection() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut a = sample("a", "a", "2026-01-31T23:59:59.950Z", 1);
    a.model = Some("sampling-only".into());
    a.account_fingerprint = None;
    a.project.project_id = None;
    let mut b = sample("b", "b", "2026-02-01T00:00:00.050Z", 2);
    b.model = Some("shared".into());
    b.account_fingerprint = Some("unknown".into());
    b.provenance.sampling_receipt_key = None;
    let mut c = sample("c", "c", "2026-03-02T00:00:00Z", 3);
    c.model = Some("reconstruction-only".into());
    c.account_fingerprint = Some("account".into());
    c.thread_id = Some("child".into());
    c.project.project_id = Some("project-other".into());
    store.upsert_event(&a).unwrap();
    store.upsert_event(&b).unwrap();
    let mut peer = b.clone();
    peer.event_id = "b-peer".into();
    peer.source_timestamp = Some("2026-01-31T23:59:59.950Z".parse().unwrap());
    peer.usage.cache_write_observed_input_tokens = 0;
    reconstructed(&store, peer);
    reconstructed(&store, c);
    drain(&mut store);
    let before = store.connection.total_changes();
    for timezone in ["UTC", "Asia/Shanghai", "America/New_York"] {
        for grain in [
            SourceUnionGrain::Hour,
            SourceUnionGrain::Day,
            SourceUnionGrain::Week,
            SourceUnionGrain::Month,
            SourceUnionGrain::Year,
        ] {
            let report = store
                .read_source_union_projection(&SourceUnionQuery {
                    timezone: timezone.into(),
                    grain,
                    ..query()
                })
                .unwrap();
            assert_eq!(report.status, "available");
            assert!(!report.history_complete && !report.production_policy_changed);
            let data = report.data.unwrap();
            assert_eq!(data.records, 3);
            conserves(&data);
            assert_eq!(data.usage.unwrap().total_tokens, 360);
            assert_eq!(data.usage.unwrap().cache_write_observed_input_tokens, 200);
            assert!(data.by_account.iter().any(|b| b.key.is_none()));
            assert!(
                data.by_account
                    .iter()
                    .any(|b| b.key.as_deref() == Some("unknown"))
            );
        }
    }
    let mut filtered = query();
    filtered.account = Some("unknown".into());
    let data = store
        .read_source_union_projection(&filtered)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(data.records, 1);
    conserves(&data);
    for (model, count) in [
        ("sampling-only", 1),
        ("shared", 1),
        ("reconstruction-only", 1),
        ("x' OR 1=1 --", 0),
    ] {
        let data = store
            .read_source_union_projection(&SourceUnionQuery {
                model: Some(model.into()),
                ..query()
            })
            .unwrap()
            .data
            .unwrap();
        assert_eq!(data.records, count);
        conserves(&data);
    }
    let january = store
        .read_source_union_projection(&SourceUnionQuery {
            end: "2026-02-01T00:00:00Z".parse().unwrap(),
            ..query()
        })
        .unwrap()
        .data
        .unwrap();
    assert_eq!(
        january.records, 1,
        "shared peer must not create a second January observation"
    );
    let combined = store
        .read_source_union_projection(&SourceUnionQuery {
            account: Some("account".into()),
            project: Some("project-other".into()),
            model: Some("reconstruction-only".into()),
            thread: Some("child".into()),
            ..query()
        })
        .unwrap()
        .data
        .unwrap();
    assert_eq!(combined.records, 1);
    conserves(&combined);
    let month = store
        .read_source_union_projection(&SourceUnionQuery {
            timezone: "Asia/Shanghai".into(),
            grain: SourceUnionGrain::Month,
            ..query()
        })
        .unwrap()
        .data
        .unwrap();
    assert_eq!(
        month
            .by_time
            .iter()
            .map(|b| (b.key.as_deref(), b.records))
            .collect::<Vec<_>>(),
        vec![(Some("2026-02"), 2), (Some("2026-03"), 1)]
    );
    let week = store
        .read_source_union_projection(&SourceUnionQuery {
            grain: SourceUnionGrain::Week,
            ..query()
        })
        .unwrap()
        .data
        .unwrap();
    assert_eq!(
        week.by_time
            .iter()
            .map(|b| (b.key.as_deref(), b.records))
            .collect::<Vec<_>>(),
        vec![(Some("2026-01-26"), 2), (Some("2026-03-02"), 1)]
    );
    assert_eq!(store.connection.total_changes(), before);
}

#[test]
fn guards_do_not_hide_out_of_scope_conflicts_or_treat_empty_as_observed_zero() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let empty = store.read_source_union_projection(&query()).unwrap();
    assert_eq!(empty.status, "no_records");
    assert!(empty.data.unwrap().usage.is_none());
    let mut zero = sample("zero", "zero", "2026-03-01T00:00:00Z", 1);
    zero.usage = TokenUsage::default();
    store.upsert_event(&zero).unwrap();
    let pending = store.read_source_union_projection(&query()).unwrap();
    assert_eq!(pending.status, "pending");
    assert!(pending.data.is_none());
    drain(&mut store);
    let data = store
        .read_source_union_projection(&query())
        .unwrap()
        .data
        .unwrap();
    assert_eq!(data.records, 1);
    assert_eq!(data.usage, Some(TokenUsage::default()));
    let mut peer = zero;
    peer.event_id = "peer".into();
    peer.account_fingerprint = Some("other".into());
    reconstructed(&store, peer);
    drain(&mut store);
    let query = SourceUnionQuery {
        model: Some("unrelated".into()),
        ..query()
    };
    let report = store.read_source_union_projection(&query).unwrap();
    assert_eq!(report.status, "unresolved");
    assert!(report.data.is_none());
    let mut invalid = query.clone();
    invalid.end = invalid.start;
    assert!(store.read_source_union_projection(&invalid).is_err());
    invalid = query;
    invalid.timezone = "invalid".into();
    assert!(store.read_source_union_projection(&invalid).is_err());
}

#[test]
fn schema40_preserves_source_and_candidate_rows_and_queries_use_range_index() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ledger.sqlite3");
    let original_rows;
    {
        let mut connection = Connection::open(&path).unwrap();
        migrations::create_legacy_schema(&mut connection, 39).unwrap();
        let mut store = LedgerStore {
            connection,
            exact_series_memo: Default::default(),
        };
        store
            .upsert_event(&sample("a", "a", "2026-04-01T00:00:00Z", 1))
            .unwrap();
        drain(&mut store);
        original_rows = fact_rows(&store.connection);
    }
    let store = LedgerStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), 40);
    assert_eq!(fact_rows(&store.connection), original_rows);
    assert_eq!(
        store
            .read_source_union_projection(&query())
            .unwrap()
            .data
            .unwrap()
            .records,
        1
    );
    let (sql, values) = query_sql(&query());
    let plan = store
        .connection
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(params_from_iter(values), |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join(" ");
    assert!(plan.contains("measurement_union_selected_time"), "{plan}");
    drop(store);
    let before = std::fs::read(&path).unwrap();
    let readonly = LedgerStore::open_read_only(&path).unwrap();
    assert_eq!(
        readonly
            .read_source_union_projection(&query())
            .unwrap()
            .data
            .unwrap()
            .records,
        1
    );
    drop(readonly);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn repeated_local_hours_and_exact_subsecond_edges_are_not_merged_or_rounded() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    for (id, at, offset) in [
        ("a", "2026-11-01T05:30:00Z", 1),
        ("b", "2026-11-01T06:30:00Z", 2),
    ] {
        store.upsert_event(&sample(id, id, at, offset)).unwrap();
    }
    drain(&mut store);
    let report = store
        .read_source_union_projection(&SourceUnionQuery {
            timezone: "America/New_York".into(),
            grain: SourceUnionGrain::Hour,
            ..query()
        })
        .unwrap();
    let data = report.data.unwrap();
    assert_eq!(data.by_time.len(), 2);
    conserves(&data);
    assert_eq!(
        data.by_time[0].key.as_deref(),
        Some("2026-11-01T01:00:00-04:00")
    );
    assert_eq!(
        data.by_time[1].key.as_deref(),
        Some("2026-11-01T01:00:00-05:00")
    );
    let mut q = query();
    q.start = "2026-11-01T05:30:00Z".parse().unwrap();
    q.end = q.start + ChronoDuration::nanoseconds(1);
    assert_eq!(
        store
            .read_source_union_projection(&q)
            .unwrap()
            .data
            .unwrap()
            .records,
        1
    );
    q.start = q.end;
    q.end += ChronoDuration::nanoseconds(1);
    assert_eq!(
        store.read_source_union_projection(&q).unwrap().status,
        "no_records"
    );
}

#[test]
fn candidate_bucket_cap_fails_without_truncation_and_empty_ranges_do_not_scan_history() {
    // Direct synthetic candidate rows isolate reader budget behavior; these are
    // not an identity-ingestion or production-promotion acceptance fixture.
    let store = LedgerStore::open_in_memory().unwrap();
    store
        .connection
        .execute_batch(
            "BEGIN;
        WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10001)
        INSERT INTO measurement_union_groups SELECT 'key',CAST(i AS TEXT),1,NULL FROM n;
        INSERT INTO measurement_union_selected
        SELECT kind,identity,'sampling',identity,'2026-01-01T00:00:00.000000000Z',
            'thread',identity,NULL,NULL,100,40,10,100,20,5,120 FROM measurement_union_groups;
        COMMIT;",
        )
        .unwrap();
    assert!(matches!(
        store.read_source_union_projection(&query()),
        Err(StoreError::InvalidRequestQuery(
            "too many union buckets; narrow scope"
        ))
    ));
    let selected = SourceUnionQuery {
        model: Some("1".into()),
        ..query()
    };
    assert_eq!(
        store
            .read_source_union_projection(&selected)
            .unwrap()
            .data
            .unwrap()
            .records,
        1
    );
    let mut empty = query();
    empty.start = "2026-12-01T00:00:00Z".parse().unwrap();
    let (sql, values) = query_sql(&empty);
    let mut statement = store.connection.prepare(&sql).unwrap();
    {
        let mut rows = statement.query(params_from_iter(values)).unwrap();
        assert!(rows.next().unwrap().is_none());
    }
    assert_eq!(
        statement.get_status(rusqlite::StatementStatus::FullscanStep),
        0
    );
    assert!(statement.get_status(rusqlite::StatementStatus::VmStep) < 100);
    store
        .connection
        .execute(
            "UPDATE measurement_union_counts SET policy_version=999 WHERE id=1",
            [],
        )
        .unwrap();
    let unsupported = store.read_source_union_projection(&selected).unwrap();
    assert_eq!(unsupported.status, "pending");
    assert!(unsupported.data.is_none());
}
