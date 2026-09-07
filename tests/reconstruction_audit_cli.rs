use codex_usage_ledger::{LedgerStore, cli_support::ingest_reconstruction_batch};
use std::{fs, process::Command};

#[test]
fn reconstruction_audit_cli_is_bounded_and_read_only() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    fs::create_dir(home.join("sessions")).unwrap();
    let rollout = home.join("sessions/rollout.jsonl");
    fs::write(&rollout,concat!(
        "{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\"}}\n",
        "{\"timestamp\":\"2026-01-01T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1100,\"total_tokens\":1100},\"last_token_usage\":{\"input_tokens\":100,\"total_tokens\":100}}}}\n"
    )).unwrap();
    let index = rusqlite::Connection::open(home.join("state_5.sqlite")).unwrap();
    index
        .execute_batch(
            "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
        )
        .unwrap();
    index
        .execute(
            "INSERT INTO threads VALUES('root',?1,NULL,'synthetic-model','{}')",
            [rollout.to_str().unwrap()],
        )
        .unwrap();
    drop(index);
    let db = home.join("ledger.sqlite3");
    let mut store = LedgerStore::open(&db).unwrap();
    let imported = ingest_reconstruction_batch(&mut store, home, "synthetic-machine", 1).unwrap();
    assert_eq!(imported.inserted_events, 1);
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    drop(connection);
    let before = fs::read(&db).unwrap();
    let source = fs::read(&rollout).unwrap();
    let index_before = fs::read(home.join("state_5.sqlite")).unwrap();
    let run = |limit: &str| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("audit-reconstruction")
            .arg("--db")
            .arg(&db)
            .arg("--codex-home")
            .arg(home)
            .args(["--thread", "root", "--max-bytes", limit])
            .output()
            .unwrap()
    };
    let output = run("4096");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["version"], 1);
    assert_eq!(report["readOnly"], true);
    assert_eq!(report["migrationReady"], false);
    assert_eq!(report["historyComplete"], false);
    assert_eq!(report["comparisons"][0]["change"], "unchanged");
    assert_eq!(report["comparisons"][0]["recordKeyMatches"], true);
    assert_eq!(report["initialCounterPrefix"]["total_tokens"], 1000);
    let partial = run("1");
    assert!(partial.status.success());
    let partial: serde_json::Value = serde_json::from_slice(&partial.stdout).unwrap();
    assert_eq!(partial["bytesRead"], 1);
    assert_eq!(partial["canonicalSeen"], false);
    assert_eq!(partial["reachedFileEnd"], false);
    assert!(!run("0").status.success());
    let streamed = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("audit-reconstruction-file")
        .arg("--db")
        .arg(&db)
        .arg("--codex-home")
        .arg(home)
        .args(["--thread", "root", "--max-bytes", "4096"])
        .output()
        .unwrap();
    assert!(
        streamed.status.success(),
        "{}",
        String::from_utf8_lossy(&streamed.stderr)
    );
    let streamed: serde_json::Value = serde_json::from_slice(&streamed.stdout).unwrap();
    assert_eq!(streamed["allStoredPositionsSeen"], true);
    assert_eq!(streamed["comparisons"]["unchanged"]["storedRecords"], 1);
    assert_eq!(streamed["migrationReady"], false);
    assert_eq!(fs::read(&db).unwrap(), before);
    assert_eq!(fs::read(&rollout).unwrap(), source);
    assert_eq!(fs::read(home.join("state_5.sqlite")).unwrap(), index_before);
}
