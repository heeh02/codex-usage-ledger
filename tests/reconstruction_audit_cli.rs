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
        "{\"timestamp\":\"2026-01-01T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1100,\"cached_input_tokens\":0,\"output_tokens\":0,\"reasoning_output_tokens\":0,\"total_tokens\":1100},\"last_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":0,\"output_tokens\":0,\"reasoning_output_tokens\":0,\"total_tokens\":100}}}}\n"
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
    let output_dir = tempfile::tempdir().unwrap();
    let draft = output_dir.path().join("correction.jsonl");
    let draft_run = || {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("draft-reconstruction-correction")
            .arg("--db")
            .arg(&db)
            .arg("--codex-home")
            .arg(home)
            .args(["--thread", "root", "--max-bytes", "4096"])
            .arg("--output")
            .arg(&draft)
            .output()
            .unwrap()
    };
    let generated = draft_run();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let generated: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    assert_eq!(generated["draftOnly"], true);
    assert_eq!(generated["migrationAuthorized"], false);
    let draft_before = fs::read(&draft).unwrap();
    assert!(!draft_run().status.success());
    assert_eq!(fs::read(&draft).unwrap(), draft_before);
    let verified = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("verify-reconstruction-correction")
        .arg("--manifest")
        .arg(&draft)
        .output()
        .unwrap();
    assert!(verified.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&verified.stdout).unwrap(),
        generated
    );
    let checked = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("verify-reconstruction-correction")
        .arg("--manifest")
        .arg(&draft)
        .arg("--against-db")
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let checked: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(checked["ledgerRowsRevalidated"], true);
    assert_eq!(checked["migrationAuthorized"], false);
    let preview = output_dir.path().join("preview.sqlite3");
    let built = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("create-correction-preview")
        .arg("--manifest")
        .arg(&draft)
        .arg("--against-db")
        .arg(&db)
        .arg("--output")
        .arg(&preview)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let built: serde_json::Value = serde_json::from_slice(&built.stdout).unwrap();
    assert_eq!(built["scope"], "single_source_correction_preview");
    assert_eq!(built["old"]["usage"]["total_tokens"], 100);
    let preview_before = fs::read(&preview).unwrap();
    let read = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("read-correction-preview")
        .arg("--preview")
        .arg(&preview)
        .args([
            "--grain",
            "month",
            "--timezone",
            "UTC",
            "--start",
            "2026-01-01T00:00:00Z",
            "--end",
            "2026-01-02T00:00:00Z",
        ])
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(read["candidate"]["usage"]["total_tokens"], 100);
    assert_eq!(read["candidate"]["byTime"][0]["key"], "2026-01");
    let compare = |limit: &str| {
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("compare-correction-sources")
            .arg("--preview")
            .arg(&preview)
            .arg("--against-db")
            .arg(&db)
            .args([
                "--thread",
                "root",
                "--start",
                "2026-01-01T00:00:00Z",
                "--end",
                "2026-01-02T00:00:00Z",
                "--limit",
                limit,
                "--include-rows",
            ])
            .output()
            .unwrap()
    };
    let compared = compare("1");
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let compared: serde_json::Value = serde_json::from_slice(&compared.stdout).unwrap();
    assert_eq!(
        compared["scope"],
        "sampling_vs_candidate_consistency_not_a_union"
    );
    assert_eq!(compared["samplingInScope"], 0);
    assert_eq!(compared["candidatesInScope"], 1);
    assert_eq!(compared["candidatesWithoutSamplingNeighbors"], 1);
    assert_eq!(compared["identityProven"], false);
    assert_eq!(compared["productionPolicyChanged"], false);
    assert_eq!(compared["previewRevalidated"], false);
    assert!(
        compared.get("usage").is_none(),
        "a comparison is not a merged total"
    );
    assert!(!compare("0").status.success());
    assert_eq!(fs::read(&preview).unwrap(), preview_before);
    assert_eq!(fs::read(&db).unwrap(), before);
    assert_eq!(fs::read(&rollout).unwrap(), source);
    assert_eq!(fs::read(home.join("state_5.sqlite")).unwrap(), index_before);
    let child = home.join("sessions/child.jsonl");
    let child_source = format!(
        "{{\"type\":\"session_meta\",\"timestamp\":\"2026-01-02T00:00:00Z\",\"payload\":{{\"id\":\"child\",\"forked_from_id\":\"root\"}}}}\n{}{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\",\"started_at\":1767312001}}}}\n",
        String::from_utf8(source.clone()).unwrap()
    );
    fs::write(&child, &child_source).unwrap();
    let index = rusqlite::Connection::open(home.join("state_5.sqlite")).unwrap();
    index
        .execute(
            "INSERT INTO threads VALUES('child',?1,NULL,'synthetic-model','{}')",
            [child.to_str().unwrap()],
        )
        .unwrap();
    drop(index);
    let index_before = fs::read(home.join("state_5.sqlite")).unwrap();
    let inherited = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("audit-inherited-prefix")
        .arg("--codex-home")
        .arg(home)
        .args(["--thread", "child", "--max-bytes", "4096"])
        .output()
        .unwrap();
    assert!(
        inherited.status.success(),
        "{}",
        String::from_utf8_lossy(&inherited.stderr)
    );
    let inherited: serde_json::Value = serde_json::from_slice(&inherited.stdout).unwrap();
    assert_eq!(inherited["identicalDeclaredPrefix"], true);
    assert_eq!(inherited["matchedRecords"], 1);
    assert_eq!(inherited["migrationReady"], false);
    assert_eq!(fs::read(&db).unwrap(), before);
    assert_eq!(fs::read(&rollout).unwrap(), source);
    assert_eq!(fs::read(&child).unwrap(), child_source.as_bytes());
    assert_eq!(fs::read(home.join("state_5.sqlite")).unwrap(), index_before);
}
