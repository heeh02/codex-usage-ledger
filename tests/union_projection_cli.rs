use codex_usage_ledger::LedgerStore;
use std::process::Command;

#[test]
fn promotion_cli_requires_existing_schema_and_persists_selection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("promote.sqlite3");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .args(["promote-union", "--db"])
            .arg(&path)
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    assert!(!path.exists());
    drop(LedgerStore::open(&path).unwrap());
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["policy"], "request_union_v2");
    assert_eq!(body["restartRequired"], true);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let policy: String = connection
        .query_row(
            "SELECT policy FROM usage_query_policy WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(policy, "request_union_v2");
}

#[test]
fn union_projection_cli_reads_by_default_and_does_not_create_or_migrate() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("candidate.sqlite3");
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .args(["union-projection", "--db"])
            .arg(&path)
            .args(arguments)
            .output()
            .unwrap()
    };
    assert!(!run(&[]).status.success());
    assert!(!run(&["--advance"]).status.success());
    assert!(!path.exists());
    drop(LedgerStore::open(&path).unwrap());
    let before = std::fs::read(&path).unwrap();
    let result = run(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["scope"],
        "staged_local_measurements_not_inference_usage"
    );
    assert_eq!(report["projectionReady"], true);
    assert_eq!(report["historyComplete"], false);
    assert_eq!(report["productionPolicyChanged"], false);
    assert_eq!(before, std::fs::read(&path).unwrap());
    let preview = |query: &str| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .args(["preview-union-bundle", "--db"])
            .arg(&path)
            .args(["--query", query])
            .output()
            .unwrap()
    };
    let response = preview(r#"{"period":"today","timezone":"UTC"}"#);
    assert!(
        response.status.success(),
        "{}",
        String::from_utf8_lossy(&response.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
    assert_eq!(response["policy"], "request_union_preview");
    assert_eq!(response["productionPolicyChanged"], false);
    assert_eq!(response["historyComplete"], false);
    assert!(response["bundle"]["summary"].is_object());
    assert!(response["bundle"]["explorer"].is_object());
    assert!(!preview(r#"{"period":"invalid"}"#).status.success());
    assert_eq!(before, std::fs::read(&path).unwrap());
    let query = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .args(["read-union-projection", "--db"])
            .arg(&path)
            .args([
                "--start",
                "2026-01-01T00:00:00Z",
                "--end",
                "2026-02-01T00:00:00Z",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let read = query(&["--grain", "year", "--timezone", "UTC"]);
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(read["version"], 2);
    assert_eq!(read["resolution"], "materialized_scope");
    assert_eq!(read["status"], "no_records");
    assert_eq!(read["data"]["records"], 0);
    assert!(read["data"]["usage"].is_null());
    assert_eq!(read["productionPolicyChanged"], false);
    assert!(!query(&["--grain", "invalid"]).status.success());
    assert!(!query(&["--timezone", "invalid"]).status.success());
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert!(
        !run(&["--batches", "2"]).status.success(),
        "write batch options require explicit advance"
    );
    assert!(run(&["--advance", "--batches", "2"]).status.success());
    {
        // Deliberately old metadata on a synthetic file verifies the CLI guard.
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA user_version=37").unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(!run(&["--advance"]).status.success());
    assert!(!query(&[]).status.success());
    assert!(!preview("{}").status.success());
    assert_eq!(before, std::fs::read(&path).unwrap());
}
