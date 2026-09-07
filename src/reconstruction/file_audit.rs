//! Streaming read-only comparison of one indexed file, not a migration receipt.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonTotals {
    records: u64,
    stored_records: u64,
    proposed_records: u64,
    stored_usage: Option<TokenUsage>,
    proposed_usage: Option<TokenUsage>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuppressedTotals {
    records: u64,
    stored_usage: TokenUsage,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconstructionFileAudit {
    version: u32,
    scope: &'static str,
    read_only: bool,
    migration_ready: bool,
    history_complete: bool,
    ledger_schema: i64,
    thread: String,
    identity_relation: &'static str,
    file_bytes_at_start: u64,
    bytes_read: usize,
    records_processed: u64,
    malformed_records: u64,
    token_records: u64,
    reached_file_end: bool,
    canonical_seen: bool,
    source_changed_during_read: bool,
    stored_scope_records: u64,
    stored_records_seen: u64,
    stored_records_not_seen: u64,
    all_stored_positions_seen: bool,
    source_keys_missing: u64,
    source_keys_matching: u64,
    source_keys_conflicting: u64,
    usage_changed_pairs: u64,
    processed_records_digest: String,
    initial_counter_prefix: Option<TokenUsage>,
    comparisons: BTreeMap<&'static str, ComparisonTotals>,
    suppressed_by_rule: BTreeMap<&'static str, SuppressedTotals>,
}

pub fn audit_reconstruction_file(
    db: &Path,
    codex_home: &Path,
    thread: &str,
    max_bytes: usize,
    max_token_rows: usize,
    allow_device_drift: bool,
) -> Result<ReconstructionFileAudit> {
    run_file_audit(
        db,
        codex_home,
        thread,
        max_bytes,
        max_token_rows,
        allow_device_drift,
        &mut correction_manifest::NoopSink,
    )
}

pub(super) fn run_file_audit(
    db: &Path,
    codex_home: &Path,
    thread: &str,
    max_bytes: usize,
    max_token_rows: usize,
    allow_device_drift: bool,
    sink: &mut dyn correction_manifest::EvidenceSink,
) -> Result<ReconstructionFileAudit> {
    if thread.is_empty()
        || !(1..=2_147_483_648).contains(&max_bytes)
        || !(1..=10_000_000).contains(&max_token_rows)
    {
        return Err(anyhow!(
            "require thread, byte limit 1..2147483648 and token-row limit 1..10000000"
        ));
    }
    let store = LedgerStore::open_reconstruction_audit(db)?;
    let target = audit::resolve_target(codex_home, thread)?;
    let before = fs::metadata(&target.path)?;
    let identity = physical_file_identity(&target.path, &before)?;
    store.with_source_audit_snapshot(|store| -> Result<_> {
        let sources = store
            .reconstruction_sources()?
            .into_iter()
            .filter(|source| {
                source.thread_id == thread
                    && (source.file_identity == identity
                        || allow_device_drift
                            && audit::device_only_drift(&source.file_identity, &identity))
            })
            .collect::<Vec<_>>();
        if sources.len() != 1 {
            return Err(anyhow!(
                "audit needs exactly one matching stored source identity"
            ));
        }
        let source = &sources[0];
        sink.begin(correction_manifest::ManifestHeader {
            version: 1,
            policy: "reconstruction_uuid7_strict_v1".into(),
            ledger_schema: store.schema_version()?,
            machine_id: source.machine_id.clone(),
            source_id: source.source_id.clone(),
            thread: thread.into(),
            stored_file_identity: source.file_identity.clone(),
            observed_file_identity: identity.clone(),
            file_bytes_at_start: before.len(),
            created_at: Utc::now(),
        })?;
        let attribution = target_attribution(store, &target)?;
        let epochs = load_account_epochs(store, &source.machine_id)?;
        let mut report = ReconstructionFileAudit {
            version: 1,
            scope: "streamed_source_comparison_not_a_migration_receipt",
            read_only: true,
            migration_ready: false,
            history_complete: false,
            ledger_schema: store.schema_version()?,
            thread: thread.into(),
            identity_relation: if source.file_identity == identity {
                "exact"
            } else {
                "unix_device_changed_same_inode_candidate"
            },
            file_bytes_at_start: before.len(),
            bytes_read: 0,
            records_processed: 0,
            malformed_records: 0,
            token_records: 0,
            reached_file_end: false,
            canonical_seen: false,
            source_changed_during_read: false,
            stored_scope_records: store.reconstruction_audit_count(source)?,
            stored_records_seen: 0,
            stored_records_not_seen: 0,
            all_stored_positions_seen: false,
            source_keys_missing: 0,
            source_keys_matching: 0,
            source_keys_conflicting: 0,
            usage_changed_pairs: 0,
            processed_records_digest: String::new(),
            initial_counter_prefix: None,
            comparisons: BTreeMap::new(),
            suppressed_by_rule: BTreeMap::new(),
        };
        let mut state = ReconstructionCheckpoint::new(&target);
        let mut checkpoint = TailCheckpoint::default();
        let mut digest = Sha256::new();
        'scan: while report.bytes_read < max_bytes && report.token_records < (max_token_rows as u64)
        {
            let mut tailer = IncrementalJsonlTailer::with_limits(
                checkpoint,
                TailLimits {
                    read_chunk_bytes: (max_bytes - report.bytes_read).min(4 * 1024 * 1024),
                    max_line_bytes: DEFAULT_MAX_LINE_BYTES,
                },
            )?;
            let batch = tailer.poll_path(&target.path)?;
            if batch.reset.is_some() {
                return Err(anyhow!("source changed during streamed audit"));
            }
            report.bytes_read += batch.bytes_read;
            for line in &batch.lines {
                if report.token_records == max_token_rows as u64 {
                    break 'scan;
                }
                let record = line.parse_json().ok();
                if record.is_none() && !line.is_blank() {
                    report.malformed_records += 1;
                }
                if state.phase == ReconstructionPhase::AwaitingCanonical
                    && record
                        .as_ref()
                        .and_then(|v| v.get("type"))
                        .and_then(Value::as_str)
                        == Some("session_meta")
                    && record
                        .as_ref()
                        .and_then(|v| v.pointer("/payload/id"))
                        .and_then(Value::as_str)
                        != Some(thread)
                {
                    return Err(anyhow!(
                        "rollout canonical identity differs from native index"
                    ));
                }
                let previous_unchanged = state.unchanged_events;
                let previous_resets = state.counter_resets;
                let had_previous_total = state.previous_total.is_some();
                let proposal = process_line(
                    &mut state,
                    line,
                    &target,
                    &source.machine_id,
                    &source.source_id,
                    &source.file_identity,
                    &attribution,
                    &epochs,
                )?;
                report.records_processed += 1;
                digest.update(line.byte_offset.to_le_bytes());
                digest.update((line.raw.len() as u64).to_le_bytes());
                digest.update(&line.raw);
                let Some(record) = record.filter(|v| {
                    v.get("type").and_then(Value::as_str) == Some("event_msg")
                        && v.pointer("/payload/type").and_then(Value::as_str) == Some("token_count")
                }) else {
                    continue;
                };
                report.token_records += 1;
                let compared =
                    audit::compare_proposal(store, &target, source, line, &record, proposal)?;
                let mut suppression_rule = None;
                if compared.change == "suppressed_candidate" {
                    let rule = if state.foreign_replay {
                        "foreign_history_guard"
                    } else if state.phase == ReconstructionPhase::AwaitingCanonical {
                        "awaiting_canonical"
                    } else if state.phase == ReconstructionPhase::ChildPrefix {
                        "child_prefix_guard"
                    } else if state.unchanged_events > previous_unchanged {
                        "unchanged_counter"
                    } else if !had_previous_total && state.initial_counter_prefix.is_some() {
                        "initial_counter_without_last"
                    } else if state.counter_resets > previous_resets {
                        "counter_reset_without_last"
                    } else {
                        "unemitted_unknown_or_zero"
                    };
                    let suppressed = report.suppressed_by_rule.entry(rule).or_default();
                    suppression_rule = Some(rule);
                    suppressed.records += 1;
                    crate::source_union::add(
                        &mut suppressed.stored_usage,
                        compared
                            .stored
                            .as_ref()
                            .expect("suppressed stored record")
                            .usage,
                    )?;
                }
                sink.record(correction_manifest::ManifestRecord {
                    byte_offset: line.byte_offset,
                    source_json_digest: source_record_digest(&record),
                    change: compared.change.into(),
                    suppression_rule: suppression_rule.map(str::to_owned),
                    stored: compared.stored.clone(),
                    proposed: compared.proposed.clone(),
                })?;
                if compared.stored.is_some() {
                    report.stored_records_seen += 1;
                    match compared.record_key_matches {
                        None => report.source_keys_missing += 1,
                        Some(true) => report.source_keys_matching += 1,
                        Some(false) => report.source_keys_conflicting += 1,
                    }
                }
                if let (Some(old), Some(new)) = (&compared.stored, &compared.proposed)
                    && old.usage != new.usage
                {
                    report.usage_changed_pairs += 1;
                }
                let total = report.comparisons.entry(compared.change).or_default();
                total.records += 1;
                if let Some(old) = compared.stored {
                    total.stored_records += 1;
                    crate::source_union::add(
                        total.stored_usage.get_or_insert_default(),
                        old.usage,
                    )?;
                }
                if let Some(new) = compared.proposed {
                    total.proposed_records += 1;
                    crate::source_union::add(
                        total.proposed_usage.get_or_insert_default(),
                        new.usage,
                    )?;
                }
            }
            if !batch.has_more {
                report.reached_file_end = batch.checkpoint.partial_line.is_empty();
                break;
            }
            checkpoint = batch.checkpoint;
        }
        let after = fs::metadata(&target.path)?;
        report.source_changed_during_read = before.len() != after.len()
            || before.modified()? != after.modified()?
            || identity != physical_file_identity(&target.path, &after)?;
        report.canonical_seen = state.phase != ReconstructionPhase::AwaitingCanonical;
        report.stored_records_not_seen = report
            .stored_scope_records
            .checked_sub(report.stored_records_seen)
            .ok_or_else(|| anyhow!("matched records exceed the stored source scope"))?;
        report.all_stored_positions_seen = report.reached_file_end
            && report.canonical_seen
            && !report.source_changed_during_read
            && report.malformed_records == 0
            && report.stored_records_not_seen == 0;
        report.initial_counter_prefix = state.initial_counter_prefix;
        report.processed_records_digest = hex::encode(digest.finalize());
        sink.finish(correction_manifest::ManifestCompletion {
            token_records: report.token_records,
            stored_records_seen: report.stored_records_seen,
            stored_scope_records: report.stored_scope_records,
            reached_file_end: report.reached_file_end,
            canonical_seen: report.canonical_seen,
            source_changed: report.source_changed_during_read,
            malformed_records: report.malformed_records,
            processed_records_digest: report.processed_records_digest.clone(),
        })?;
        Ok(report)
    })
}
