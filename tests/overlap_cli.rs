use codex_usage_ledger::{
    AttributionConfidence, DataQuality, EventProvenance, LedgerStore, ProjectAttribution,
    TokenUsage, UsageEvent,
};
use std::process::Command;

#[test]
fn overlap_cli_reads_scoped_evidence_without_mutating_the_database() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ledger.sqlite3");
    {
        let mut store = LedgerStore::open(&path).unwrap();
        store
            .upsert_event(&UsageEvent {
                event_id: "synthetic-zero".into(),
                observed_at: "2026-01-02T01:00:00Z".parse().unwrap(),
                source_timestamp: None,
                thread_id: Some("synthetic-thread".into()),
                parent_thread_id: None,
                model: Some("synthetic-model".into()),
                cwd: None,
                account_fingerprint: None,
                account_confidence: AttributionConfidence::Unknown,
                project: ProjectAttribution {
                    project_id: None,
                    project_name: None,
                    confidence: AttributionConfidence::Unknown,
                    method: "unknown".into(),
                },
                usage: TokenUsage::default(),
                quality: DataQuality::Confirmed,
                quality_reason: None,
                provenance: EventProvenance {
                    source_turn_id: None,
                    candidate_rollout_event_id: None,
                    sampling_receipt_key: None,
                    source_record_key: None,
                    machine_id: "synthetic-machine".into(),
                    source_id: "synthetic-source".into(),
                    rollout_id: "synthetic-rollout".into(),
                    file_identity: "synthetic-file".into(),
                    byte_offset: 1,
                    line_number: 1,
                },
            })
            .unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("audit-overlap")
        .arg("--db")
        .arg(&path)
        .args([
            "--thread",
            "synthetic-thread",
            "--start",
            "2026-01-02T00:00:00Z",
            "--end",
            "2026-01-03T00:00:00Z",
            "--limit",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["readOnly"], true);
    assert_eq!(report["auditVersion"], 3);
    assert_eq!(report["requestEqualityProven"], false);
    assert_eq!(report["historyComplete"], false);
    assert_eq!(report["rows"].as_array().unwrap().len(), 1);
    assert_eq!(report["rows"][0]["status"], "not_linked");
    assert!(report["rows"][0]["comparison"]["candidateId"].is_null());
    assert!(report["rows"][0]["comparison"]["candidateUsage"].is_null());
    assert_eq!(report["rows"][0]["confirmedUsage"]["total_tokens"], 0);
    assert_eq!(report["rows"][0]["retainedSideSelectedByDayPolicy"], true);
    assert_eq!(report["dayPolicyContexts"][0]["samplingRecords"], 1);
    assert_eq!(
        report["dayPolicyContexts"][0]["scope"],
        "full_storage_day_all_accounts_and_models"
    );
    assert!(report["next"].is_null());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let rejected = Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
        .arg("audit-overlap")
        .arg("--db")
        .arg(&path)
        .args([
            "--thread",
            "synthetic-thread",
            "--start",
            "2026-01-02T00:00:00Z",
            "--end",
            "2026-01-03T00:00:00Z",
            "--after-id",
            "unpaired",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
