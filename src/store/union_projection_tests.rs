use super::*;
use crate::store::tests::event;

fn keyed(id: &str, key: &str, offset: u64) -> UsageEvent {
    let mut result = event(id, DataQuality::Confirmed, offset);
    result.provenance.source_record_key = Some(key.into());
    result
}

fn reconstruct(store: &LedgerStore, event: UsageEvent) {
    let transaction = store.connection.unchecked_transaction().unwrap();
    upsert_reconstruction_event_in(
        &transaction,
        &ReconstructionEvent {
            event,
            counter_epoch: 0,
        },
    )
    .unwrap();
    transaction.commit().unwrap();
}

fn drain(store: &mut LedgerStore) -> UnionProjectionProgress {
    for _ in 0..100 {
        let progress = store.stage_source_union_batch(2, 2, 20).unwrap();
        if progress.projection_ready {
            return progress;
        }
    }
    panic!("synthetic projection never settled");
}

fn selected_usage(store: &LedgerStore) -> TokenUsage {
    store.connection.query_row(
        "SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(cached_input_tokens),0),
            COALESCE(SUM(cache_write_input_tokens),0),COALESCE(SUM(cache_write_observed_input_tokens),0),
            COALESCE(SUM(output_tokens),0),COALESCE(SUM(reasoning_output_tokens),0),COALESCE(SUM(total_tokens),0)
         FROM measurement_union_selected",[],read_usage
    ).unwrap()
}

fn read_usage(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenUsage> {
    Ok(TokenUsage {
        input_tokens: u64_from_sql(row.get(0)?, 0)?,
        cached_input_tokens: u64_from_sql(row.get(1)?, 1)?,
        cache_write_input_tokens: u64_from_sql(row.get(2)?, 2)?,
        cache_write_observed_input_tokens: u64_from_sql(row.get(3)?, 3)?,
        output_tokens: u64_from_sql(row.get(4)?, 4)?,
        reasoning_output_tokens: u64_from_sql(row.get(5)?, 5)?,
        total_tokens: u64_from_sql(row.get(6)?, 6)?,
    })
}

fn check_counts(store: &LedgerStore) {
    let actual: (i64, i64, i64) = store
        .connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM measurement_union_dirty),
        (SELECT COUNT(*) FROM measurement_union_selected),
        (SELECT COUNT(*) FROM measurement_union_groups WHERE unresolved_reason IS NOT NULL)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let cached: (i64, i64, i64) = store
        .connection
        .query_row(
            "SELECT pending,selected,unresolved FROM measurement_union_counts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, cached);
}

fn fact_snapshot(store: &LedgerStore) -> Vec<Vec<String>> {
    [
        "usage_events",
        "retained_request_evidence",
        "retained_request_assignments",
        "source_record_evidence",
        "reconstruction_usage_events",
        "daily_usage_rollups",
        "reconstruction_daily_rollups",
        "file_cursors",
        "effective_thread_day_source",
    ]
    .iter()
    .map(|table| {
        let mut statement = store
            .connection
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
        let columns = statement.column_count();
        let mut rows = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|i| row.get_ref(i).map(|value| format!("{value:?}")))
                    .collect::<rusqlite::Result<Vec<_>>>()
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
fn incremental_union_preserves_components_dimensions_and_noop_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("projection.sqlite3");
    let mut store = LedgerStore::open(&path).unwrap();
    let mut a = keyed("sample-a", "a", 1);
    a.model = Some("sampling-only-model".into());
    let b = keyed("sample-b", "b", 2);
    store.upsert_event(&a).unwrap();
    store.upsert_event(&b).unwrap();
    let mut peer = b.clone();
    peer.event_id = "rebuilt-b".into();
    peer.source_timestamp = Some(b.observed_at + ChronoDuration::milliseconds(100));
    reconstruct(&store, peer);
    reconstruct(&store, keyed("rebuilt-c", "c", 3));
    let baseline = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap()
        .usage;
    let facts = fact_snapshot(&store);
    let progress = drain(&mut store);
    assert_eq!(
        fact_snapshot(&store),
        facts,
        "candidate writes cannot change source facts or old selector"
    );
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (3, 0)
    );
    assert!(!progress.production_policy_changed && !progress.history_complete);
    let expected = TokenUsage {
        input_tokens: 300,
        cached_input_tokens: 120,
        cache_write_input_tokens: 30,
        cache_write_observed_input_tokens: 300,
        output_tokens: 60,
        reasoning_output_tokens: 15,
        total_tokens: 360,
    };
    assert_eq!(selected_usage(&store), expected);
    assert_eq!(
        baseline.total_tokens, 240,
        "old policy remains isolated from staging"
    );
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap()
            .usage,
        baseline
    );
    for column in [
        "account_fingerprint",
        "model",
        "project_id",
        "thread_id",
        "date(effective_at)",
    ] {
        let sql=format!("SELECT SUM(input_tokens),SUM(cached_input_tokens),SUM(cache_write_input_tokens),
            SUM(cache_write_observed_input_tokens),SUM(output_tokens),SUM(reasoning_output_tokens),SUM(total_tokens)
            FROM measurement_union_selected GROUP BY {column}");
        let rows = store
            .connection
            .prepare(&sql)
            .unwrap()
            .query_map([], read_usage)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut total = TokenUsage::default();
        for usage in rows {
            checked_add_usage(&mut total, usage).unwrap();
        }
        assert_eq!(total, expected);
    }
    let sample_only: i64 = store
        .connection
        .query_row(
            "SELECT total_tokens FROM measurement_union_selected WHERE model='sampling-only-model'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(sample_only, 120);
    check_counts(&store);
    drop(store);
    let mut store = LedgerStore::open(&path).unwrap();
    let before = store.connection.total_changes();
    let progress = store.stage_source_union_batch(1, 1, 1).unwrap();
    assert_eq!(
        (progress.scanned_records, progress.recomputed_groups),
        (0, 0)
    );
    assert!(progress.projection_ready);
    assert_eq!(store.connection.total_changes(), before);
    assert_eq!(selected_usage(&store), expected);
}

#[test]
fn late_peer_conflict_rekey_and_missing_assignment_recompute_only_affected_groups() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let source = keyed("source", "shared", 1);
    store.upsert_event(&source).unwrap();
    store
        .upsert_event(&keyed("unrelated", "untouched", 2))
        .unwrap();
    drain(&mut store);
    // Unrelated rows cannot be rewritten while fixing one group.
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER preserve_unrelated BEFORE DELETE ON measurement_union_selected
        WHEN OLD.identity='untouched' BEGIN SELECT RAISE(ABORT,'unrelated group rewritten'); END;",
        )
        .unwrap();
    let mut peer = source.clone();
    peer.event_id = "peer".into();
    peer.account_fingerprint = Some("different-account".into());
    peer.source_timestamp = Some(source.observed_at + ChronoDuration::milliseconds(100));
    reconstruct(&store, peer);
    assert!(
        !store
            .source_union_projection_progress()
            .unwrap()
            .projection_ready
    );
    let progress = drain(&mut store);
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (1, 1)
    );
    let reason: String = store
        .connection
        .query_row(
            "SELECT unresolved_reason FROM measurement_union_groups WHERE identity='shared'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reason, "conflicting_dimensions");
    // Synthetic assignment repair tests dependency invalidation, not authorization
    // to relabel real historical accounts.
    store.connection.execute("UPDATE retained_request_assignments SET account_fingerprint='different-account' WHERE event_id='source'",[]).unwrap();
    assert_eq!(
        store
            .source_union_projection_progress()
            .unwrap()
            .pending_groups,
        1
    );
    assert_eq!(drain(&mut store).selected_groups, 2);
    // Removing the key leaves an unresolved singleton, and restores the peer.
    store.connection.execute("DELETE FROM source_record_evidence WHERE evidence_source='sampling' AND event_id='source'",[]).unwrap();
    let progress = drain(&mut store);
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (2, 1)
    );
    store
        .connection
        .execute(
            "INSERT INTO source_record_evidence VALUES('sampling','source','shared')",
            [],
        )
        .unwrap();
    assert_eq!(drain(&mut store).unresolved_groups, 0);
    store
        .connection
        .execute(
            "DELETE FROM retained_request_assignments WHERE event_id='source'",
            [],
        )
        .unwrap();
    assert_eq!(drain(&mut store).unresolved_groups, 1);
    let reason: String = store
        .connection
        .query_row(
            "SELECT unresolved_reason FROM measurement_union_groups WHERE identity='shared'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reason, "missing_assignment");
    check_counts(&store);
}

#[test]
fn bounded_upgrade_resume_late_insert_and_failure_are_atomic() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schema37.sqlite3");
    {
        let mut connection = Connection::open(&path).unwrap();
        migrations::create_legacy_schema(&mut connection, 37).unwrap();
        let mut old = LedgerStore {
            connection,
            exact_series_memo: Default::default(),
        };
        for (i, id) in ["b", "d", "f"].iter().enumerate() {
            old.upsert_event(&keyed(id, id, i as u64)).unwrap();
        }
    }
    let mut store = LedgerStore::open(&path).unwrap();
    let initial = store.source_union_projection_progress().unwrap();
    assert!(!initial.backfill_complete);
    assert_eq!(
        (initial.pending_groups, initial.selected_groups),
        (0, 0),
        "migration doesn't scan old facts"
    );
    let first = store.stage_source_union_batch(1, 1, 10).unwrap();
    assert_eq!((first.scanned_records, first.selected_groups), (1, 1));
    drop(store);
    let mut store = LedgerStore::open(&path).unwrap();
    store.upsert_event(&keyed("a", "a", 4)).unwrap(); // below the cursor
    store.upsert_event(&keyed("z", "z", 5)).unwrap(); // beyond initial target
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_stage BEFORE INSERT ON measurement_union_selected
        BEGIN SELECT RAISE(ABORT,'synthetic interruption'); END;",
        )
        .unwrap();
    let before = serde_json::to_value(store.source_union_projection_progress().unwrap()).unwrap();
    let cursor: String = store
        .connection
        .query_row(
            "SELECT last_id FROM measurement_union_backfill WHERE evidence_source='sampling'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(store.stage_source_union_batch(1, 20, 10).is_err());
    assert_eq!(
        before,
        serde_json::to_value(store.source_union_projection_progress().unwrap()).unwrap()
    );
    assert_eq!(
        cursor,
        store
            .connection
            .query_row(
                "SELECT last_id FROM measurement_union_backfill WHERE evidence_source='sampling'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap()
    );
    check_counts(&store);
    store
        .connection
        .execute_batch("DROP TRIGGER reject_stage")
        .unwrap();
    assert_eq!(drain(&mut store).selected_groups, 5);
    assert_eq!(selected_usage(&store).total_tokens, 600);
    check_counts(&store);
}

#[test]
fn missing_identity_thread_fanout_zero_and_removal_are_not_fabricated_measurements() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut zero = keyed("zero", "zero", 0);
    zero.usage = TokenUsage::default();
    store.upsert_event(&zero).unwrap();
    store
        .upsert_event(&event("legacy", DataQuality::Confirmed, 1))
        .unwrap();
    let mut no_thread = keyed("no-thread", "no-thread", 2);
    no_thread.thread_id = None;
    store.upsert_event(&no_thread).unwrap();
    let progress = drain(&mut store);
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (1, 2)
    );
    assert_eq!(
        selected_usage(&store).total_tokens,
        0,
        "this is a recorded zero, not an unresolved amount"
    );
    let mut other = zero.clone();
    other.event_id = "duplicate-side".into();
    other.provenance.byte_offset = 3;
    store.upsert_event(&other).unwrap();
    let before = serde_json::to_value(store.source_union_projection_progress().unwrap()).unwrap();
    assert!(matches!(
        store.stage_source_union_batch(10, 20, 1),
        Err(StoreError::UnionLimit)
    ));
    assert_eq!(
        before,
        serde_json::to_value(store.source_union_projection_progress().unwrap()).unwrap()
    );
    let progress = drain(&mut store);
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (0, 3)
    );
    store
        .connection
        .execute(
            "DELETE FROM retained_request_evidence WHERE event_id IN ('zero','duplicate-side')",
            [],
        )
        .unwrap();
    let progress = drain(&mut store);
    assert_eq!(
        (progress.selected_groups, progress.unresolved_groups),
        (0, 2)
    );
    check_counts(&store);
}

#[test]
fn member_budget_is_per_batch_and_late_pair_moves_one_canonical_day() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut source = keyed("sampling-boundary", "boundary", 1);
    source.source_timestamp = Some("2026-01-31T23:59:59.950Z".parse().unwrap());
    let mut peer = source.clone();
    peer.event_id = "reconstruction-boundary".into();
    peer.source_timestamp = Some("2026-02-01T00:00:00.050Z".parse().unwrap());
    reconstruct(&store, peer);
    drain(&mut store);
    let day = |store: &LedgerStore| {
        store.connection.query_row(
        "SELECT date(effective_at) FROM measurement_union_selected WHERE identity='boundary'", [],
        |row| row.get::<_,String>(0)).unwrap()
    };
    assert_eq!(day(&store), "2026-02-01");
    store.upsert_event(&source).unwrap();
    for index in 0..4 {
        store
            .upsert_event(&keyed(
                &format!("sample-{index}"),
                &format!("later-{index}"),
                index + 2,
            ))
            .unwrap();
    }
    let progress = store.stage_source_union_batch(20, 20, 2).unwrap();
    assert_eq!(
        progress.recomputed_groups, 1,
        "the pair consumes both observation slots"
    );
    assert!(!progress.projection_ready);
    assert_eq!(day(&store), "2026-01-31");
    let progress = drain(&mut store);
    assert_eq!(progress.selected_groups, 5);
    assert_eq!(selected_usage(&store).total_tokens, 600);
    let old_day: i64 = store
        .connection
        .query_row(
            "SELECT COUNT(*) FROM measurement_union_selected WHERE date(effective_at)='2026-02-01'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        old_day, 0,
        "a late counterpart must not leave a copy in the old month"
    );
    check_counts(&store);
}
