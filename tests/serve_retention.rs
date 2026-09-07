use codex_usage_ledger::{
    AttributionConfidence, DataQuality, EventProvenance, LedgerStore, ProjectAttribution,
    TokenUsage, UsageEvent,
};
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct OwnedServer(Child);
impl Drop for OwnedServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn dashboard_startup_does_not_compact_or_erase_conflicting_retained_history() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("ledger.sqlite3");
    let home = temp.path().join("empty-codex");
    std::fs::create_dir(&home).unwrap();
    let mut store = LedgerStore::open(&db).unwrap();
    let at = chrono::Utc::now() - chrono::Duration::days(30);
    store
        .upsert_event(&UsageEvent {
            event_id: "synthetic-legacy".into(),
            observed_at: at,
            source_timestamp: Some(at),
            thread_id: Some("root".into()),
            parent_thread_id: None,
            model: Some("synthetic-model".into()),
            cwd: None,
            account_fingerprint: None,
            account_confidence: AttributionConfidence::Unknown,
            project: ProjectAttribution {
                project_id: None,
                project_name: None,
                confidence: AttributionConfidence::Unknown,
                method: "test".into(),
            },
            usage: TokenUsage {
                input_tokens: 100,
                total_tokens: 120,
                output_tokens: 20,
                ..Default::default()
            },
            quality: DataQuality::Confirmed,
            quality_reason: None,
            provenance: EventProvenance {
                source_turn_id: None,
                candidate_rollout_event_id: None,
                sampling_receipt_key: None,
                source_record_key: None,
                machine_id: "machine".into(),
                source_id: "source".into(),
                rollout_id: "root".into(),
                file_identity: "file".into(),
                byte_offset: 1,
                line_number: 1,
            },
        })
        .unwrap();
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute(
            "UPDATE retained_request_evidence SET event_hash='synthetic-mismatch'",
            [],
        )
        .unwrap();
    drop(connection);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut server = OwnedServer(
        Command::new(env!("CARGO_BIN_EXE_codex-usage-ledger"))
            .arg("serve")
            .arg("--db")
            .arg(&db)
            .arg("--codex-home")
            .arg(&home)
            .arg("--listen")
            .arg(address.to_string())
            .arg("--web-root")
            .arg(temp.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while Instant::now() < deadline {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "view-only startup exited during automatic maintenance"
        );
        if let Ok(mut socket) =
            std::net::TcpStream::connect_timeout(&address, Duration::from_millis(100))
        {
            socket
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            if write!(
                socket,
                "GET /v1/bundle?period=lifetime HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n"
            )
            .is_ok()
            {
                let mut data = String::new();
                if socket.read_to_string(&mut data).is_ok()
                    && let Some((_, body)) = data.split_once("\r\n\r\n")
                    && let Ok(json) = serde_json::from_str::<serde_json::Value>(body)
                    && json["collection"]["phase"] == "idle"
                    && json["collection"]["mode"] == "serve"
                {
                    assert_eq!(json["summary"]["usage"]["confirmed"]["total"], 120);
                    ready = true;
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(ready, "dashboard never reached its usable idle state");
    assert!(server.0.try_wait().unwrap().is_none());
    drop(server);
    let c = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM usage_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row(
            "SELECT event_hash FROM retained_request_evidence",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "synthetic-mismatch"
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM compacted_event_keys", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
