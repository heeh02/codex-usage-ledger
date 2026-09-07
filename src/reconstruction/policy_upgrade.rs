use super::*;

pub(super) const SOURCE_PREFIX: &str = "reconstruction-policy-upgrade-v2:";
pub(super) const REVIEW_REQUIRED: &str = "reconstruction_parser_policy_review_required";

#[derive(Debug, Serialize, Deserialize)]
struct Upgrade {
    version: u32,
    policy: String,
    expected_main: String,
    target_end: u64,
    target_lines: u64,
    parent: Option<String>,
    model_hint: Option<String>,
    cwd_hint: Option<String>,
    file_len: u64,
    modified: DateTime<Utc>,
    prefix_digest: String,
    state: Option<ReconstructionCheckpoint>,
    finished: bool,
    historical_facts_unchanged: bool,
}

fn fingerprint(cursor: &FileCursor) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(&(
            &cursor.machine_id,
            &cursor.source_id,
            &cursor.file_identity,
            cursor.byte_offset,
            cursor.line_number,
            &cursor.parser_state_json,
        ))
        .expect("cursor tuple"),
    ))
}

fn progress_id(thread: &str) -> String {
    format!("{SOURCE_PREFIX}{thread}")
}

/// Rebuild only parser state through the old committed prefix, in bounded
/// slices. Historical event rows are never replayed into the ledger here.
#[allow(clippy::too_many_arguments)]
pub(super) fn advance(
    store: &mut LedgerStore,
    target: &Target,
    main: &FileCursor,
    old: &ReconstructionCheckpoint,
    previous_status: &ReconstructionSourceStatus,
    _epochs: &[AccountEpoch],
    chunk_bytes: usize,
) -> Result<(BatchOutcome, ReconstructionSourceStatus, u64)> {
    let before_meta = target.path.metadata()?;
    let identity = physical_file_identity(&target.path, &before_meta)?;
    if identity != main.file_identity {
        return Err(SourceContinuityError::IdentityChanged.into());
    }
    let target_end = if old.tail.partial_line.is_empty() {
        main.byte_offset
    } else {
        old.tail.partial_offset
    };
    let id = progress_id(&target.thread_id);
    let prior = store.get_cursor(&main.machine_id, &id)?;
    let mut upgrade = if let Some(cursor) = &prior {
        let value: Upgrade = serde_json::from_str(
            cursor
                .parser_state_json
                .as_deref()
                .ok_or(SourceContinuityError::CheckpointUnavailable)?,
        )
        .map_err(|_| SourceContinuityError::CheckpointUnavailable)?;
        let state = value
            .state
            .as_ref()
            .ok_or(SourceContinuityError::UnsupportedPolicy)?;
        if value.version != 1
            || value.policy != correction_manifest::CURRENT_RECONSTRUCTION_POLICY
            || value.finished
            || value.expected_main != fingerprint(main)
            || value.target_end != target_end
            || value.target_lines != main.line_number
            || value.prefix_digest.len() != 64
            || !value
                .prefix_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || value.parent != target.parent_thread_id
            || cursor.file_identity != identity
            || cursor.byte_offset != state.tail.next_offset
            || cursor.line_number != state.tail.completed_lines
            || state.tail.next_offset > target_end
            || !state.tail.partial_line.is_empty()
            || state.parser_policy.as_deref()
                != Some(correction_manifest::CURRENT_RECONSTRUCTION_POLICY)
        {
            return Err(SourceContinuityError::UnsupportedPolicy.into());
        }
        value
    } else {
        Upgrade {
            version: 1,
            policy: correction_manifest::CURRENT_RECONSTRUCTION_POLICY.into(),
            expected_main: fingerprint(main),
            target_end,
            target_lines: main.line_number,
            parent: target.parent_thread_id.clone(),
            model_hint: target.model.clone(),
            cwd_hint: target.cwd.clone(),
            file_len: before_meta.len(),
            modified: before_meta.modified()?.into(),
            prefix_digest: "0".repeat(64),
            state: Some(ReconstructionCheckpoint::new(target)),
            finished: false,
            historical_facts_unchanged: true,
        }
    };
    let before_modified: DateTime<Utc> = before_meta.modified()?.into();
    if before_meta.len() < main.byte_offset
        || before_meta.len() < upgrade.file_len
        || (before_meta.len() == upgrade.file_len && before_modified != upgrade.modified)
    {
        return Err(SourceContinuityError::ContentChanged.into());
    }
    let mut state = upgrade
        .state
        .take()
        .ok_or(SourceContinuityError::UnsupportedPolicy)?;
    let mut tailer = IncrementalJsonlTailer::with_limits(
        state.tail.clone(),
        TailLimits {
            read_chunk_bytes: chunk_bytes,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        },
    )?;
    let mut file = fs::File::open(&target.path)?;
    let opened = file.metadata()?;
    if physical_file_identity(&target.path, &opened)? != identity || opened.len() < target_end {
        return Err(SourceContinuityError::IdentityChanged.into());
    }
    let batch = tailer.poll_reader(&mut file, target_end, &identity)?;
    if batch.reset.is_some() {
        return Err(SourceContinuityError::ContentChanged.into());
    }
    let after = target.path.metadata()?;
    let after_modified: DateTime<Utc> = after.modified()?.into();
    if physical_file_identity(&target.path, &after)? != identity
        || after.len() < before_meta.len()
        || (after.len() == before_meta.len() && after_modified != before_modified)
    {
        return Err(SourceContinuityError::ContentChanged.into());
    }
    if batch.checkpoint.next_offset == target_end && !batch.checkpoint.partial_line.is_empty() {
        return Err(SourceContinuityError::CheckpointUnavailable.into());
    }
    let frozen_target = Target {
        thread_id: target.thread_id.clone(),
        parent_thread_id: upgrade.parent.clone(),
        path: target.path.clone(),
        cwd: upgrade.cwd_hint.clone(),
        model: upgrade.model_hint.clone(),
    };
    let attribution = TargetAttribution {
        parent_thread_id: upgrade.parent.clone(),
        project: ProjectAttribution {
            project_id: None,
            project_name: None,
            confidence: AttributionConfidence::Unknown,
            method: "state_requalification_only".into(),
        },
    };
    for line in &batch.lines {
        // The returned proposal is intentionally discarded. It is not a new
        // request and is not a reviewed historical correction.
        let _ = process_line(
            &mut state,
            line,
            &frozen_target,
            &main.machine_id,
            &main.source_id,
            &identity,
            &attribution,
            &[],
        )?;
        let mut digest = Sha256::new();
        digest.update(upgrade.prefix_digest.as_bytes());
        digest.update(line.byte_offset.to_le_bytes());
        digest.update((line.raw.len() as u64).to_le_bytes());
        digest.update(&line.raw);
        upgrade.prefix_digest = hex::encode(digest.finalize());
    }
    let previous_offset = state.tail.next_offset;
    state.tail = batch.checkpoint;
    if !state.tail.partial_line.is_empty() {
        state.tail.next_offset = state.tail.partial_offset;
        state.tail.partial_line.clear();
    }
    let finished = state.tail.next_offset == target_end;
    if (!finished && state.tail.next_offset == previous_offset)
        || (finished && state.tail.completed_lines != main.line_number)
    {
        return Err(SourceContinuityError::CheckpointUnavailable.into());
    }
    let progress_offset = state.tail.next_offset;
    let progress_lines = state.tail.completed_lines;
    let mut status = previous_status.clone();
    status.bytes_processed = main.byte_offset;
    status.bytes_total = after.len();
    status.updated_at = Utc::now();
    status.last_error = None;
    status.status =
        if finished && after.len() == main.byte_offset && old.tail.partial_line.is_empty() {
            ReconstructionStatus::Reconstructed
        } else {
            ReconstructionStatus::Reconstructing
        };
    let replacement = if finished {
        status.prefix_events = state.prefix_events;
        status.unchanged_events = state.unchanged_events;
        status.counter_resets = state.counter_resets;
        state.tail = old.tail.clone(); // Same committed boundary; partial record is still uncommitted.
        let mut cursor = main.clone();
        cursor.parser_state_json = Some(serde_json::to_string(&state)?);
        cursor.updated_at = status.updated_at;
        Some(cursor)
    } else {
        upgrade.state = Some(state);
        None
    };
    upgrade.finished = finished;
    upgrade.file_len = after.len();
    upgrade.modified = after_modified;
    let progress = FileCursor {
        machine_id: main.machine_id.clone(),
        source_id: id,
        file_identity: identity,
        byte_offset: progress_offset,
        line_number: progress_lines,
        parser_state_json: Some(serde_json::to_string(&upgrade)?),
        updated_at: status.updated_at,
    };
    let committed = store.commit_reconstruction_policy_step(
        main,
        prior.as_ref(),
        &progress,
        &status,
        replacement.as_ref(),
    );
    if matches!(
        committed,
        Err(crate::store::StoreError::ReconstructionPolicyOverlap)
    ) {
        return Err(SourceContinuityError::UnsupportedPolicy.into());
    }
    committed?;
    Ok((BatchOutcome::default(), status, batch.bytes_read as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn token(at: &str, total: u64, last: u64) -> Value {
        serde_json::json!({"timestamp":at,"type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":{"input_tokens":total,"cached_input_tokens":total,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":total},
            "last_token_usage":{"input_tokens":last,"cached_input_tokens":last,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":last}}}})
    }

    fn fixture(
        records: usize,
    ) -> (
        tempfile::TempDir,
        LedgerStore,
        Target,
        ReconstructionCheckpoint,
        FileCursor,
        ReconstructionSourceStatus,
    ) {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("sessions")).unwrap();
        let path = temp.path().join("sessions/root.jsonl");
        let mut body = format!(
            "{}\n",
            serde_json::json!({"timestamp":"2026-09-01T00:00:00Z","type":"session_meta","payload":{"id":"root"}})
        );
        for index in 1..=records {
            body.push_str(&format!(
                "{}\n",
                token("2026-09-01T00:00:01Z", index as u64 * 100, 100)
            ));
        }
        fs::write(&path, body).unwrap();
        let target = Target {
            thread_id: "root".into(),
            parent_thread_id: None,
            path: path.clone(),
            cwd: None,
            model: Some("model".into()),
        };
        let mut store = LedgerStore::open(temp.path().join("ledger.sqlite3")).unwrap();
        let mut tailer = IncrementalJsonlTailer::default();
        let batch = tailer.poll_path(&path).unwrap();
        let identity = physical_file_identity(&path, &path.metadata().unwrap()).unwrap();
        let mut old = ReconstructionCheckpoint::new(&target);
        let attribution = target_attribution(&store, &target).unwrap();
        for line in &batch.lines {
            let _ = process_line(
                &mut old,
                line,
                &target,
                "machine",
                &source_id("root"),
                &identity,
                &attribution,
                &[],
            )
            .unwrap();
        }
        old.tail = batch.checkpoint;
        old.parser_policy = None;
        let main = FileCursor {
            machine_id: "machine".into(),
            source_id: source_id("root"),
            file_identity: identity.clone(),
            byte_offset: old.tail.next_offset,
            line_number: old.tail.completed_lines,
            parser_state_json: Some(serde_json::to_string(&old).unwrap()),
            updated_at: Utc::now(),
        };
        let status = ReconstructionSourceStatus {
            machine_id: "machine".into(),
            source_id: source_id("root"),
            thread_id: "root".into(),
            file_identity: identity,
            status: ReconstructionStatus::Reconstructed,
            bytes_total: main.byte_offset,
            bytes_processed: main.byte_offset,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
            last_error: None,
            updated_at: Utc::now(),
        };
        store
            .upsert_reconstruction_events_and_cursor(&[], &status, &main)
            .unwrap();
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads VALUES('root',?1,NULL,'model','{}')",
                [path.to_str().unwrap()],
            )
            .unwrap();
        (temp, store, target, old, main, status)
    }

    #[test]
    fn requalification_resumes_small_slices_and_idle_does_not_repeat_it() {
        let (temp, mut store, target, old, main, mut status) = fixture(30);
        let mut previous = 0;
        let mut calls = 0;
        loop {
            let (_, next, bytes) =
                advance(&mut store, &target, &main, &old, &status, &[], 1024).unwrap();
            assert!(bytes <= 1024);
            let cursor = store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .unwrap();
            assert!(cursor.byte_offset > previous);
            assert!(cursor.parser_state_json.as_ref().unwrap().len() < 4096);
            let progress: Upgrade =
                serde_json::from_str(cursor.parser_state_json.as_ref().unwrap()).unwrap();
            assert!(
                progress
                    .state
                    .as_ref()
                    .is_none_or(|state| state.tail.partial_line.is_empty())
            );
            previous = cursor.byte_offset;
            calls += 1;
            if progress.finished {
                break;
            }
            assert_eq!(
                store
                    .get_cursor("machine", &main.source_id)
                    .unwrap()
                    .unwrap(),
                main
            );
            assert!(
                next.last_error.is_none(),
                "healthy upgrade progress is not a source error"
            );
            status = next;
            drop(store);
            store = LedgerStore::open(temp.path().join("ledger.sqlite3")).unwrap();
        }
        assert!(calls > 1);
        let upgraded = store
            .get_cursor("machine", &main.source_id)
            .unwrap()
            .unwrap();
        assert_eq!(upgraded.byte_offset, main.byte_offset);
        let state: ReconstructionCheckpoint =
            serde_json::from_str(upgraded.parser_state_json.as_ref().unwrap()).unwrap();
        assert_eq!(
            state.parser_policy.as_deref(),
            Some(correction_manifest::CURRENT_RECONSTRUCTION_POLICY)
        );
        assert_eq!(state.previous_total.unwrap().total_tokens, 3000);
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            0
        );
        let idle = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!((idle.bytes_read, idle.files_advanced), (0, 0));
    }

    #[test]
    fn active_upgrade_is_scheduled_even_without_new_source_bytes() {
        let (temp, mut store, target, old, main, status) = fixture(20);
        advance(&mut store, &target, &main, &old, &status, &[], 1024).unwrap();
        let report = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert!(report.bytes_read > 0);
        assert_eq!(report.inserted_events, 0);
        let upgraded = store
            .get_cursor("machine", &main.source_id)
            .unwrap()
            .unwrap();
        let state: ReconstructionCheckpoint =
            serde_json::from_str(upgraded.parser_state_json.as_ref().unwrap()).unwrap();
        assert_eq!(
            state.parser_policy.as_deref(),
            Some(correction_manifest::CURRENT_RECONSTRUCTION_POLICY)
        );
    }

    #[test]
    fn source_change_and_stale_main_do_not_overwrite_upgrade_progress() {
        let (_temp, mut store, target, old, main, status) = fixture(20);
        advance(&mut store, &target, &main, &old, &status, &[], 1024).unwrap();
        let prior = store
            .get_cursor("machine", &progress_id("root"))
            .unwrap()
            .unwrap();
        let mut newer = main.clone();
        newer.parser_state_json = Some("{\"different\":true}".into());
        store.advance_cursor(&newer).unwrap();
        let error = advance(&mut store, &target, &main, &old, &status, &[], 1024).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<crate::store::StoreError>(),
            Some(crate::store::StoreError::ReconstructionPolicyConflict)
        ));
        assert_eq!(
            store
                .get_cursor("machine", &main.source_id)
                .unwrap()
                .unwrap(),
            newer
        );
        assert_eq!(
            store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .unwrap(),
            prior
        );
        fs::File::options()
            .write(true)
            .open(&target.path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
            ))
            .unwrap();
        assert!(advance(&mut store, &target, &main, &old, &status, &[], 1024).is_err());
        assert_eq!(
            store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .unwrap(),
            prior
        );
    }

    #[test]
    fn history_beyond_the_cursor_requires_review_not_automatic_requalification() {
        let (_temp, mut store, target, old, main, status) = fixture(1);
        let mut live = old.clone();
        let line = JsonlLine {
            byte_offset: main.byte_offset,
            line_number: main.line_number + 1,
            raw: serde_json::to_vec(&token("2026-09-01T00:00:02Z", 200, 100)).unwrap(),
        };
        let attribution = target_attribution(&store, &target).unwrap();
        let event = process_line(
            &mut live,
            &line,
            &target,
            "machine",
            &main.source_id,
            &main.file_identity,
            &attribution,
            &[],
        )
        .unwrap()
        .unwrap();
        store
            .upsert_reconstruction_events_and_cursor(&[event], &status, &main)
            .unwrap();
        assert!(advance(&mut store, &target, &main, &old, &status, &[], 4096).is_err());
        assert_eq!(
            store
                .get_cursor("machine", &main.source_id)
                .unwrap()
                .unwrap(),
            main
        );
        assert!(
            store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100
        );
    }

    #[test]
    fn partial_record_stays_uncommitted_until_after_policy_upgrade() {
        let (_temp, mut store, target, mut old, mut main, mut status) = fixture(1);
        let next = format!("{}\n", token("2026-09-01T00:00:02Z", 200, 100));
        fs::OpenOptions::new()
            .append(true)
            .open(&target.path)
            .unwrap()
            .write_all(&next.as_bytes()[..40])
            .unwrap();
        let mut tailer = IncrementalJsonlTailer::from_checkpoint(old.tail.clone());
        old.tail = tailer.poll_path(&target.path).unwrap().checkpoint;
        main.byte_offset = old.tail.next_offset;
        main.parser_state_json = Some(serde_json::to_string(&old).unwrap());
        status.bytes_total = main.byte_offset;
        status.bytes_processed = main.byte_offset;
        store
            .upsert_reconstruction_events_and_cursor(&[], &status, &main)
            .unwrap();
        advance(&mut store, &target, &main, &old, &status, &[], 4096).unwrap();
        assert_eq!(
            store
                .get_cursor("machine", &main.source_id)
                .unwrap()
                .unwrap()
                .byte_offset,
            main.byte_offset
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&target.path)
            .unwrap()
            .write_all(&next.as_bytes()[40..])
            .unwrap();
        assert_eq!(
            ingest_target(
                &mut store,
                "machine",
                &target,
                target.path.metadata().unwrap().len(),
                &status,
                &[]
            )
            .unwrap()
            .0
            .inserted,
            1
        );
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100
        );
    }

    #[test]
    fn unknown_future_policy_is_preserved_and_reported_for_review() {
        let (temp, mut store, target, mut old, mut main, _status) = fixture(1);
        old.parser_policy = Some("future-parser-policy".into());
        main.parser_state_json = Some(serde_json::to_string(&old).unwrap());
        store.advance_cursor(&main).unwrap();
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(&target.path)
                .unwrap(),
            "{}",
            token("2026-09-01T00:00:02Z", 200, 100)
        )
        .unwrap();
        let report = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(report.unrecoverable_sources, 1);
        assert_eq!(report.identity_review_sources, 0);
        assert!(report.issues.iter().any(|issue| issue == REVIEW_REQUIRED));
        assert_eq!(
            store
                .get_cursor("machine", &main.source_id)
                .unwrap()
                .unwrap(),
            main
        );
        assert!(
            store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn final_state_and_receipt_roll_back_together_when_source_write_fails() {
        let (_temp, mut store, target, old, main, status) = fixture(1);
        store
            .connection()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_policy_source BEFORE UPDATE ON reconstruction_sources
            BEGIN SELECT RAISE(ABORT, 'synthetic policy failure'); END;",
            )
            .unwrap();
        assert!(advance(&mut store, &target, &main, &old, &status, &[], 4096).is_err());
        assert_eq!(
            store
                .get_cursor("machine", &main.source_id)
                .unwrap()
                .unwrap(),
            main
        );
        assert!(
            store
                .get_cursor("machine", &progress_id("root"))
                .unwrap()
                .is_none()
        );
        store
            .connection()
            .execute_batch("DROP TRIGGER fail_policy_source")
            .unwrap();
        advance(&mut store, &target, &main, &old, &status, &[], 4096).unwrap();
        let current = store
            .get_cursor("machine", &main.source_id)
            .unwrap()
            .unwrap();
        let state: ReconstructionCheckpoint =
            serde_json::from_str(current.parser_state_json.as_ref().unwrap()).unwrap();
        assert_eq!(
            state.parser_policy.as_deref(),
            Some(correction_manifest::CURRENT_RECONSTRUCTION_POLICY)
        );
    }

    #[test]
    fn legacy_live_checkpoint_is_requalified_without_recounting_its_prefix() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("sessions")).unwrap();
        let path = temp.path().join("sessions/child.jsonl");
        let prefix = format!(
            "{}\n{}\n{}\n",
            serde_json::json!({"timestamp":"2026-09-01T00:00:00Z","type":"session_meta","payload":{"id":"child"}}),
            serde_json::json!({"type":"session_meta","payload":{"id":"parent"}}),
            token("2026-09-01T00:00:01Z", 100, 100)
        );
        fs::write(&path, &prefix).unwrap();
        let db = temp.path().join("ledger.sqlite3");
        let mut store = LedgerStore::open(&db).unwrap();
        let target = Target {
            thread_id: "child".into(),
            parent_thread_id: Some("parent".into()),
            path: path.clone(),
            cwd: None,
            model: Some("child-model".into()),
        };
        let identity = physical_file_identity(&path, &path.metadata().unwrap()).unwrap();
        let mut tailer = IncrementalJsonlTailer::default();
        let batch = tailer.poll_path(&path).unwrap();
        let mut legacy = ReconstructionCheckpoint::new(&target);
        legacy.tail = batch.checkpoint;
        legacy.phase = ReconstructionPhase::Live; // The old replay guard escaped too early.
        legacy.previous_total = Some(TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 100,
            total_tokens: 100,
            ..TokenUsage::default()
        });
        legacy.canonical_at = Some(
            DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        );
        let mut encoded = serde_json::to_value(&legacy).unwrap();
        encoded.as_object_mut().unwrap().remove("parser_policy");
        let cursor = FileCursor {
            machine_id: "machine".into(),
            source_id: source_id("child"),
            file_identity: identity.clone(),
            byte_offset: prefix.len() as u64,
            line_number: 3,
            parser_state_json: Some(encoded.to_string()),
            updated_at: Utc::now(),
        };
        let status = ReconstructionSourceStatus {
            machine_id: "machine".into(),
            source_id: source_id("child"),
            thread_id: "child".into(),
            file_identity: identity,
            status: ReconstructionStatus::Reconstructed,
            bytes_total: prefix.len() as u64,
            bytes_processed: prefix.len() as u64,
            prefix_events: 0,
            unchanged_events: 0,
            counter_resets: 0,
            last_error: None,
            updated_at: Utc::now(),
        };
        let mut seeded = legacy.clone();
        seeded.previous_total = None;
        seeded.model = Some("legacy-model".into());
        let attribution = target_attribution(&store, &target).unwrap();
        let historical = process_line(
            &mut seeded,
            batch.lines.last().unwrap(),
            &target,
            "machine",
            &cursor.source_id,
            &cursor.file_identity,
            &attribution,
            &[],
        )
        .unwrap()
        .unwrap();
        let historical_id = historical.event.event_id.clone();
        store
            .upsert_reconstruction_events_and_cursor(&[historical], &status, &cursor)
            .unwrap();
        let before_history = store
            .reconstruction_audit_fact(&historical_id)
            .unwrap()
            .unwrap();
        writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(),"{}\n{}\n{}",
            token("2026-09-01T00:00:02Z",200,100),
            serde_json::json!({"type":"event_msg","payload":{"type":"task_started","started_at":1788220803}}),
            token("2026-09-01T00:00:04Z",250,50)).unwrap();
        let mut unsafe_state = legacy.clone();
        let unqualified = IncrementalJsonlTailer::from_checkpoint(legacy.tail.clone())
            .poll_path(&path)
            .unwrap();
        let mut unsafe_total = 0;
        for line in &unqualified.lines {
            if let Some(event) = process_line(
                &mut unsafe_state,
                line,
                &target,
                "machine",
                &cursor.source_id,
                &cursor.file_identity,
                &attribution,
                &[],
            )
            .unwrap()
            {
                unsafe_total += event.event.usage.total_tokens;
            }
        }
        assert_eq!(
            unsafe_total, 150,
            "blindly reusing the old live state includes 100 inherited tokens"
        );
        let (outcome, _, _) = ingest_target(
            &mut store,
            "machine",
            &target,
            path.metadata().unwrap().len(),
            &status,
            &[],
        )
        .unwrap();
        assert_eq!(
            outcome.inserted, 0,
            "the upgrade must neither recount history nor continue from the wrong live state"
        );
        let after_history = store
            .reconstruction_audit_fact(&historical_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&before_history).unwrap(),
            serde_json::to_value(&after_history).unwrap()
        );
        assert_eq!(before_history.stored_hash, after_history.stored_hash);
        assert_eq!(before_history.usage, after_history.usage);
        assert_eq!(
            store
                .get_cursor("machine", &source_id("child"))
                .unwrap()
                .unwrap()
                .byte_offset,
            cursor.byte_offset
        );
        drop(store);
        let mut store = LedgerStore::open(&db).unwrap();
        let (outcome, _, _) = ingest_target(
            &mut store,
            "machine",
            &target,
            path.metadata().unwrap().len(),
            &status,
            &[],
        )
        .unwrap();
        assert_eq!(outcome.inserted, 1);
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            150
        );
        let expected = TokenUsage {
            input_tokens: 150,
            cached_input_tokens: 150,
            total_tokens: 150,
            ..TokenUsage::default()
        };
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage,
            expected
        );
        for dimension in [
            crate::store::AggregateDimension::Account,
            crate::store::AggregateDimension::Project,
            crate::store::AggregateDimension::Model,
            crate::store::AggregateDimension::Thread,
            crate::store::AggregateDimension::Day,
        ] {
            let buckets = store
                .aggregate_exact_time_series(
                    crate::store::TimeGrain::Day,
                    Some(dimension),
                    &crate::store::AggregateFilter::default(),
                    "Asia/Shanghai",
                )
                .unwrap();
            for field in [
                "input_tokens",
                "cached_input_tokens",
                "cache_write_input_tokens",
                "cache_write_observed_input_tokens",
                "output_tokens",
                "reasoning_output_tokens",
                "total_tokens",
            ] {
                let sum: u64 = buckets
                    .iter()
                    .map(|bucket| {
                        serde_json::to_value(bucket.usage).unwrap()[field]
                            .as_u64()
                            .unwrap_or(0)
                    })
                    .sum();
                assert_eq!(
                    sum,
                    serde_json::to_value(expected).unwrap()[field]
                        .as_u64()
                        .unwrap_or(0),
                    "{dimension:?}/{field}"
                );
            }
        }
    }
}
