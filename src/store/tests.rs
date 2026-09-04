use chrono::TimeZone;
use serde_json::json;
use tempfile::tempdir;

use super::*;
use crate::identity::{AuthIdentity, ClaimSource};
use crate::quota::normalize_rate_limit_event;

fn event(id: &str, quality: DataQuality, offset: u64) -> UsageEvent {
    UsageEvent {
        event_id: id.to_owned(),
        observed_at: Utc.with_ymd_and_hms(2026, 8, 31, 1, 0, 0).unwrap(),
        source_timestamp: None,
        thread_id: Some("thread".to_owned()),
        parent_thread_id: None,
        model: Some("gpt-5.6-sol".to_owned()),
        cwd: Some("/work/demo".to_owned()),
        account_fingerprint: Some("acct-fp".to_owned()),
        account_confidence: AttributionConfidence::Verified,
        project: ProjectAttribution {
            project_id: Some("project-1".to_owned()),
            project_name: Some("Demo".to_owned()),
            confidence: AttributionConfidence::Verified,
            method: "native_project_id".to_owned(),
        },
        usage: TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 40,
            cache_write_input_tokens: 10,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        },
        quality,
        quality_reason: None,
        provenance: EventProvenance {
            machine_id: "machine".to_owned(),
            source_id: "rollout-path".to_owned(),
            rollout_id: "rollout".to_owned(),
            file_identity: "inode-1".to_owned(),
            byte_offset: offset,
            line_number: offset / 10,
        },
    }
}

fn cursor(offset: u64) -> FileCursor {
    FileCursor {
        machine_id: "machine".to_owned(),
        source_id: "rollout-path".to_owned(),
        file_identity: "inode-1".to_owned(),
        byte_offset: offset,
        line_number: offset / 10,
        parser_state_json: Some(r#"{"model":"gpt-5.6-sol"}"#.to_owned()),
        updated_at: Utc.with_ymd_and_hms(2026, 8, 31, 1, 1, 0).unwrap(),
    }
}

#[test]
fn opens_wal_database_and_runs_migrations() {
    let directory = tempdir().unwrap();
    let store = LedgerStore::open(directory.path().join("ledger.sqlite")).unwrap();
    assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    let mode: String = store
        .connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode.to_ascii_lowercase(), "wal");
}

#[test]
fn user_confirmed_account_count_is_persistent_and_clearable() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    assert_eq!(store.user_confirmed_account_count().unwrap(), None);
    store.set_user_confirmed_account_count(Some(4)).unwrap();
    assert_eq!(store.user_confirmed_account_count().unwrap(), Some(4));
    store.set_user_confirmed_account_count(None).unwrap();
    assert_eq!(store.user_confirmed_account_count().unwrap(), None);
}

#[test]
fn typed_table_counts_hide_schema_queries_from_cli_callers() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    assert_eq!(
        store.ledger_table_counts().unwrap(),
        LedgerTableCounts::default()
    );
    store
        .upsert_event(&event("counted", DataQuality::Confirmed, 10))
        .unwrap();
    assert_eq!(
        store.ledger_table_counts().unwrap(),
        LedgerTableCounts {
            raw_events: 1,
            compacted_event_keys: 0,
            file_cursors: 0,
        }
    );
}

#[test]
fn confirmed_usage_is_guarded_in_rust_and_sqlite() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut invalid = event("invalid", DataQuality::Confirmed, 10);
    invalid.usage.reasoning_output_tokens = invalid.usage.output_tokens + 1;
    assert!(matches!(
        store.upsert_event(&invalid),
        Err(StoreError::InvalidConfirmedUsage(
            crate::types::TokenUsageInvariantError::ReasoningExceedsOutput
        ))
    ));

    store
        .upsert_event(&event("valid", DataQuality::Confirmed, 20))
        .unwrap();
    let trigger_error = store.connection().execute(
        "UPDATE usage_events SET total_tokens = total_tokens + 1 WHERE event_id = 'valid'",
        [],
    );
    assert!(
        trigger_error.is_err(),
        "SQLite trigger must reject invalid confirmed usage"
    );
    let rollup_trigger_error = store.connection().execute(
        "UPDATE daily_usage_rollups SET total_tokens = total_tokens + 1
         WHERE quality = 'confirmed'",
        [],
    );
    assert!(
        rollup_trigger_error.is_err(),
        "SQLite trigger must reject invalid confirmed rollups"
    );

    invalid.quality = DataQuality::Quarantined;
    store.upsert_event(&invalid).unwrap();
}

#[test]
fn old_confirmed_events_cannot_bypass_validation_during_direct_compaction() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut invalid = event("old-invalid", DataQuality::Confirmed, 10);
    invalid.observed_at = Utc::now() - ChronoDuration::days(RAW_EVENT_RETENTION_DAYS + 1);
    invalid.source_timestamp = Some(invalid.observed_at);
    invalid.usage.total_tokens += 1;

    assert!(matches!(
        store.upsert_events_and_cursor(&[invalid], &cursor(10)),
        Err(StoreError::InvalidConfirmedUsage(
            crate::types::TokenUsageInvariantError::TotalDoesNotConserve
        ))
    ));
    assert!(
        store
            .get_cursor("machine", "rollout-path")
            .unwrap()
            .is_none(),
        "failed compact writes must not advance their source cursor"
    );
    assert_eq!(
        store.ledger_table_counts().unwrap(),
        LedgerTableCounts::default()
    );
}

#[test]
fn schema_24_refuses_preexisting_invalid_confirmed_rollups() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("invalid-rollup.sqlite");
    {
        let store = LedgerStore::open(&path).unwrap();
        store
            .connection()
            .execute_batch(
                "DROP TRIGGER daily_usage_rollups_confirmed_usage_insert_guard;
                 DROP TRIGGER daily_usage_rollups_confirmed_usage_update_guard;
                 PRAGMA user_version = 23;
                 INSERT INTO daily_usage_rollups(
                     local_day, thread_key, account_key, project_key, model_key, quality,
                     event_count, input_tokens, cached_input_tokens,
                     cache_write_input_tokens, cache_write_observed_input_tokens,
                     output_tokens, reasoning_output_tokens, total_tokens
                 ) VALUES (
                     '2026-09-04', 'thread', 'account', 'project', 'model', 'confirmed',
                     1, 100, 40, 10, 100, 20, 21, 120
                 );",
            )
            .unwrap();
    }

    assert!(matches!(
        LedgerStore::open(&path),
        Err(StoreError::PersistedUsageInvariantViolation {
            table: "daily_usage_rollups",
            rows: 1,
        })
    ));
}

#[test]
fn schema_24_repairs_legacy_reconstruction_coverage_without_changing_tokens() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("legacy-coverage.sqlite");
    {
        let store = LedgerStore::open(&path).unwrap();
        store
            .connection()
            .execute_batch(
                "DROP TRIGGER reconstruction_usage_insert_guard;
                 DROP TRIGGER reconstruction_usage_update_guard;
                 DROP TRIGGER daily_usage_rollups_confirmed_usage_insert_guard;
                 DROP TRIGGER daily_usage_rollups_confirmed_usage_update_guard;
                 PRAGMA user_version = 23;
                 INSERT INTO reconstruction_usage_events(
                     event_id, event_hash, observed_at, source_timestamp, thread_id,
                     parent_thread_id, model, cwd, account_fingerprint, account_confidence,
                     project_id, project_name, project_confidence, project_method,
                     input_tokens, cached_input_tokens, cache_write_input_tokens,
                     cache_write_observed_input_tokens, output_tokens,
                     reasoning_output_tokens, total_tokens, machine_id, source_id,
                     file_identity, byte_offset, line_number, counter_epoch
                 ) VALUES (
                     'legacy', 'hash', '2026-09-04T00:00:00Z',
                     '2026-09-04T00:00:00Z', 'thread', NULL, 'model', NULL,
                     'account', 'verified', 'project', 'Project', 'verified', 'test',
                     100, 40, 10, 1000, 20, 5, 120, 'machine', 'source',
                     'file', 10, 1, 0
                 );",
            )
            .unwrap();
    }

    let store = LedgerStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), 24);
    let repaired: (i64, i64, i64) = store
        .connection()
        .query_row(
            "SELECT input_tokens, cache_write_observed_input_tokens, total_tokens
             FROM reconstruction_usage_events WHERE event_id = 'legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(repaired, (100, 100, 120));
    let receipt: i64 = store
        .connection()
        .query_row(
            "SELECT row_count FROM migration_repairs
             WHERE repair_id = 'schema24-reconstruction-cache-write-coverage'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(receipt, 1);
    let rollup: (i64, i64) = store
        .connection()
        .query_row(
            "SELECT cache_write_observed_input_tokens, total_tokens
             FROM reconstruction_daily_rollups WHERE thread_key = 'thread'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(rollup, (100, 120));
}

#[test]
fn upgrades_existing_schema_with_replay_cursor_and_effective_time_indexes() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("upgrade.sqlite");
    {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(MIGRATION_1).unwrap();
        connection.execute_batch(MIGRATION_2).unwrap();
        connection.execute_batch(MIGRATION_3).unwrap();
        connection.execute_batch(MIGRATION_4).unwrap();
        connection.pragma_update(None, "user_version", 4).unwrap();
    }
    let mut store = LedgerStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    store.advance_cursor(&cursor(10)).unwrap();
    assert_eq!(
        store
            .get_cursor("machine", "rollout-path")
            .unwrap()
            .unwrap()
            .parser_state_json,
        cursor(10).parser_state_json
    );
}

#[test]
fn upgrades_v15_to_cache_write_buckets_without_changing_old_totals() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("upgrade-v15.sqlite");
    {
        let connection = Connection::open(&path).unwrap();
        for migration in [
            MIGRATION_1,
            MIGRATION_2,
            MIGRATION_3,
            MIGRATION_4,
            MIGRATION_5,
            MIGRATION_6,
            MIGRATION_7,
            MIGRATION_8,
            MIGRATION_9,
            MIGRATION_10,
            MIGRATION_11,
            MIGRATION_12,
            MIGRATION_13,
            MIGRATION_14,
            MIGRATION_15,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO daily_usage_rollups(
                         local_day, thread_key, account_key, project_key, model_key, quality,
                         event_count, input_tokens, cached_input_tokens, output_tokens,
                         reasoning_output_tokens, total_tokens
                     ) VALUES ('2026-08-31', 'old-thread', 'acct-fp', 'project-1',
                               'gpt-5.6-sol', 'confirmed', 1, 100, 40, 20, 5, 120)",
                [],
            )
            .unwrap();
        connection.pragma_update(None, "user_version", 15).unwrap();
    }

    let mut store = LedgerStore::open(&path).unwrap();
    let migrated = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    assert_eq!(migrated.usage.total_tokens, 120);
    assert_eq!(migrated.usage.cache_write_input_tokens, 0);
    assert_eq!(migrated.usage.cache_write_observed_input_tokens, 0);

    store
        .upsert_event(&event("new-cache-schema", DataQuality::Confirmed, 30))
        .unwrap();
    let after = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    assert_eq!(after.usage.total_tokens, 240);
    assert_eq!(after.usage.cache_write_input_tokens, 10);
    assert_eq!(after.usage.cache_write_observed_input_tokens, 100);
    assert_eq!(after.usage.uncached_input_tokens(), 110);
}

#[test]
fn event_upsert_is_replay_safe_and_cursor_is_transactional() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let original = event("event-1", DataQuality::Confirmed, 10);
    let result = store
        .upsert_events_and_cursor(std::slice::from_ref(&original), &cursor(20))
        .unwrap();
    assert_eq!(result.inserted, 1);
    assert_eq!(
        store.upsert_event(&original).unwrap(),
        UpsertOutcome::Unchanged
    );

    let mut corrected = original.clone();
    corrected.model = Some("gpt-5.6-terra".to_owned());
    assert_eq!(
        store.upsert_event(&corrected).unwrap(),
        UpsertOutcome::Updated
    );
    assert_eq!(store.get_event("event-1").unwrap().unwrap(), corrected);
    assert_eq!(
        store.get_cursor("machine", "rollout-path").unwrap(),
        Some(cursor(20))
    );
}

#[test]
fn cursor_regression_is_rejected_but_rotation_can_reset() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    store.advance_cursor(&cursor(100)).unwrap();
    let error = store.advance_cursor(&cursor(90)).unwrap_err();
    assert!(matches!(error, StoreError::CursorRegression { .. }));

    let mut rotated = cursor(0);
    rotated.file_identity = "inode-2".to_owned();
    store.advance_cursor(&rotated).unwrap();
    assert_eq!(
        store.get_cursor("machine", "rollout-path").unwrap(),
        Some(rotated)
    );
}

#[test]
fn explicit_cursor_reset_allows_verified_same_inode_truncate() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    store.advance_cursor(&cursor(100)).unwrap();
    let reset = cursor(0);
    store.reset_cursor(&reset).unwrap();
    assert_eq!(
        store.get_cursor("machine", "rollout-path").unwrap(),
        Some(reset)
    );
}

#[test]
fn cursor_can_be_recovered_after_source_path_moves_to_archive() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let original = cursor(100);
    store.advance_cursor(&original).unwrap();
    let recovered = store
        .get_cursor_by_file_identity("machine", "inode-1")
        .unwrap()
        .unwrap();
    assert_eq!(recovered.source_id, "rollout-path");
    assert_eq!(recovered.byte_offset, 100);
    assert_eq!(recovered.parser_state_json, original.parser_state_json);
}

#[test]
fn aggregates_default_to_confirmed_and_support_dimensions_and_filters() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    store
        .upsert_event(&event("confirmed", DataQuality::Confirmed, 10))
        .unwrap();
    store
        .upsert_event(&event("quarantined", DataQuality::Quarantined, 20))
        .unwrap();

    let trusted = store.aggregate_usage(&AggregateFilter::default()).unwrap();
    assert_eq!(trusted.event_count, 1);
    assert_eq!(trusted.usage.total_tokens, 120);

    let all = store
        .aggregate_usage(&AggregateFilter {
            quality: None,
            ..AggregateFilter::default()
        })
        .unwrap();
    assert_eq!(all.event_count, 2);
    assert_eq!(all.usage.total_tokens, 240);

    let buckets = store
        .aggregate_by(
            AggregateDimension::Quality,
            &AggregateFilter {
                quality: None,
                ..AggregateFilter::default()
            },
        )
        .unwrap();
    assert_eq!(buckets.len(), 2);
}

#[test]
fn verified_rollup_survives_compaction_and_old_event_replay() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut first = event("old-1", DataQuality::Confirmed, 10);
    first.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 1, 1, 0, 0).unwrap());
    let mut second = event("old-2", DataQuality::Quarantined, 20);
    second.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 2, 1, 0, 0).unwrap());
    second.model = Some("gpt-5.6-luna".to_owned());
    second.project.project_id = Some("project-2".to_owned());
    store.upsert_event(&first).unwrap();
    store.upsert_event(&second).unwrap();

    // Recreate the state of an upgrade from schema 8: raw facts exist but
    // the new rollup is initially empty and must be backfilled in chunks.
    store
        .connection()
        .execute_batch(
            "DELETE FROM daily_usage_rollups;
                 UPDATE rollup_state SET
                    last_backfilled_rowid = 0,
                    target_rowid = (SELECT COALESCE(MAX(rowid), 0) FROM usage_events),
                    complete = 0,
                    verified_at = NULL;",
        )
        .unwrap();
    while !store.backfill_rollup_chunk(1).unwrap().complete {}

    let all = AggregateFilter {
        quality: None,
        ..AggregateFilter::default()
    };
    let raw = store.aggregate_usage(&all).unwrap();
    let rolled_up = store.aggregate_rollup_usage(&all).unwrap();
    assert_eq!(raw, rolled_up);
    assert_eq!(raw.event_count, 2);
    assert_eq!(
        store
            .aggregate_rollup_by(AggregateDimension::Model, &all)
            .unwrap()
            .len(),
        2
    );

    store.verify_rollup_before_compaction().unwrap();
    let before_compaction = store.aggregate_rollup_usage(&all).unwrap();
    let cutoff = Utc.with_ymd_and_hms(2026, 8, 20, 0, 0, 0).unwrap();
    assert_eq!(store.compact_raw_events_chunk(cutoff, 100).unwrap(), 2);
    assert_eq!(store.aggregate_usage(&all).unwrap().event_count, 0);
    assert_eq!(
        store.aggregate_rollup_usage(&all).unwrap(),
        before_compaction
    );

    let replay = store
        .upsert_events_and_cursor(&[first.clone(), second.clone()], &cursor(30))
        .unwrap();
    assert_eq!(replay.inserted, 0);
    assert_eq!(replay.unchanged, 2);
    assert_eq!(
        store.aggregate_rollup_usage(&all).unwrap(),
        before_compaction
    );

    let mut conflicting = first;
    conflicting.usage.total_tokens += 1;
    let error = store
        .upsert_events_and_cursor(&[conflicting], &cursor(40))
        .unwrap_err();
    assert!(matches!(error, StoreError::CompactedEventConflict { .. }));
    assert_eq!(
        store
            .get_cursor("machine", "rollout-path")
            .unwrap()
            .unwrap()
            .byte_offset,
        30
    );
}

#[test]
fn historical_account_reassignment_preserves_compacted_totals() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut old = event("historical-unknown", DataQuality::Confirmed, 10);
    old.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 1, 1, 0, 0).unwrap());
    old.account_fingerprint = None;
    old.account_confidence = AttributionConfidence::Unknown;
    store.upsert_event(&old).unwrap();
    store.verify_rollup_before_compaction().unwrap();
    assert_eq!(
        store
            .compact_raw_events_chunk(Utc.with_ymd_and_hms(2026, 8, 10, 0, 0, 0).unwrap(), 10,)
            .unwrap(),
        1
    );
    let before = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    store
        .replace_historical_auth_epochs(
            "machine",
            "logs2-auth-history-v1",
            &[HistoricalAuthEpochInput {
                observed_from: Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
                observed_to: Some(Utc.with_ymd_and_hms(2026, 8, 10, 0, 0, 0).unwrap()),
                account_fingerprint: "historical-account".to_owned(),
                workspace_fingerprint: "historical-workspace".to_owned(),
                confidence: AttributionConfidence::Inferred,
            }],
        )
        .unwrap();
    let after = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    assert_eq!(before, after);
    let attributed = store
        .aggregate_rollup_usage(&AggregateFilter {
            account_fingerprint: Some("historical-account".to_owned()),
            ..AggregateFilter::default()
        })
        .unwrap();
    assert_eq!(attributed, before);
}

#[test]
fn aggregate_time_uses_source_timestamp_before_ingest_time() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut backfilled = event("backfilled", DataQuality::Confirmed, 10);
    backfilled.observed_at = Utc.with_ymd_and_hms(2026, 8, 31, 12, 0, 0).unwrap();
    backfilled.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap());
    store.upsert_event(&backfilled).unwrap();

    let august_first = store
        .aggregate_usage(&AggregateFilter {
            start_inclusive: Some(Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap()),
            end_exclusive: Some(Utc.with_ymd_and_hms(2026, 8, 2, 0, 0, 0).unwrap()),
            ..AggregateFilter::default()
        })
        .unwrap();
    assert_eq!(august_first.event_count, 1);
    let buckets = store
        .aggregate_by(AggregateDimension::Day, &AggregateFilter::default())
        .unwrap();
    assert_eq!(buckets[0].key.as_deref(), Some("2026-08-01"));
}

#[test]
fn day_buckets_respect_iana_timezone_boundaries() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut before_midnight = event("before-midnight", DataQuality::Confirmed, 10);
    before_midnight.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 1, 15, 30, 0).unwrap());
    let mut after_midnight = event("after-midnight", DataQuality::Confirmed, 20);
    after_midnight.source_timestamp = Some(Utc.with_ymd_and_hms(2026, 8, 1, 16, 30, 0).unwrap());
    store.upsert_event(&before_midnight).unwrap();
    store.upsert_event(&after_midnight).unwrap();
    let buckets = store
        .aggregate_by_day(&AggregateFilter::default(), "Asia/Shanghai")
        .unwrap();
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].key.as_deref(), Some("2026-08-01"));
    assert_eq!(buckets[1].key.as_deref(), Some("2026-08-02"));
    assert!(matches!(
        store.aggregate_by_day(&AggregateFilter::default(), "Mars/Olympus"),
        Err(StoreError::InvalidTimezone(_))
    ));
}

#[test]
fn thread_catalog_merges_native_titles_with_replay_safe_parentage() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let at = Utc.with_ymd_and_hms(2026, 8, 31, 3, 0, 0).unwrap();
    let mut child = event("child-event", DataQuality::Confirmed, 10);
    child.thread_id = Some("child-thread".to_owned());
    child.parent_thread_id = Some("root-thread".to_owned());
    child.model = Some("gpt-5.6-sol".to_owned());
    store.upsert_event(&child).unwrap();

    store
        .upsert_thread_catalog(&ThreadCatalogRecord {
            thread_id: "child-thread".to_owned(),
            parent_thread_id: None,
            project_id: Some("project".to_owned()),
            project_name: Some("Project".to_owned()),
            title: Some("Native child title".to_owned()),
            model: Some("gpt-5.6-sol".to_owned()),
            agent_nickname: Some("Curie".to_owned()),
            agent_role: Some("explorer".to_owned()),
            agent_path: Some("/root/child".to_owned()),
            depth: Some(1),
            created_at: at,
            updated_at: at,
            archived: false,
            has_user_event: false,
            source_kind: "state_5".to_owned(),
        })
        .unwrap();

    let stored: (Option<String>, Option<String>, Option<String>, Option<i64>) = store
        .connection()
        .query_row(
            "SELECT parent_thread_id, title, agent_nickname, depth
                 FROM thread_catalog WHERE thread_id = 'child-thread'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(stored.0.as_deref(), Some("root-thread"));
    assert_eq!(stored.1.as_deref(), Some("Native child title"));
    assert_eq!(stored.2.as_deref(), Some("Curie"));
    assert_eq!(stored.3, Some(1));
}

#[test]
fn native_catalog_deletion_marks_history_without_deleting_usage() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let at = Utc.with_ymd_and_hms(2026, 8, 31, 3, 0, 0).unwrap();
    let mut usage = event("historical-session-event", DataQuality::Confirmed, 25);
    usage.thread_id = Some("historical-session".to_owned());
    store.upsert_event(&usage).unwrap();
    let before = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    let record = ThreadCatalogRecord {
        thread_id: "historical-session".to_owned(),
        parent_thread_id: None,
        project_id: Some("project".to_owned()),
        project_name: Some("Project".to_owned()),
        title: Some("Preserved historical session".to_owned()),
        model: Some("gpt-5.6-sol".to_owned()),
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        depth: Some(0),
        created_at: at,
        updated_at: at,
        archived: false,
        has_user_event: true,
        source_kind: "state_5".to_owned(),
    };
    store
        .sync_native_thread_catalog_batch(std::slice::from_ref(&record))
        .unwrap();
    let present: bool = store
        .connection()
        .query_row(
            "SELECT present_in_codex FROM thread_catalog WHERE thread_id = ?1",
            ["historical-session"],
            |row| row.get(0),
        )
        .unwrap();
    assert!(present);

    store.sync_native_thread_catalog_batch(&[]).unwrap();
    let (present, title): (bool, String) = store
        .connection()
        .query_row(
            "SELECT present_in_codex, title FROM thread_catalog WHERE thread_id = ?1",
            ["historical-session"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(!present);
    assert_eq!(title, "Preserved historical session");
    let after = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn project_rebinding_changes_only_the_project_dimension() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut usage = event("unassigned", DataQuality::Confirmed, 10);
    usage.project.project_id = None;
    usage.project.project_name = None;
    store.upsert_event(&usage).unwrap();
    let before = store
        .aggregate_rollup_usage(&AggregateFilter::default())
        .unwrap();
    let at = Utc.with_ymd_and_hms(2026, 8, 31, 3, 0, 0).unwrap();
    store
        .upsert_thread_catalog(&ThreadCatalogRecord {
            thread_id: "thread".to_owned(),
            parent_thread_id: None,
            project_id: Some("project-resolved".to_owned()),
            project_name: Some("Resolved".to_owned()),
            title: None,
            model: None,
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            depth: Some(0),
            created_at: at,
            updated_at: at,
            archived: false,
            has_user_event: true,
            source_kind: "state_5".to_owned(),
        })
        .unwrap();
    store.reproject_usage_from_catalog().unwrap();
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap(),
        before
    );
    let projects = store
        .aggregate_rollup_by(AggregateDimension::Project, &AggregateFilter::default())
        .unwrap();
    assert_eq!(projects[0].key.as_deref(), Some("project-resolved"));
    assert_eq!(projects[0].usage.total_tokens, 120);
}

#[test]
fn auth_epochs_are_temporal_and_never_store_raw_identity() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let first = AuthIdentity {
        account_fingerprint: Some("account-fp-1".to_owned()),
        person_fingerprint: Some("person-fp".to_owned()),
        workspace_fingerprint: Some("workspace-fp-1".to_owned()),
        auth_epoch: "generation-1".to_owned(),
        confidence: AttributionConfidence::Verified,
        person_claim_source: ClaimSource::ChatGptUserId,
        workspace_claim_source: ClaimSource::ChatGptAccountId,
        workspace_claim_consistent: true,
        issuer_fingerprint: Some("issuer-fp".to_owned()),
        plan_type: Some("pro".to_owned()),
        access_token_expires_at: None,
    };
    let at_one = Utc.with_ymd_and_hms(2026, 8, 31, 1, 0, 0).unwrap();
    let at_two = Utc.with_ymd_and_hms(2026, 8, 31, 2, 0, 0).unwrap();
    let epoch = store
        .append_auth_epoch("machine", "auth-file", &first, at_one)
        .unwrap();
    let duplicate = store
        .append_auth_epoch("machine", "auth-file", &first, at_two)
        .unwrap();
    assert_eq!(epoch.epoch_id, duplicate.epoch_id);

    let mut second = first.clone();
    second.auth_epoch = "generation-2".to_owned();
    second.account_fingerprint = Some("account-fp-2".to_owned());
    store
        .append_auth_epoch("machine", "auth-file", &second, at_two)
        .unwrap();
    let epochs = store.list_auth_epochs("machine", "auth-file").unwrap();
    assert_eq!(epochs.len(), 2);
    assert_eq!(epochs[0].observed_to, Some(at_two));
    assert_eq!(epochs[1].observed_to, None);
    let at_three = Utc.with_ymd_and_hms(2026, 8, 31, 3, 0, 0).unwrap();
    assert!(
        store
            .close_current_auth_epoch("machine", "auth-file", at_three)
            .unwrap()
    );
    assert_eq!(
        store.list_auth_epochs("machine", "auth-file").unwrap()[1].observed_to,
        Some(at_three)
    );
}

#[test]
fn official_account_usage_is_account_scoped_and_historical_days_are_revisable() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };

    let mut store = LedgerStore::open_in_memory().unwrap();
    let first_at = Utc.with_ymd_and_hms(2026, 8, 31, 1, 0, 0).unwrap();
    let second_at = Utc.with_ymd_and_hms(2026, 8, 31, 2, 0, 0).unwrap();
    let mut usage = OfficialAccountUsage {
        summary: OfficialUsageSummary {
            lifetime_tokens: Some(61_052_184_141),
            peak_daily_tokens: Some(4_124_570_551),
            ..OfficialUsageSummary::default()
        },
        daily_usage_buckets: vec![OfficialDailyUsageBucket {
            start_date: "2026-08-28".to_owned(),
            tokens: 3_773_478_465,
        }],
        thread_usage: None,
    };
    store
        .upsert_official_account_usage("account-a", first_at, &usage)
        .unwrap();
    usage.daily_usage_buckets[0].tokens = 3_800_000_000;
    store
        .upsert_official_account_usage("account-a", second_at, &usage)
        .unwrap();

    assert_eq!(store.list_official_accounts().unwrap(), vec!["account-a"]);
    assert_eq!(
        store.official_daily_usage("account-a", None, None).unwrap()[0].tokens,
        3_800_000_000
    );
    let latest = store
        .latest_official_account_usage("account-a")
        .unwrap()
        .unwrap();
    assert_eq!(latest.observed_at, second_at);
    assert_eq!(latest.usage.summary.lifetime_tokens, Some(61_052_184_141));
    assert!(
        store
            .latest_official_account_usage("account-b")
            .unwrap()
            .is_none()
    );
}

#[test]
fn quota_projects_and_manual_assignments_round_trip() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let observed_at = Utc.with_ymd_and_hms(2026, 8, 31, 3, 0, 0).unwrap();
    let snapshot = normalize_rate_limit_event(&json!({
        "limit_id": "codex_bengalfox",
        "limit_name": "GPT-5.3-Codex-Spark",
        "primary": { "used_percent": 20, "window_minutes": 300 }
    }))
    .unwrap();
    let id = store
        .append_quota_snapshot("account-fp", "generation", observed_at, &snapshot)
        .unwrap();
    let latest = store.latest_quota_snapshot("account-fp").unwrap().unwrap();
    assert_eq!(latest.snapshot_id, id);
    assert_eq!(latest.snapshot, snapshot);
    assert_eq!(
        store.list_quota_snapshots("account-fp", 10).unwrap().len(),
        1
    );

    let project = ProjectRecord {
        project_id: "project".to_owned(),
        project_name: "Project".to_owned(),
        roots: vec!["/work/project".into(), "/work/project/sub".into()],
        git_identities: vec!["https://credential@github.com/owner/project.git".to_owned()],
    };
    store.upsert_project(&project, observed_at).unwrap();
    let stored_projects = store.list_projects().unwrap();
    assert_eq!(stored_projects[0].project_id, project.project_id);
    assert_eq!(
        stored_projects[0].git_identities,
        vec!["github.com/owner/project".to_owned()]
    );

    let assignment = ManualProjectAssignment {
        project_id: "project".to_owned(),
        project_name: None,
    };
    store
        .upsert_manual_assignment("thread:one", &assignment, observed_at)
        .unwrap();
    let stored = store.get_manual_assignment("thread:one").unwrap().unwrap();
    assert_eq!(stored.assignment, assignment);
}
