use codex_usage_ledger::LedgerStore;
use std::process::Command;

#[test]
fn backfill_command_resumes_then_remains_complete_across_process_restarts() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ledger.sqlite3");
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("backfill-requests")
            .arg("--db")
            .arg(&path)
            .args(extra)
            .output()
            .unwrap()
    };
    assert!(!run(&[]).status.success());
    assert!(!path.exists());
    drop(LedgerStore::open(&path).unwrap());
    // Simulate a persisted unfinished empty target. The first process completes
    // it; the second must not restart the completed work.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("UPDATE request_backfill_state SET complete=0", [])
        .unwrap();
    for expected in [1, 0] {
        let output = run(&[]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["batchesAttempted"], expected);
        assert_eq!(report["backfillComplete"], true);
        assert_eq!(report["historyComplete"], false);
        assert_eq!(report["sourceImport"], false);
    }
    assert!(!run(&["--batches", "101"]).status.success());
    assert!(!run(&["--batches", "0"]).status.success());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 24).unwrap();
    drop(connection);
    assert!(!run(&[]).status.success());
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        24
    );
}
