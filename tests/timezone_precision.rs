use axum::http::StatusCode;
use codex_usage_ledger::{
    AttributionConfidence, DataQuality, EventProvenance, LedgerStore, ProjectAttribution,
    TokenUsage, UsageEvent,
    api::{ApiState, router},
};
use std::io::{Read, Write};

#[tokio::test]
async fn legacy_hourly_facts_remain_visible_but_unprovable_timezone_splits_return_422() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.sqlite3");
    let at = "2026-01-01T00:10:00Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap();
    let mut store = LedgerStore::open(&path).unwrap();
    store
        .upsert_event(&UsageEvent {
            event_id: "legacy".into(),
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
                cached_input_tokens: 80,
                output_tokens: 20,
                total_tokens: 120,
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
    while !store.backfill_rollup_chunk(100).unwrap().complete {}
    store.verify_rollup_before_compaction().unwrap();
    store
        .compact_raw_events_chunk(at + chrono::Duration::days(30), 100)
        .unwrap();
    drop(store);
    // Model an old synthetic ledger with durable hourly facts but no request detail.
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("DELETE FROM retained_request_evidence", [])
        .unwrap();
    drop(db);
    let app = router(ApiState::with_store(LedgerStore::open(&path).unwrap()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    for (zone, status) in [
        ("UTC", StatusCode::OK),
        ("Asia/Kathmandu", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let (actual_status, json) = tokio::task::spawn_blocking(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            write!(stream, "GET /v1/timeseries?period=custom&startDate=2026-01-01&endDate=2026-01-01&timezone={zone}&grain=hour HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            let mut response = String::new(); stream.read_to_string(&mut response).unwrap();
            let (headers, body) = response.split_once("\r\n\r\n").unwrap();
            let status = headers.split_whitespace().nth(1).unwrap().parse::<u16>().unwrap();
            (status, serde_json::from_str::<serde_json::Value>(body).unwrap())
        }).await.unwrap();
        assert_eq!(actual_status, status.as_u16());
        if status == StatusCode::OK {
            assert_eq!(json["points"][0]["confirmed"]["total"], 120);
            assert_eq!(json["points"][0]["date"], "2026-01-01T00:00");
        } else {
            assert_eq!(json["code"], "insufficient_time_precision");
            assert!(
                json["error"]
                    .as_str()
                    .unwrap()
                    .contains("per-request time evidence")
            );
        }
    }
    let (status, detail)=tokio::task::spawn_blocking(move || {
        let mut stream=std::net::TcpStream::connect(address).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        write!(stream,"GET /v1/explorer?period=custom&startDate=2026-01-01&endDate=2026-01-01&timezone=UTC&session=root HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut response=String::new();stream.read_to_string(&mut response).unwrap();
        let (headers,body)=response.split_once("\r\n\r\n").unwrap();
        (headers.split_whitespace().nth(1).unwrap().to_owned(),serde_json::from_str::<serde_json::Value>(body).unwrap())
    }).await.unwrap();
    assert_eq!(status, "200");
    assert_eq!(detail["selectedSession"]["treeUsage"]["total"], 120);
    assert!(detail["selectedSession"]["localDistributions"]["tree"]["models"].is_null());
    assert!(detail["selectedSession"]["localDistributions"]["tree"]["accounts"].is_null());
    server.abort();
    let _ = server.await;
}
