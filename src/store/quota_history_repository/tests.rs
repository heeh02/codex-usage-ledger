use super::*;
use crate::quota::normalize_rate_limit_event;

fn base() -> DateTime<Utc> {
    DateTime::from_timestamp(1767225600, 0).unwrap()
}
fn snapshot(reset_seconds: i64, used: i64) -> QuotaSnapshot {
    normalize_rate_limit_event(&serde_json::json!({"limit_id":"pool-a","primary":{
        "used_percent":used,"window_minutes":10080,"resets_at":base().timestamp()+reset_seconds
    }}))
    .unwrap()
}
fn add(store: &mut LedgerStore, millis: i64, reset: i64, used: i64) {
    store
        .append_quota_snapshot(
            "account-a",
            "epoch",
            base() + ChronoDuration::milliseconds(millis),
            &snapshot(reset, used),
        )
        .unwrap();
}

#[test]
fn quota_history_pages_stay_frozen_after_late_append_and_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite3");
    let mut store = LedgerStore::open(&path).unwrap();
    for index in 0..6 {
        add(&mut store, index * 1000, (index / 2 + 1) * 100, 20);
    }
    let first = store.quota_history_page("account-a", None, 1).unwrap();
    assert!(first.index_ready);
    assert_eq!(
        first.intervals[0].first_observed_at,
        timestamp(base() + ChronoDuration::seconds(4))
    );
    let cursor = first.next.unwrap();
    let revision = cursor.view.revision;
    add(&mut store, 3500, 300, 20); // Moves the newest boundary earlier, removing its old current version.
    let fresh = store.quota_history_page("account-a", None, 20).unwrap();
    assert!(fresh.view.revision > revision);
    assert_eq!(
        fresh.intervals[0].first_observed_at,
        timestamp(base() + ChronoDuration::milliseconds(3500))
    );
    assert_eq!(fresh.intervals[0].sample_count, 3);
    let current_revision = fresh.view.revision;
    add(&mut store, 3500, 300, 20);
    assert_eq!(
        store
            .quota_history_page("account-a", None, 20)
            .unwrap()
            .view
            .revision,
        current_revision
    );
    drop(store);
    let store = LedgerStore::open(&path).unwrap();
    let cursor: QuotaHistoryCursor =
        serde_json::from_str(&serde_json::to_string(&cursor).unwrap()).unwrap();
    let remaining = store
        .quota_history_page("account-a", Some(&cursor), 20)
        .unwrap();
    assert_eq!(remaining.view.revision, revision);
    assert_eq!(remaining.intervals.len(), 2);
    assert_eq!(
        remaining
            .intervals
            .iter()
            .map(|row| row.sample_count)
            .collect::<Vec<_>>(),
        vec![2, 2]
    );
    assert!(remaining.next.is_none());
    assert!(
        store
            .quota_history_page("other-account", Some(&cursor), 20)
            .is_err()
    );
    let mut tampered = cursor.clone();
    tampered.view.revision += 1;
    assert!(
        store
            .quota_history_page("account-a", Some(&tampered), 20)
            .is_err()
    );
    let other = LedgerStore::open_in_memory().unwrap();
    assert!(
        other
            .quota_history_page("account-a", Some(&cursor), 20)
            .is_err()
    );
}

#[test]
fn quota_history_traverses_more_than_one_thousand_snapshots_without_preview_cap() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    for index in 0..1105 {
        add(
            &mut store,
            index * 1000,
            7200,
            if index % 2 == 0 { 20 } else { 10 },
        );
    }
    let mut cursor = None;
    let mut ids = std::collections::BTreeSet::new();
    let mut count = 0;
    loop {
        let page = store
            .quota_history_page("account-a", cursor.as_ref(), 17)
            .unwrap();
        assert!(page.index_ready);
        assert!(!page.source_history_complete);
        assert!(page.intervals.len() <= 17);
        for row in page.intervals {
            assert!(ids.insert(row.id));
            count += row.sample_count;
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 553);
    assert_eq!(count, 1105);
    assert!(store.quota_history_page("account-a", None, 0).is_err());
}

#[test]
fn quota_history_upgrade_backfill_is_atomic_and_does_not_rewrite_existing_windows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schema36.sqlite3");
    let mut connection = Connection::open(&path).unwrap();
    migrations::create_legacy_schema(&mut connection, 36).unwrap();
    let value = snapshot(7200, 20);
    let json = serde_json::to_string(&value).unwrap();
    let pool = &value.pools[0];
    let window = &pool.windows[0];
    connection.execute("INSERT INTO quota_snapshots VALUES('legacy','account-a','epoch',?1,'token_count_event',?2)",params![timestamp(base()),json]).unwrap();
    connection.execute("INSERT INTO quota_window_observations(observation_id,snapshot_id,window_ordinal,account_fingerprint,auth_epoch,observed_at,stream_key,pool_key,limit_id,role,used_percent,window_seconds,resets_at_unix)
        VALUES('legacy:0','legacy',0,'account-a','epoch',?1,?2,?3,'pool-a','primary',20,'604800',?4)",
        params![timestamp(base()),crate::quota::window_stream_key(pool,window),pool.pool_key,base().timestamp()+7200]).unwrap();
    drop(connection);
    let mut store = LedgerStore::open(&path).unwrap();
    assert!(
        !store
            .quota_history_page("account-a", None, 20)
            .unwrap()
            .index_ready
    );
    store.connection.execute_batch("CREATE TRIGGER refuse_boundary BEFORE INSERT ON quota_boundary_versions BEGIN SELECT RAISE(ABORT,'synthetic'); END;").unwrap();
    assert!(store.backfill_quota_history_chunk(1).is_err());
    assert_eq!(
        store
            .connection
            .query_row("SELECT revision FROM quota_boundary_state", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    store
        .connection
        .execute_batch("DROP TRIGGER refuse_boundary")
        .unwrap();
    assert!(store.backfill_quota_history_chunk(1).unwrap());
    let changes = store.connection.total_changes();
    assert!(store.backfill_quota_history_chunk(1).unwrap());
    assert_eq!(store.connection.total_changes(), changes);
    let page = store.quota_history_page("account-a", None, 20).unwrap();
    assert_eq!(page.intervals.len(), 1);
    assert_eq!(page.intervals[0].sample_count, 1);
    assert_eq!(
        store
            .connection
            .query_row("SELECT normalized_json FROM quota_snapshots", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        json
    );
}

#[test]
fn quota_history_failed_live_boundary_update_rolls_back_source_and_revision() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    add(&mut store, 0, 100, 20);
    let revision = store
        .quota_history_page("account-a", None, 20)
        .unwrap()
        .view
        .revision;
    store.connection.execute_batch("CREATE TRIGGER refuse_boundary BEFORE INSERT ON quota_boundary_versions BEGIN SELECT RAISE(ABORT,'synthetic'); END;").unwrap();
    assert!(
        store
            .append_quota_snapshot(
                "account-a",
                "epoch",
                base() + ChronoDuration::seconds(1),
                &snapshot(200, 20)
            )
            .is_err()
    );
    let page = store.quota_history_page("account-a", None, 20).unwrap();
    assert_eq!(page.view.revision, revision);
    assert_eq!(page.intervals.len(), 1);
    assert_eq!(page.intervals[0].sample_count, 1);
    assert_eq!(
        store.list_quota_snapshots("account-a", 1000).unwrap().len(),
        1
    );
}

#[test]
fn quota_history_all_account_pages_preserve_stream_ownership() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    add(&mut store, 0, 100, 20);
    add(&mut store, 1000, 200, 20);
    store
        .append_quota_snapshot(
            "account-b",
            "epoch",
            base() + ChronoDuration::milliseconds(500),
            &snapshot(100, 30),
        )
        .unwrap();
    let first = store.quota_history_page("all", None, 1).unwrap();
    assert_eq!(first.intervals[0].account_id, "account-a");
    let second = store
        .quota_history_page("all", first.next.as_ref(), 1)
        .unwrap();
    assert_eq!(second.intervals[0].account_id, "account-b");
    let third = store
        .quota_history_page("all", second.next.as_ref(), 1)
        .unwrap();
    assert_eq!(third.intervals[0].account_id, "account-a");
    assert!(third.next.is_none());
    assert_eq!(first.view.revision, third.view.revision);
    assert_eq!(
        store
            .quota_history_page("account-a", None, 20)
            .unwrap()
            .intervals
            .len(),
        2
    );
    assert_eq!(
        store
            .quota_history_page("account-b", None, 20)
            .unwrap()
            .intervals
            .len(),
        1
    );
}
