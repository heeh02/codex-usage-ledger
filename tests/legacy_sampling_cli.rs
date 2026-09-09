use codex_usage_ledger::LedgerStore;
use std::process::Command;

#[test]
fn requalification_cli_never_creates_or_migrates_its_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite3");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .args(["audit-legacy-sampling", "--db"])
            .arg(&path)
            .arg("--codex-home")
            .arg(dir.path())
            .args([
                "--thread",
                "synthetic",
                "--start",
                "2026-01-01T00:00:00Z",
                "--end",
                "2026-02-01T00:00:00Z",
            ])
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    assert!(!path.exists());
    drop(LedgerStore::open(&path).unwrap());
    let original = std::fs::read(&path).unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("PRAGMA user_version=35").unwrap();
    drop(connection);
    let original = std::fs::read(&path).unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
