use super::*;
use std::io::Write;

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join("sessions")).unwrap();
    let path = home.join("sessions/root.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    let amount = |total: u64| TokenUsage {
        input_tokens: total - 10,
        cached_input_tokens: (total - 10) / 2,
        cache_write_input_tokens: (total - 10) / 10,
        cache_write_observed_input_tokens: total - 10,
        output_tokens: 10,
        reasoning_output_tokens: 3,
        total_tokens: total,
    };
    let token = |at: &str, total: u64, last: u64| {
        serde_json::json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{
        "total_token_usage":amount(total),
        "last_token_usage":amount(last)}}})
    };
    let lines = [
        serde_json::json!({"type":"session_meta","timestamp":"2026-01-01T00:00:00Z","payload":{"id":"root"}}),
        token("2026-01-01T00:00:10Z", 100, 100),
        token("2026-01-01T00:00:20Z", 150, 50),
        serde_json::json!({"type":"session_meta","timestamp":"2026-01-01T00:00:30Z","payload":{"id":"foreign"}}),
        token("2026-01-01T00:00:30Z", 777, 777),
    ];
    let mut offsets = Vec::new();
    let mut offset = 0;
    for line in lines {
        let line = format!("{line}\n");
        offsets.push(offset);
        offset += line.len() as u64;
        file.write_all(line.as_bytes()).unwrap();
    }
    drop(file);
    let index = Connection::open(home.join("state_5.sqlite")).unwrap();
    index
        .execute_batch(
            "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
        )
        .unwrap();
    index
        .execute(
            "INSERT INTO threads VALUES('root',?1,NULL,NULL,NULL)",
            [path.to_str().unwrap()],
        )
        .unwrap();
    drop(index);
    let db = dir.path().join("source.sqlite3");
    let mut store = LedgerStore::open(&db).unwrap();
    crate::reconstruction::ingest_reconstruction_batch(&mut store, &home, "synthetic-machine", 1)
        .unwrap();
    let source = store.reconstruction_sources().unwrap().remove(0);
    let mut legacy = crate::store::tests::event("unused", DataQuality::Confirmed, offsets[4]);
    legacy.event_id = crate::reconstruction::stable_event_id(
        &source.machine_id,
        &source.file_identity,
        "root",
        offsets[4],
    );
    legacy.thread_id = Some("root".into());
    legacy.source_timestamp = Some("2026-01-01T00:00:30Z".parse().unwrap());
    legacy.usage = TokenUsage {
        input_tokens: 777,
        total_tokens: 777,
        ..Default::default()
    };
    legacy.provenance.machine_id = source.machine_id;
    legacy.provenance.source_id = source.source_id;
    legacy.provenance.file_identity = source.file_identity;
    legacy.provenance.rollout_id = "root".into();
    let tx = store.connection.unchecked_transaction().unwrap();
    upsert_reconstruction_event_in(
        &tx,
        &ReconstructionEvent {
            event: legacy,
            counter_epoch: 0,
        },
    )
    .unwrap();
    tx.execute("UPDATE reconstruction_usage_events SET input_tokens=990,total_tokens=1000,event_hash=?1 WHERE byte_offset=?2",params!["a".repeat(64),offsets[1] as i64]).unwrap();
    tx.execute("DELETE FROM source_record_evidence", [])
        .unwrap();
    rebuild_reconstruction_rollups_in(&tx).unwrap();
    tx.commit().unwrap();
    drop(store);
    let manifest = dir.path().join("correction.jsonl");
    let checked = crate::reconstruction::write_correction_manifest(
        &db, &home, "root", 10000, 100, false, &manifest,
    )
    .unwrap();
    (dir, db, manifest, checked.body_sha256)
}

#[test]
fn shadow_correction_archives_originals_conserves_and_is_idempotent() {
    let (dir, db, manifest, seal) = fixture();
    let original = std::fs::read(&db).unwrap();
    let shadow = dir.path().join("shadow.sqlite3");
    create_review_shadow(&db, &shadow).unwrap();
    assert!(create_review_shadow(&db, &shadow).is_err());
    assert!(apply_shadow_correction(&db, &manifest, &seal).is_err());
    assert!(apply_shadow_correction(&shadow, &manifest, &"0".repeat(64)).is_err());
    let receipt = apply_shadow_correction(&shadow, &manifest, &seal).unwrap();
    assert_eq!(
        (
            receipt.archived_records,
            receipt.corrected_records,
            receipt.suppressed_records
        ),
        (3, 2, 1)
    );
    assert_eq!(
        apply_shadow_correction(&shadow, &manifest, &seal)
            .unwrap()
            .status,
        "already_applied"
    );
    let mut store = LedgerStore::open(&shadow).unwrap();
    let expected = TokenUsage {
        input_tokens: 140,
        cached_input_tokens: 70,
        cache_write_input_tokens: 14,
        cache_write_observed_input_tokens: 140,
        output_tokens: 10,
        reasoning_output_tokens: 3,
        total_tokens: 150,
    };
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap()
            .usage,
        expected
    );
    for dimension in [
        AggregateDimension::Model,
        AggregateDimension::Account,
        AggregateDimension::Project,
        AggregateDimension::Thread,
        AggregateDimension::Day,
    ] {
        let mut sum = TokenUsage::default();
        for row in store
            .aggregate_rollup_by(dimension, &AggregateFilter::default())
            .unwrap()
        {
            checked_add_usage(&mut sum, row.usage).unwrap();
        }
        assert_eq!(sum, expected);
    }
    assert_eq!(
        store
            .aggregate_rollup_usage(&AggregateFilter::default())
            .unwrap()
            .usage
            .total_tokens,
        150
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT SUM(total_tokens) FROM review_old_reconstruction",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1827
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT COUNT(*) FROM source_record_evidence", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    for _ in 0..100 {
        if store
            .stage_source_union_batch(10, 10, 100)
            .unwrap()
            .projection_ready
        {
            break;
        }
    }
    drop(store);
    let preview = LedgerStore::open_source_union_main_preview(&shadow).unwrap();
    assert_eq!(
        preview
            .aggregate_usage(&AggregateFilter::default())
            .unwrap()
            .usage
            .total_tokens,
        150
    );
    drop(preview);
    assert_eq!(std::fs::read(&db).unwrap(), original);
    let changed = Connection::open(&shadow).unwrap();
    changed
        .execute("UPDATE reconstruction_usage_events SET model='changed'", [])
        .unwrap();
    drop(changed);
    assert!(apply_shadow_correction(&shadow, &manifest, &seal).is_err());
}

#[test]
fn failed_application_rolls_back_archive_receipt_and_all_amounts() {
    let (dir, db, manifest, seal) = fixture();
    let shadow = dir.path().join("shadow.sqlite3");
    create_review_shadow(&db, &shadow).unwrap();
    let store = Connection::open(&shadow).unwrap();
    store.execute_batch("CREATE TRIGGER refuse_correction BEFORE DELETE ON reconstruction_usage_events BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    drop(store);
    assert!(apply_shadow_correction(&shadow, &manifest, &seal).is_err());
    let store = Connection::open(&shadow).unwrap();
    assert_eq!(
        store
            .query_row("SELECT COUNT(*) FROM review_old_reconstruction", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .query_row("SELECT COUNT(*) FROM review_correction_receipts", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .query_row(
                "SELECT SUM(total_tokens) FROM reconstruction_usage_events",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1827
    );
}

#[test]
fn review_copy_preserves_sparse_rowid_based_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite3");
    let store = LedgerStore::open(&source).unwrap();
    store.connection.execute_batch("CREATE TABLE cursor_rowid_fixture(value TEXT); INSERT INTO cursor_rowid_fixture(rowid,value) VALUES(100,'first'),(300,'last');").unwrap();
    drop(store);
    let shadow = dir.path().join("shadow.sqlite3");
    create_review_shadow(&source, &shadow).unwrap();
    let reader = Connection::open(&shadow).unwrap();
    let ids = reader
        .prepare("SELECT rowid FROM cursor_rowid_fixture ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(ids, vec![100, 300]);
}

fn linked_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    let (dir, db, manifest, seal) = fixture();
    let shadow = dir.path().join("shadow.sqlite3");
    create_review_shadow(&db, &shadow).unwrap();
    apply_shadow_correction(&shadow, &manifest, &seal).unwrap();
    let mut store = LedgerStore::open(&shadow).unwrap();
    let rows=store.connection.prepare(&format!("SELECT {EVENT_SELECT_COLUMNS} FROM (SELECT r.*,'confirmed' AS quality,NULL AS quality_reason,thread_id AS rollout_id FROM reconstruction_usage_events r) ORDER BY source_timestamp"))
        .unwrap().query_map([],row_to_event).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    let mut samples = Vec::new();
    for (index, mut event) in rows.into_iter().enumerate() {
        event.event_id = format!("logs2-post-sampling:{}", index + 1);
        event.provenance.source_record_key = None;
        event.provenance.source_id = crate::sampling::POST_SAMPLING_SOURCE_ID.into();
        event.provenance.byte_offset = index as u64 + 1;
        event.provenance.line_number = index as u64 + 1;
        event.usage.cache_write_observed_input_tokens = 0;
        samples.push(event);
    }
    store
        .upsert_verified_events_and_cursor(
            &samples,
            &FileCursor {
                machine_id: "synthetic-machine".into(),
                source_id: crate::sampling::POST_SAMPLING_SOURCE_ID.into(),
                file_identity: "synthetic-log".into(),
                byte_offset: 2,
                line_number: 2,
                parser_state_json: None,
                updated_at: Utc::now(),
            },
        )
        .unwrap();
    (dir, shadow, manifest, seal)
}

fn link_options() -> crate::sampling::LegacySamplingAuditOptions {
    crate::sampling::LegacySamplingAuditOptions {
        start: "2026-01-01T00:00:00Z".parse().unwrap(),
        end: "2026-01-02T00:00:00Z".parse().unwrap(),
        limit: 100,
        max_bytes: 10000,
        include_links: true,
    }
}

#[test]
fn sampling_links_restore_union_without_changing_facts_or_labels() {
    let (dir, shadow, manifest, seal) = linked_fixture();
    let before = LedgerStore::open_read_only(&shadow).unwrap();
    let hashes = before
        .connection
        .prepare("SELECT event_id,event_hash FROM retained_request_evidence ORDER BY event_id")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(before);
    let result = serde_json::to_value(
        link_shadow_sampling(
            &shadow,
            &dir.path().join("home"),
            &manifest,
            &seal,
            link_options(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["linked"], 2);
    assert_eq!(result["metadataConflicts"], 0);
    assert_eq!(result["tokenFactsChanged"], false);
    let again = serde_json::to_value(
        link_shadow_sampling(
            &shadow,
            &dir.path().join("home"),
            &manifest,
            &seal,
            link_options(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(again["linked"], 0);
    assert_eq!(again["unchanged"], 2);
    assert_eq!(
        apply_shadow_correction(&shadow, &manifest, &seal)
            .unwrap()
            .status,
        "already_applied"
    );
    let mut store = LedgerStore::open(&shadow).unwrap();
    let after = store
        .connection
        .prepare("SELECT event_id,event_hash FROM retained_request_evidence ORDER BY event_id")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(after, hashes);
    for _ in 0..100 {
        if store
            .stage_source_union_batch(10, 10, 100)
            .unwrap()
            .projection_ready
        {
            break;
        }
    }
    drop(store);
    let preview = LedgerStore::open_source_union_main_preview(&shadow).unwrap();
    let total = preview
        .aggregate_usage(&AggregateFilter::default())
        .unwrap();
    assert_eq!(total.event_count, 2);
    assert_eq!(total.usage.total_tokens, 150);
    assert_eq!(
        (
            total.usage.input_tokens,
            total.usage.cached_input_tokens,
            total.usage.cache_write_input_tokens,
            total.usage.output_tokens
        ),
        (140, 70, 14, 10)
    );
    assert_eq!(
        total.usage.cache_write_observed_input_tokens, 0,
        "unknown coverage remains separate from consumption"
    );
}

#[test]
fn sampling_link_failure_rolls_back_metadata_upgrade_and_first_link() {
    let (dir, shadow, manifest, seal) = linked_fixture();
    let store = Connection::open(&shadow).unwrap();
    store.execute_batch("CREATE TRIGGER reject_second_link BEFORE INSERT ON source_record_evidence WHEN NEW.evidence_source='sampling' AND NEW.event_id='logs2-post-sampling:2' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    drop(store);
    assert!(
        link_shadow_sampling(
            &shadow,
            &dir.path().join("home"),
            &manifest,
            &seal,
            link_options()
        )
        .is_err()
    );
    let store = Connection::open(&shadow).unwrap();
    assert_eq!(
        store
            .query_row(
                "SELECT COUNT(*) FROM source_record_evidence WHERE evidence_source='sampling'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .query_row("SELECT version FROM review_shadow_meta", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn sampling_metadata_conflicts_remain_visible_without_relabeling() {
    let (dir, shadow, manifest, seal) = linked_fixture();
    let store = LedgerStore::open(&shadow).unwrap();
    store
        .connection
        .execute(
            "UPDATE retained_request_assignments SET account_fingerprint='other-account'",
            [],
        )
        .unwrap();
    drop(store);
    let result = serde_json::to_value(
        link_shadow_sampling(
            &shadow,
            &dir.path().join("home"),
            &manifest,
            &seal,
            link_options(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["linked"], 2);
    assert_eq!(result["metadataConflicts"], 2);
    let mut store = LedgerStore::open(&shadow).unwrap();
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
        store
            .source_union_projection_progress()
            .unwrap()
            .unresolved_groups,
        2
    );
    assert_eq!(store.connection.query_row("SELECT COUNT(*) FROM retained_request_assignments WHERE account_fingerprint='other-account'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn sampling_links_refuse_ambiguous_primary_machine_binding() {
    let (dir, shadow, manifest, seal) = linked_fixture();
    let store = LedgerStore::open(&shadow).unwrap();
    let tx = store.connection.unchecked_transaction().unwrap();
    write_cursor_in(
        &tx,
        &FileCursor {
            machine_id: "another-machine".into(),
            source_id: crate::sampling::POST_SAMPLING_SOURCE_ID.into(),
            file_identity: "another-log".into(),
            byte_offset: 0,
            line_number: 0,
            parser_state_json: None,
            updated_at: Utc::now(),
        },
    )
    .unwrap();
    tx.commit().unwrap();
    drop(store);
    assert!(
        link_shadow_sampling(
            &shadow,
            &dir.path().join("home"),
            &manifest,
            &seal,
            link_options()
        )
        .is_err()
    );
    let store = Connection::open(&shadow).unwrap();
    assert_eq!(
        store
            .query_row(
                "SELECT COUNT(*) FROM source_record_evidence WHERE evidence_source='sampling'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
