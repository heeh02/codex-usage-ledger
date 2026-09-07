use super::*;
use crate::store::ReconstructionAuditFact;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Comparison {
    pub(super) byte_offset: u64,
    pub(super) change: &'static str,
    pub(super) record_key_matches: Option<bool>,
    pub(super) stored: Option<ReconstructionAuditFact>,
    pub(super) proposed: Option<ReconstructionAuditFact>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> (tempfile::TempDir, LedgerStore, PathBuf) {
        fixture_with_device_drift(false)
    }

    #[test]
    fn correction_draft_roundtrips_without_changing_sources_or_overwriting_files() {
        let (temp, store, path) = fixture();
        let out = tempfile::tempdir().unwrap();
        let draft = out.path().join("draft.jsonl");
        let db = temp.path().join("ledger.sqlite3");
        let before_source = fs::read(&path).unwrap();
        let before_db = fs::read(&db).unwrap();
        let changes = store.connection().total_changes();
        let report = serde_json::to_value(
            super::super::write_correction_manifest(
                &db,
                temp.path(),
                "root",
                4096,
                100,
                false,
                &draft,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(report["records"], 1);
        assert_eq!(report["fullSourceScan"], true);
        assert_eq!(
            report["categories"]["changed_candidate"]["storedUsage"]["total_tokens"],
            1100
        );
        assert_eq!(
            report["categories"]["changed_candidate"]["proposedUsage"]["total_tokens"],
            100
        );
        assert_eq!(report["migrationAuthorized"], false);
        assert_eq!(report["sourceRevalidated"], false);
        let bytes = fs::read(&draft).unwrap();
        assert!(
            super::super::write_correction_manifest(
                &db,
                temp.path(),
                "root",
                4096,
                100,
                false,
                &draft
            )
            .is_err()
        );
        assert_eq!(fs::read(&draft).unwrap(), bytes);
        assert!(
            super::super::write_correction_manifest(
                &db,
                temp.path(),
                "root",
                4096,
                100,
                false,
                &temp.path().join("unsafe.jsonl")
            )
            .is_err()
        );
        assert!(!temp.path().join("unsafe.jsonl").exists());
        let partial = out.path().join("partial-scan.jsonl");
        let report = serde_json::to_value(
            super::super::write_correction_manifest(
                &db,
                temp.path(),
                "root",
                1,
                100,
                false,
                &partial,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(report["fullSourceScan"], false);
        assert_eq!(report["records"], 0);
        assert_eq!(store.connection().total_changes(), changes);
        assert_eq!(fs::read(&path).unwrap(), before_source);
        assert_eq!(fs::read(&db).unwrap(), before_db);
        let checked = serde_json::to_value(
            super::super::verify_correction_against_ledger(&draft, &db).unwrap(),
        )
        .unwrap();
        assert_eq!(checked["ledgerRowsRevalidated"], true);
        assert_eq!(checked["sourceRevalidated"], false);
        // Hash equality alone is not enough: metadata projections can change
        // while an ingestion hash stays unchanged.
        store
            .connection()
            .execute(
                "UPDATE reconstruction_usage_events SET model='changed-after-draft'",
                [],
            )
            .unwrap();
        assert!(super::super::verify_correction_against_ledger(&draft, &db).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&draft).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn correction_preview_is_separate_and_failed_seals_roll_back_candidate_rows() {
        let (temp, store, path) = fixture();
        let out = tempfile::tempdir().unwrap();
        let db = temp.path().join("ledger.sqlite3");
        let draft = out.path().join("draft.jsonl");
        let preview = out.path().join("preview.sqlite3");
        super::super::write_correction_manifest(&db, temp.path(), "root", 4096, 100, false, &draft)
            .unwrap();
        let db_before = fs::read(&db).unwrap();
        let source_before = fs::read(&path).unwrap();
        let changes = store.connection().total_changes();
        let report = serde_json::to_value(
            crate::store::create_correction_preview(&draft, &db, &preview).unwrap(),
        )
        .unwrap();
        assert_eq!(report["old"]["usage"]["total_tokens"], 1100);
        assert_eq!(report["candidate"]["usage"]["total_tokens"], 100);
        assert_eq!(report["migrationAuthorized"], false);
        let preview_before = fs::read(&preview).unwrap();
        assert!(crate::store::create_correction_preview(&draft, &db, &preview).is_err());
        assert_eq!(fs::read(&preview).unwrap(), preview_before);
        let mut bytes = fs::read(&draft).unwrap();
        bytes.pop();
        fs::write(&draft, bytes).unwrap();
        let failed = out.path().join("failed.sqlite3");
        assert!(crate::store::create_correction_preview(&draft, &db, &failed).is_err());
        assert!(
            crate::store::read_correction_preview(
                &failed,
                &crate::store::CorrectionPreviewFilter::default()
            )
            .is_err()
        );
        let connection = Connection::open(&failed).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM preview_facts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row("SELECT ready FROM preview_meta", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(store.connection().total_changes(), changes);
        assert_eq!(fs::read(&db).unwrap(), db_before);
        assert_eq!(fs::read(&path).unwrap(), source_before);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&preview).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn streaming_audit_reaches_beyond_prefix_and_checks_unseen_stored_positions() {
        let (temp, store, path) = fixture();
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        let padding =
            serde_json::json!({"type":"response_item","payload":{"text":"x".repeat(8192)}});
        for _ in 0..650 {
            writeln!(file, "{padding}").unwrap();
        }
        let token = serde_json::json!({"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":{"input_tokens":1200,"cached_input_tokens":1060,"output_tokens":0,"total_tokens":1200},
            "last_token_usage":{"input_tokens":100,"cached_input_tokens":60,"output_tokens":0,"total_tokens":100}}}});
        writeln!(file, "{token}").unwrap();
        drop(file);
        let db = temp.path().join("ledger.sqlite3");
        let before = fs::read(&path).unwrap();
        let db_before = fs::read(&db).unwrap();
        let changes = store.connection().total_changes();
        let run = |bytes, rows| {
            serde_json::to_value(
                super::super::audit_reconstruction_file(
                    &db,
                    temp.path(),
                    "root",
                    bytes,
                    rows,
                    false,
                )
                .unwrap(),
            )
            .unwrap()
        };
        let partial = run(4096, 100);
        assert_eq!(partial["reachedFileEnd"], false);
        assert_eq!(partial["allStoredPositionsSeen"], false);
        let limited = run(8 * 1024 * 1024, 1);
        assert_eq!(limited["tokenRecords"], 1);
        assert_eq!(limited["reachedFileEnd"], false);
        let full = run(8 * 1024 * 1024, 100);
        assert_eq!(full["reachedFileEnd"], true);
        assert_eq!(full["allStoredPositionsSeen"], true);
        assert_eq!(full["tokenRecords"], 2);
        assert_eq!(full["storedRecordsSeen"], 1);
        assert_eq!(full["usageChangedPairs"], 1);
        assert_eq!(
            full["comparisons"]["changed_candidate"]["storedUsage"]["total_tokens"],
            1100
        );
        assert_eq!(
            full["comparisons"]["changed_candidate"]["proposedUsage"]["total_tokens"],
            100
        );
        assert_eq!(
            full["comparisons"]["new_candidate"]["proposedUsage"]["total_tokens"],
            100
        );
        assert!(full["comparisons"]["new_candidate"]["storedUsage"].is_null());
        assert_eq!(full["migrationReady"], false);
        assert_eq!(full["historyComplete"], false);
        assert_eq!(store.connection().total_changes(), changes);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(&db).unwrap(), db_before);
        // A retained event at a source position with no corresponding token row
        // must prevent a coverage claim even after reaching EOF.
        store.connection().execute("UPDATE reconstruction_usage_events SET event_id='orphaned-position',byte_offset=999",[]).unwrap();
        let orphan = run(8 * 1024 * 1024, 100);
        assert_eq!(orphan["storedRecordsNotSeen"], 1);
        assert_eq!(orphan["allStoredPositionsSeen"], false);
    }

    #[test]
    fn streaming_audit_old_schema_and_partial_json_never_trigger_migration() {
        let (temp, store, path) = fixture();
        let db = temp.path().join("ledger.sqlite3");
        // Version guard fixture: shared source tables are unchanged in 35..38.
        store
            .connection()
            .execute_batch("PRAGMA user_version=35")
            .unwrap();
        let before = store.connection().total_changes();
        let report =
            super::super::audit_reconstruction_file(&db, temp.path(), "root", 4096, 100, false)
                .unwrap();
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["ledgerSchema"], 35);
        assert_eq!(store.schema_version().unwrap(), 35);
        assert_eq!(store.connection().total_changes(), before);
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"partial\"")
            .unwrap();
        let json = serde_json::to_value(
            super::super::audit_reconstruction_file(&db, temp.path(), "root", 4096, 100, false)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["reachedFileEnd"], false);
        assert_eq!(json["allStoredPositionsSeen"], false);
        store
            .connection()
            .execute_batch("PRAGMA user_version=34")
            .unwrap();
        assert!(
            super::super::audit_reconstruction_file(&db, temp.path(), "root", 4096, 100, false)
                .is_err()
        );
        assert_eq!(store.schema_version().unwrap(), 34);
    }

    #[test]
    fn streaming_audit_explains_foreign_history_suppression() {
        let (temp, store, path) = fixture();
        let original = fs::read_to_string(&path).unwrap();
        let token = original.lines().nth(1).unwrap();
        let foreign = serde_json::json!({"type":"session_meta","payload":{"id":"ancestor"}});
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "{foreign}").unwrap();
        let offset = file.metadata().unwrap().len();
        writeln!(file, "{token}").unwrap();
        drop(file);
        let identity = physical_file_identity(&path, &fs::metadata(&path).unwrap()).unwrap();
        let id = stable_event_id("machine", &identity, "root", offset);
        store
            .connection()
            .execute(
                "UPDATE reconstruction_usage_events SET event_id=?1,byte_offset=?2",
                rusqlite::params![id, offset as i64],
            )
            .unwrap();
        let report = serde_json::to_value(
            super::super::audit_reconstruction_file(
                &temp.path().join("ledger.sqlite3"),
                temp.path(),
                "root",
                4096,
                100,
                false,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(report["allStoredPositionsSeen"], true);
        assert_eq!(
            report["suppressedByRule"]["foreign_history_guard"]["records"],
            1
        );
        assert_eq!(
            report["suppressedByRule"]["foreign_history_guard"]["storedUsage"]["total_tokens"],
            1100
        );
        assert_eq!(
            report["comparisons"]["suppressed_candidate"]["storedUsage"]["total_tokens"],
            1100
        );
        assert_eq!(report["migrationReady"], false);
    }

    fn fixture_with_device_drift(drift: bool) -> (tempfile::TempDir, LedgerStore, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let sessions = temp.path().join("sessions");
        fs::create_dir(&sessions).unwrap();
        let path = sessions.join("rollout.jsonl");
        let meta = serde_json::json!({"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"id":"root"}});
        let token = serde_json::json!({"timestamp":"2026-01-01T00:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":{"input_tokens":1100,"cached_input_tokens":1000,"output_tokens":0,"total_tokens":1100},
            "last_token_usage":{"input_tokens":100,"cached_input_tokens":60,"output_tokens":0,"total_tokens":100}}}});
        fs::write(&path, format!("{meta}\n{token}\n")).unwrap();
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads VALUES('root',?1,NULL,'model','{}')",
                [path.to_str().unwrap()],
            )
            .unwrap();
        drop(index);
        let mut identity = physical_file_identity(&path, &fs::metadata(&path).unwrap()).unwrap();
        if drift {
            let parts = identity.split(':').collect::<Vec<_>>();
            identity = format!("unix:{}:{}", parts[1].parse::<u64>().unwrap() + 1, parts[2]);
        }
        let target = Target {
            thread_id: "root".into(),
            parent_thread_id: None,
            path: path.clone(),
            cwd: None,
            model: Some("model".into()),
        };
        let mut store = LedgerStore::open(temp.path().join("ledger.sqlite3")).unwrap();
        let attribution = target_attribution(&store, &target).unwrap();
        let mut state = ReconstructionCheckpoint::new(&target);
        let mut tail = IncrementalJsonlTailer::default();
        let batch = tail.poll_path(&path).unwrap();
        let mut old = None;
        for line in &batch.lines {
            old = process_line(
                &mut state,
                line,
                &target,
                "machine",
                &source_id("root"),
                &identity,
                &attribution,
                &[],
            )
            .unwrap()
            .or(old);
        }
        let mut old = old.unwrap();
        // Exact source record, but the older parser incorrectly emitted its total.
        old.event.usage = TokenUsage {
            input_tokens: 1100,
            cached_input_tokens: 1000,
            total_tokens: 1100,
            ..Default::default()
        };
        let status = ReconstructionSourceStatus {
            machine_id: "machine".into(),
            source_id: source_id("root"),
            thread_id: "root".into(),
            file_identity: identity.clone(),
            status: ReconstructionStatus::Reconstructed,
            bytes_total: batch.bytes_read as u64,
            bytes_processed: batch.bytes_read as u64,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
            last_error: None,
            updated_at: Utc::now(),
        };
        let cursor = FileCursor {
            machine_id: "machine".into(),
            source_id: source_id("root"),
            file_identity: identity,
            byte_offset: batch.bytes_read as u64,
            line_number: 2,
            parser_state_json: None,
            updated_at: Utc::now(),
        };
        store
            .upsert_reconstruction_events_and_cursor(&[old], &status, &cursor)
            .unwrap();
        (temp, store, path)
    }

    #[test]
    fn prefix_audit_matches_old_and_new_without_mutating_any_input() {
        let (temp, store, path) = fixture();
        let source = fs::read(&path).unwrap();
        let index = fs::read(temp.path().join("state_5.sqlite")).unwrap();
        let changes = store.connection().total_changes();
        let cursor = store.get_cursor("machine", &source_id("root")).unwrap();
        let report =
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 100, false).unwrap();
        assert!(report.canonical_seen && report.reached_file_end);
        assert!(
            !report.migration_ready
                && !report.history_complete
                && !report.source_changed_during_read
        );
        assert_eq!(report.comparisons.len(), 1);
        let row = &report.comparisons[0];
        assert_eq!(row.change, "changed_candidate");
        assert_eq!(row.record_key_matches, Some(true));
        assert_eq!(row.stored.as_ref().unwrap().usage.total_tokens, 1100);
        assert_eq!(row.proposed.as_ref().unwrap().usage.total_tokens, 100);
        assert!(row.stored.as_ref().unwrap().stored_hash.is_some());
        assert_eq!(report.initial_counter_prefix.unwrap().total_tokens, 1000);
        assert_eq!(store.connection().total_changes(), changes);
        assert_eq!(
            store.get_cursor("machine", &source_id("root")).unwrap(),
            cursor
        );
        assert_eq!(fs::read(&path).unwrap(), source);
        assert_eq!(fs::read(temp.path().join("state_5.sqlite")).unwrap(), index);
        assert!(store.connection().is_autocommit());
    }

    #[test]
    #[cfg(unix)]
    fn identity_change_does_not_delete_history_or_restart_reconstruction() {
        let (temp, mut store, _) = fixture_with_device_drift(true);
        let cursor = store.get_cursor("machine", &source_id("root")).unwrap();
        for _ in 0..2 {
            let report =
                ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
            assert_eq!(report.bytes_read, 0);
            assert_eq!(report.inserted_events, 0);
            assert_eq!(report.pending_sources, 0);
            assert!(
                report
                    .issues
                    .iter()
                    .any(|issue| issue == "source_identity_verification_required")
            );
            assert_eq!(
                store.get_cursor("machine", &source_id("root")).unwrap(),
                cursor
            );
            assert_eq!(
                store
                    .aggregate_rollup_usage(&crate::store::AggregateFilter::default())
                    .unwrap()
                    .usage
                    .total_tokens,
                1100,
                "preserve the previous facts for a reviewed correction, not automatic deletion"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn device_drift_is_opt_in_diagnostic_not_an_identity_rebind() {
        let (temp, store, _) = fixture_with_device_drift(true);
        let changes = store.connection().total_changes();
        assert!(
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 100, false).is_err()
        );
        let report =
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 100, true).unwrap();
        assert_eq!(
            report.identity_relation,
            "unix_device_changed_same_inode_candidate"
        );
        assert!(!report.migration_ready);
        assert_eq!(report.comparisons[0].record_key_matches, Some(true));
        assert_eq!(
            report.comparisons[0]
                .stored
                .as_ref()
                .unwrap()
                .usage
                .total_tokens,
            1100
        );
        assert_eq!(
            report.comparisons[0]
                .proposed
                .as_ref()
                .unwrap()
                .usage
                .total_tokens,
            100
        );
        assert_eq!(store.connection().total_changes(), changes);
        assert!(!device_only_drift("unix:1:2", "unix:2:3"));
        assert!(!device_only_drift("unix:bad:2", "unix:2:2"));
        assert!(!device_only_drift("unix:1:2:extra", "unix:2:2"));
    }

    #[test]
    fn missing_or_invalid_saved_parser_state_never_restarts_from_zero() {
        for state in [None, Some("{"), Some("{}")] {
            let (temp, mut store, path) = fixture();
            store
                .connection()
                .execute("UPDATE file_cursors SET parser_state_json=?1", [state])
                .unwrap();
            let saved = store.get_cursor("machine", &source_id("root")).unwrap();
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(b"\n")
                .unwrap();
            let report =
                ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
            assert_eq!(report.identity_review_sources, 1);
            assert_eq!(report.bytes_read, 0);
            assert_eq!(
                store.get_cursor("machine", &source_id("root")).unwrap(),
                saved
            );
        }
    }

    #[test]
    fn unbound_source_recovery_never_overrides_history_progress_or_review() {
        for evidence in [
            "events",
            "cursor",
            "bytes",
            "prefix",
            "unchanged",
            "reset",
            "review",
        ] {
            let (temp, mut store, _) = fixture();
            store.connection().execute_batch("UPDATE reconstruction_sources SET file_identity='',status='unrecoverable',bytes_processed=0,prefix_events=0,unchanged_events=0,counter_resets=0,last_error='initial failure'").unwrap();
            if evidence != "events" {
                store
                    .connection()
                    .execute("DELETE FROM reconstruction_usage_events", [])
                    .unwrap();
            }
            if evidence != "cursor" {
                store
                    .connection()
                    .execute("DELETE FROM file_cursors", [])
                    .unwrap();
            }
            let sql = match evidence {
                "bytes" => "UPDATE reconstruction_sources SET bytes_processed=1",
                "prefix" => "UPDATE reconstruction_sources SET prefix_events=1",
                "unchanged" => "UPDATE reconstruction_sources SET unchanged_events=1",
                "reset" => "UPDATE reconstruction_sources SET counter_resets=1",
                "review" => {
                    "UPDATE reconstruction_sources SET last_error='source_identity_verification_required'"
                }
                _ => "SELECT 1",
            };
            store.connection().execute_batch(sql).unwrap();
            let saved = store.get_cursor("machine", &source_id("root")).unwrap();
            for _ in 0..2 {
                let report =
                    ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
                assert_eq!(report.identity_review_sources, 1, "{evidence}");
                assert_eq!(report.bytes_read, 0, "{evidence}");
                assert_eq!(report.inserted_events, 0, "{evidence}");
                let source = store.reconstruction_sources().unwrap().remove(0);
                assert!(source.file_identity.is_empty(), "{evidence}");
                assert_eq!(
                    store.get_cursor("machine", &source_id("root")).unwrap(),
                    saved,
                    "{evidence}"
                );
            }
        }
    }

    #[test]
    fn a_source_missing_on_first_discovery_is_imported_when_it_arrives() {
        let (temp, old_store, path) = fixture();
        drop(old_store);
        let parked = temp.path().join("parked.jsonl");
        fs::rename(&path, &parked).unwrap();
        let mut store = LedgerStore::open(temp.path().join("late-ledger.sqlite3")).unwrap();
        let absent = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(absent.inserted_events, 0);
        assert!(
            absent
                .issues
                .iter()
                .any(|issue| issue == "indexed_reconstruction_source_unavailable")
        );
        drop(store);
        let mut store = LedgerStore::open(temp.path().join("late-ledger.sqlite3")).unwrap();
        fs::rename(parked, path).unwrap();
        let arrived = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(
            arrived.inserted_events, 1,
            "a transiently missing first file must not be permanently skipped"
        );
        assert_eq!(
            store
                .aggregate_rollup_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100
        );
        let idle = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(idle.bytes_read, 0);
        assert_eq!(idle.inserted_events, 0);
    }

    #[test]
    fn structurally_inconsistent_checkpoint_cannot_replay_from_zero() {
        let (temp, mut store, path) = fixture();
        let target = load_targets(temp.path()).unwrap().remove(0);
        let encoded = serde_json::to_string(&ReconstructionCheckpoint::new(&target)).unwrap();
        store
            .connection()
            .execute("UPDATE file_cursors SET parser_state_json=?1", [encoded])
            .unwrap();
        let saved = store.get_cursor("machine", &source_id("root")).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        let report = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(report.identity_review_sources, 1);
        assert_eq!(report.bytes_read, 0);
        assert_eq!(
            store.get_cursor("machine", &source_id("root")).unwrap(),
            saved
        );
    }

    #[test]
    fn read_time_identity_conflict_preserves_the_saved_cursor() {
        let (temp, mut store, path) = fixture();
        store
            .connection()
            .execute("UPDATE file_cursors SET file_identity='older-identity'", [])
            .unwrap();
        let saved = store.get_cursor("machine", &source_id("root")).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        let report = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(report.identity_review_sources, 1);
        assert_eq!(
            store.get_cursor("machine", &source_id("root")).unwrap(),
            saved
        );
        let wrapped = source_read_error(std::io::Error::from(std::io::ErrorKind::NotFound), true)
            .context("synthetic context");
        assert!(wrapped.is::<SourceContinuityError>());
    }

    #[test]
    #[cfg(unix)]
    fn review_survives_temporary_source_disappearance_and_return() {
        let (temp, mut store, path) = fixture_with_device_drift(true);
        ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        let saved = store.get_cursor("machine", &source_id("root")).unwrap();
        let parked = temp.path().join("parked.jsonl");
        fs::rename(&path, &parked).unwrap();
        let missing = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(missing.identity_review_sources, 1);
        fs::rename(parked, path).unwrap();
        let restored = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(restored.identity_review_sources, 1);
        assert_eq!(
            store.get_cursor("machine", &source_id("root")).unwrap(),
            saved
        );
    }

    #[test]
    fn in_place_truncation_preserves_the_checkpoint_for_verification() {
        let (temp, mut store, path) = fixture();
        let saved = store.get_cursor("machine", &source_id("root")).unwrap();
        fs::write(path, "{}\n").unwrap();
        for _ in 0..2 {
            let report =
                ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
            assert_eq!(
                store.get_cursor("machine", &source_id("root")).unwrap(),
                saved
            );
            assert_eq!(report.identity_review_sources, 1);
            assert_eq!(report.inserted_events, 0);
        }
    }

    #[test]
    fn actual_file_replacement_also_preserves_existing_facts() {
        let (temp, mut store, path) = fixture();
        let saved = store.get_cursor("machine", &source_id("root")).unwrap();
        let replacement = path.with_file_name("replacement.jsonl");
        fs::write(&replacement, fs::read(&path).unwrap()).unwrap();
        fs::rename(replacement, &path).unwrap();
        let report = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(report.bytes_read, 0);
        assert_eq!(report.pending_sources, 0);
        assert_eq!(
            store.get_cursor("machine", &source_id("root")).unwrap(),
            saved
        );
        assert_eq!(
            store
                .aggregate_rollup_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            1100
        );
    }

    #[test]
    fn audit_limits_and_mismatches_do_not_claim_complete_history() {
        let (temp, store, path) = fixture();
        let partial =
            audit_reconstruction_prefix(&store, temp.path(), "root", 1, 100, false).unwrap();
        assert_eq!(partial.bytes_read, 1);
        assert!(!partial.canonical_seen && !partial.reached_file_end);
        assert!(partial.comparisons.is_empty());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{}\n")
            .unwrap();
        let limited =
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 1, false).unwrap();
        assert!(!limited.reached_file_end);
        assert!(audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 0, false).is_err());
        fs::write(
            &path,
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"other\"}}\n",
        )
        .unwrap();
        assert!(
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 100, false)
                .unwrap_err()
                .to_string()
                .contains("canonical identity")
        );
        let outside = temp.path().join("outside.jsonl");
        fs::write(&outside, "not an authorized rollout").unwrap();
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute(
                "UPDATE threads SET rollout_path=?1",
                [outside.to_str().unwrap()],
            )
            .unwrap();
        drop(index);
        assert!(
            audit_reconstruction_prefix(&store, temp.path(), "root", 4096, 100, false)
                .unwrap_err()
                .to_string()
                .contains("outside the explicit Codex source roots")
        );
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconstructionPrefixAudit {
    version: u32,
    scope: &'static str,
    read_only: bool,
    migration_ready: bool,
    history_complete: bool,
    thread: String,
    identity_relation: &'static str,
    bytes_read: usize,
    records_processed: usize,
    canonical_seen: bool,
    reached_file_end: bool,
    partial_line_bytes: usize,
    source_changed_during_read: bool,
    prefix_digest: String,
    initial_counter_prefix: Option<TokenUsage>,
    comparisons: Vec<Comparison>,
}

/// Preview only one bounded source prefix against stored facts. No source,
/// checkpoint, derived selector, catalog or event is created/updated/deleted.
pub fn audit_reconstruction_prefix(
    store: &LedgerStore,
    codex_home: &Path,
    thread: &str,
    max_bytes: usize,
    max_rows: usize,
    allow_device_drift: bool,
) -> Result<ReconstructionPrefixAudit> {
    if thread.is_empty()
        || !(1..=16 * 1024 * 1024).contains(&max_bytes)
        || !(1..=500).contains(&max_rows)
    {
        return Err(anyhow!(
            "require thread, byte limit 1..16777216 and row limit 1..500"
        ));
    }
    let target = resolve_target(codex_home, thread)?;
    audit_target_prefix(
        store,
        target,
        thread,
        max_bytes,
        max_rows,
        allow_device_drift,
    )
}

pub(super) fn resolve_target(codex_home: &Path, thread: &str) -> Result<Target> {
    let home = codex_home.canonicalize()?;
    let index = home.join("state_5.sqlite").canonicalize()?;
    if !index.starts_with(&home)
        || index.file_name().and_then(|n| n.to_str()) != Some("state_5.sqlite")
    {
        return Err(anyhow!("source index is outside the explicit Codex home"));
    }
    let mut target = load_targets(&home)?
        .into_iter()
        .find(|target| target.thread_id == thread)
        .ok_or_else(|| anyhow!("thread is absent from the native index"))?;
    target.path = target.path.canonicalize()?;
    let allowed = [home.join("sessions"), home.join("archived_sessions")]
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| root.starts_with(&home) && target.path.starts_with(root));
    if !allowed {
        return Err(anyhow!(
            "rollout is outside the explicit Codex source roots"
        ));
    }
    Ok(target)
}

fn audit_target_prefix(
    store: &LedgerStore,
    target: Target,
    thread: &str,
    max_bytes: usize,
    max_rows: usize,
    allow_device_drift: bool,
) -> Result<ReconstructionPrefixAudit> {
    let before = fs::metadata(&target.path)?;
    let identity = physical_file_identity(&target.path, &before)?;
    store.with_source_audit_snapshot(|store| -> Result<_> {
        let sources = store
            .reconstruction_sources()?
            .into_iter()
            .filter(|source| {
                source.thread_id == thread
                    && (source.file_identity == identity
                        || (allow_device_drift
                            && device_only_drift(&source.file_identity, &identity)))
            })
            .collect::<Vec<_>>();
        if sources.len() != 1 {
            return Err(anyhow!(
                "audit needs exactly one matching stored source identity"
            ));
        }
        let source = &sources[0];
        let attribution = target_attribution(store, &target)?;
        let epochs = load_account_epochs(store, &source.machine_id)?;
        let mut tailer = IncrementalJsonlTailer::with_limits(
            TailCheckpoint::default(),
            TailLimits {
                read_chunk_bytes: max_bytes,
                max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            },
        )?;
        let batch = tailer.poll_path(&target.path)?;
        let mut digest = Sha256::new();
        for line in &batch.lines {
            digest.update(line.byte_offset.to_le_bytes());
            digest.update((line.raw.len() as u64).to_le_bytes());
            digest.update(&line.raw);
        }
        digest.update(&batch.checkpoint.partial_line);
        let mut state = ReconstructionCheckpoint::new(&target);
        let mut comparisons = Vec::new();
        let mut processed = 0;
        let mut tokens = 0;
        for line in &batch.lines {
            if tokens == max_rows {
                break;
            }
            let record = line.parse_json().ok();
            if state.phase == ReconstructionPhase::AwaitingCanonical
                && record
                    .as_ref()
                    .and_then(|v| v.get("type"))
                    .and_then(Value::as_str)
                    == Some("session_meta")
                && record
                    .as_ref()
                    .and_then(|v| v.pointer("/payload/id"))
                    .and_then(Value::as_str)
                    != Some(thread)
            {
                return Err(anyhow!(
                    "rollout canonical identity differs from native index"
                ));
            }
            let proposal = process_line(
                &mut state,
                line,
                &target,
                &source.machine_id,
                &source.source_id,
                &source.file_identity,
                &attribution,
                &epochs,
            )?;
            processed += 1;
            if record
                .as_ref()
                .and_then(|v| v.get("type"))
                .and_then(Value::as_str)
                != Some("event_msg")
                || record
                    .as_ref()
                    .and_then(|v| v.pointer("/payload/type"))
                    .and_then(Value::as_str)
                    != Some("token_count")
            {
                continue;
            }
            tokens += 1;
            comparisons.push(compare_proposal(
                store,
                &target,
                source,
                line,
                record.as_ref().unwrap(),
                proposal,
            )?);
        }
        let after = fs::metadata(&target.path)?;
        Ok(ReconstructionPrefixAudit {
            version: 1,
            scope: "bounded_reconstruction_prefix_not_a_migration_receipt",
            read_only: true,
            migration_ready: false,
            history_complete: false,
            thread: thread.into(),
            identity_relation: if source.file_identity == identity {
                "exact"
            } else {
                "unix_device_changed_same_inode_candidate"
            },
            bytes_read: batch.bytes_read,
            records_processed: processed,
            canonical_seen: state.phase != ReconstructionPhase::AwaitingCanonical,
            reached_file_end: processed == batch.lines.len()
                && !batch.has_more
                && batch.checkpoint.partial_line.is_empty(),
            partial_line_bytes: batch.checkpoint.partial_line.len(),
            source_changed_during_read: before.len() != after.len()
                || before.modified()? != after.modified()?
                || identity != physical_file_identity(&target.path, &after)?,
            prefix_digest: hex::encode(digest.finalize()),
            initial_counter_prefix: state.initial_counter_prefix,
            comparisons,
        })
    })
}

pub(super) fn compare_proposal(
    store: &LedgerStore,
    target: &Target,
    source: &ReconstructionSourceStatus,
    line: &JsonlLine,
    record: &Value,
    proposal: Option<ReconstructionEvent>,
) -> Result<Comparison> {
    let thread = target.thread_id.as_str();
    let id = stable_event_id(
        &source.machine_id,
        &source.file_identity,
        thread,
        line.byte_offset,
    );
    let stored = store.reconstruction_audit_fact(&id)?;
    if let Some(stored) = &stored {
        stored.usage.validate()?;
    }
    let key = source_record_key(
        &source.machine_id,
        &source.file_identity,
        thread,
        line.byte_offset,
        &source_record_digest(record),
    );
    let key_matches = stored
        .as_ref()
        .and_then(|old| old.record_key.as_ref())
        .map(|old| old == &key);
    let proposed = proposal.map(|row| {
        let event = row.event;
        ReconstructionAuditFact {
            event_id: event.event_id,
            stored_hash: None,
            at: event.source_timestamp.unwrap_or(event.observed_at),
            thread: event.thread_id,
            model: event.model,
            account: event.account_fingerprint,
            project: event.project.project_id,
            record_key: event.provenance.source_record_key,
            usage: event.usage,
        }
    });
    let change = match (&stored, &proposed) {
        (None, None) => "not_emitted",
        (None, Some(_)) => "new_candidate",
        (Some(_), None) => "suppressed_candidate",
        (Some(old), Some(new))
            if old.usage == new.usage
                && old.at == new.at
                && old.thread == new.thread
                && old.model == new.model
                && old.account == new.account
                && old.project == new.project =>
        {
            "unchanged"
        }
        _ => "changed_candidate",
    };
    Ok(Comparison {
        byte_offset: line.byte_offset,
        change,
        record_key_matches: key_matches,
        stored,
        proposed,
    })
}

pub(super) fn device_only_drift(old: &str, current: &str) -> bool {
    fn parts(value: &str) -> Option<(u64, u64)> {
        let mut parts = value.split(':');
        if parts.next() != Some("unix") {
            return None;
        }
        let device = parts.next()?.parse().ok()?;
        let inode = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some((device, inode))
    }
    matches!((parts(old),parts(current)), (Some((a,i)),Some((b,j))) if a!=b&&i==j&&i!=0)
}
