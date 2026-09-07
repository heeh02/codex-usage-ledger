use codex_usage_ledger::LedgerStore;
use std::process::Command;

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
    assert_eq!(before, std::fs::read(&path).unwrap());
}
