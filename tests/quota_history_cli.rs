use codex_usage_ledger::LedgerStore;
use std::process::Command;

#[test]
fn quota_history_cli_is_read_only_and_never_creates_missing_ledgers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite3");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("quota-history")
            .arg("--db")
            .arg(&path)
            .args(["--account", "account-a"])
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    assert!(!path.exists());
    drop(LedgerStore::open(&path).unwrap());
    let before = std::fs::read(&path).unwrap();
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let page: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(page["indexReady"], true);
    assert_eq!(page["sourceHistoryComplete"], false);
    assert_eq!(page["view"]["account"], "account-a");
    assert_eq!(page["intervals"].as_array().unwrap().len(), 0);
    assert!(!String::from_utf8_lossy(&result.stdout).contains("cursor_key"));
    assert_eq!(before, std::fs::read(&path).unwrap());
}
