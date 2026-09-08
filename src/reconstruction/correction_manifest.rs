//! Sealed, review-only JSONL drafts. No apply operation or ledger writes.
use super::*;
pub(super) const CURRENT_RECONSTRUCTION_POLICY: &str = "reconstruction_shared_stream_v2";
use crate::store::ReconstructionAuditFact;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, BufWriter, Read, Write},
};

const MAX_LINE: usize = 128 * 1024;
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ManifestHeader {
    pub version: u32,
    pub policy: String,
    pub ledger_schema: i64,
    pub machine_id: String,
    pub source_id: String,
    pub thread: String,
    pub stored_file_identity: String,
    pub observed_file_identity: String,
    pub file_bytes_at_start: u64,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_prefix_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_prefix_sha256: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManifestRecord {
    pub byte_offset: u64,
    pub source_json_digest: String,
    pub change: String,
    pub suppression_rule: Option<String>,
    pub stored: Option<ReconstructionAuditFact>,
    pub proposed: Option<ReconstructionAuditFact>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ManifestCompletion {
    pub token_records: u64,
    pub stored_records_seen: u64,
    pub stored_scope_records: u64,
    pub reached_file_end: bool,
    pub canonical_seen: bool,
    pub source_changed: bool,
    pub malformed_records: u64,
    pub processed_records_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_prefix_revalidated: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Entry {
    Header(ManifestHeader),
    Record(Box<ManifestRecord>),
    Completion(ManifestCompletion),
    Seal { sha256: String, records: u64 },
}

pub(super) trait EvidenceSink {
    fn begin(&mut self, header: ManifestHeader) -> Result<()>;
    fn record(&mut self, record: ManifestRecord) -> Result<()>;
    fn finish(&mut self, completion: ManifestCompletion) -> Result<()>;
}
pub(super) struct NoopSink;
impl EvidenceSink for NoopSink {
    fn begin(&mut self, _: ManifestHeader) -> Result<()> {
        Ok(())
    }
    fn record(&mut self, _: ManifestRecord) -> Result<()> {
        Ok(())
    }
    fn finish(&mut self, _: ManifestCompletion) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCategory {
    records: u64,
    stored_usage: Option<TokenUsage>,
    proposed_usage: Option<TokenUsage>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVerification {
    #[serde(skip)]
    pub(crate) binding: ManifestBinding,
    scope: &'static str,
    pub(crate) records: u64,
    pub(crate) body_sha256: String,
    pub(crate) full_source_scan: bool,
    draft_only: bool,
    migration_authorized: bool,
    source_revalidated: bool,
    ledger_rows_revalidated: bool,
    categories: BTreeMap<String, ManifestCategory>,
}

#[derive(Debug)]
pub(crate) struct ManifestBinding {
    pub policy: String,
    pub machine_id: String,
    pub thread: String,
    pub stored_file_identity: String,
    pub observed_file_identity: String,
}

pub fn write_correction_manifest(
    db: &Path,
    home: &Path,
    thread: &str,
    max_bytes: usize,
    max_tokens: usize,
    allow_device_drift: bool,
    output: &Path,
) -> Result<ManifestVerification> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    if parent.starts_with(home.canonicalize()?) {
        return Err(anyhow!(
            "draft output must be outside the Codex source home"
        ));
    }
    let output = parent.join(
        output
            .file_name()
            .ok_or_else(|| anyhow!("draft output needs a filename"))?,
    );
    let mut sink = DraftWriter {
        path: output.clone(),
        writer: None,
        digest: Sha256::new(),
        records: 0,
        bytes: 0,
    };
    file_audit::run_file_audit(
        db,
        home,
        thread,
        max_bytes,
        max_tokens,
        allow_device_drift,
        &mut sink,
    )?;
    drop(sink);
    verify_correction_manifest(&output)
}

struct DraftWriter {
    path: PathBuf,
    writer: Option<BufWriter<fs::File>>,
    digest: Sha256,
    records: u64,
    bytes: u64,
}
impl DraftWriter {
    fn emit(&mut self, entry: Entry, hash: bool) -> Result<()> {
        let mut bytes = serde_json::to_vec(&entry)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_LINE || self.bytes + bytes.len() as u64 > MAX_FILE {
            return Err(anyhow!("draft manifest exceeds output budget"));
        }
        self.writer
            .as_mut()
            .ok_or_else(|| anyhow!("draft not opened"))?
            .write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        if hash {
            self.digest.update(&bytes);
        }
        Ok(())
    }
}
impl EvidenceSink for DraftWriter {
    fn begin(&mut self, header: ManifestHeader) -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        self.writer = Some(BufWriter::new(options.open(&self.path)?));
        self.emit(Entry::Header(header), true)
    }
    fn record(&mut self, record: ManifestRecord) -> Result<()> {
        self.emit(Entry::Record(Box::new(record)), true)?;
        self.records += 1;
        Ok(())
    }
    fn finish(&mut self, completion: ManifestCompletion) -> Result<()> {
        self.emit(Entry::Completion(completion), true)?;
        self.emit(
            Entry::Seal {
                sha256: hex::encode(self.digest.clone().finalize()),
                records: self.records,
            },
            false,
        )?;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| anyhow!("draft not opened"))?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        Ok(())
    }
}

pub fn verify_correction_manifest(path: &Path) -> Result<ManifestVerification> {
    verify_with_store(path, None, None)
}

pub fn verify_correction_against_ledger(path: &Path, db: &Path) -> Result<ManifestVerification> {
    let store = LedgerStore::open_reconstruction_audit(db)?;
    store.with_source_audit_snapshot(|store| verify_with_store(path, Some(store), None))
}

/// Callbacks run before the final seal. Callers must stage transactionally and
/// commit only after this returns successfully and required coverage is checked.
pub(crate) fn visit_correction_records(
    path: &Path,
    store: &LedgerStore,
    mut visit: impl FnMut(&ManifestRecord) -> Result<()>,
) -> Result<ManifestVerification> {
    verify_with_store(path, Some(store), Some(&mut visit))
}

type RecordVisitor<'a> = &'a mut dyn FnMut(&ManifestRecord) -> Result<()>;
fn verify_with_store(
    path: &Path,
    store: Option<&LedgerStore>,
    mut visitor: Option<RecordVisitor<'_>>,
) -> Result<ManifestVerification> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut digest = Sha256::new();
    let mut header = None;
    let mut completion = None;
    let mut sealed = None;
    let mut last_offset = None;
    let mut records = 0;
    let mut stored_count = 0;
    let mut total_bytes = 0_u64;
    let mut categories = BTreeMap::<String, ManifestCategory>::new();
    let mut ledger_source_count = None;
    loop {
        let mut line = Vec::new();
        let read = (&mut reader)
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        total_bytes += read as u64;
        if read > MAX_LINE
            || total_bytes > MAX_FILE
            || line.last() != Some(&b'\n')
            || sealed.is_some()
        {
            return Err(anyhow!(
                "manifest truncated, oversized or contains data after its seal"
            ));
        }
        let entry: Entry = serde_json::from_slice(&line)?;
        if let Entry::Seal {
            sha256,
            records: declared,
        } = entry
        {
            if completion.is_none()
                || declared != records
                || sha256 != hex::encode(digest.clone().finalize())
            {
                return Err(anyhow!("manifest seal/count does not match"));
            }
            sealed = Some(sha256);
            continue;
        }
        digest.update(&line);
        match entry {
            Entry::Header(value) if header.is_none() && records == 0 => {
                let valid_capture = match value.version {
                    1 => {
                        value.captured_prefix_bytes.is_none()
                            && value.captured_prefix_sha256.is_none()
                    }
                    2 => {
                        value
                            .captured_prefix_bytes
                            .is_some_and(|n| n <= value.file_bytes_at_start)
                            && value
                                .captured_prefix_sha256
                                .as_deref()
                                .is_some_and(valid_digest)
                    }
                    _ => false,
                };
                if !valid_capture
                    || !matches!(
                        value.policy.as_str(),
                        "reconstruction_uuid7_strict_v1" | CURRENT_RECONSTRUCTION_POLICY
                    )
                    || !(35..=41).contains(&value.ledger_schema)
                    || value.machine_id.is_empty()
                    || value.thread.is_empty()
                    || value.source_id != source_id(&value.thread)
                    || value.stored_file_identity.is_empty()
                    || (value.stored_file_identity != value.observed_file_identity
                        && !audit::device_only_drift(
                            &value.stored_file_identity,
                            &value.observed_file_identity,
                        ))
                {
                    return Err(anyhow!("unsupported manifest source/policy"));
                }
                if let Some(store) = store {
                    let sources = store.reconstruction_sources()?;
                    let source = sources
                        .iter()
                        .find(|source| {
                            source.machine_id == value.machine_id
                                && source.source_id == value.source_id
                                && source.thread_id == value.thread
                                && source.file_identity == value.stored_file_identity
                        })
                        .ok_or_else(|| anyhow!("draft source binding no longer matches ledger"))?;
                    ledger_source_count = Some(store.reconstruction_audit_count(source)?);
                }
                header = Some(value);
            }
            Entry::Record(row) if header.is_some() && completion.is_none() => {
                let header = header.as_ref().unwrap();
                if last_offset.is_some_and(|last| last >= row.byte_offset)
                    || !valid_digest(&row.source_json_digest)
                {
                    return Err(anyhow!("invalid or duplicate source position/digest"));
                }
                validate_record(header, &row)?;
                if let Some(store) = store {
                    let id = stable_event_id(
                        &header.machine_id,
                        &header.stored_file_identity,
                        &header.thread,
                        row.byte_offset,
                    );
                    if store.reconstruction_audit_fact(&id)? != row.stored {
                        return Err(anyhow!(
                            "draft old fact or expected absence no longer matches ledger at byte {}",
                            row.byte_offset
                        ));
                    }
                }
                if let Some(visit) = visitor.as_mut() {
                    visit(&row)?;
                }
                last_offset = Some(row.byte_offset);
                records += 1;
                if records > 10_000_000 {
                    return Err(anyhow!("too many manifest records"));
                }
                let category = categories.entry(row.change).or_default();
                category.records += 1;
                if let Some(old) = row.stored {
                    stored_count += 1;
                    crate::source_union::add(
                        category.stored_usage.get_or_insert_default(),
                        old.usage,
                    )?;
                }
                if let Some(new) = row.proposed {
                    crate::source_union::add(
                        category.proposed_usage.get_or_insert_default(),
                        new.usage,
                    )?;
                }
            }
            Entry::Completion(value) if header.is_some() && completion.is_none() => {
                if value.token_records != records
                    || value.stored_records_seen != stored_count
                    || value.stored_scope_records < stored_count
                    || !valid_digest(&value.processed_records_digest)
                    || ledger_source_count.is_some_and(|count| count != value.stored_scope_records)
                {
                    return Err(anyhow!("manifest coverage counts disagree"));
                }
                completion = Some(value);
            }
            _ => return Err(anyhow!("invalid manifest entry order")),
        }
    }
    let body_sha256 = sealed.ok_or_else(|| anyhow!("manifest has no completion seal"))?;
    let completion = completion.ok_or_else(|| anyhow!("manifest has no completion"))?;
    let header = header.ok_or_else(|| anyhow!("manifest missing header"))?;
    if header.version == 1 && completion.captured_prefix_revalidated.is_some()
        || header.version == 2 && completion.captured_prefix_revalidated.is_none()
    {
        return Err(anyhow!("manifest capture proof/version mismatch"));
    }
    let source_stable = if header.version == 2 {
        completion.captured_prefix_revalidated == Some(true)
    } else {
        !completion.source_changed
    };
    Ok(ManifestVerification {
        binding: ManifestBinding {
            policy: header.policy,
            machine_id: header.machine_id,
            thread: header.thread,
            stored_file_identity: header.stored_file_identity,
            observed_file_identity: header.observed_file_identity,
        },
        scope: "sealed_reconstruction_correction_draft",
        records,
        body_sha256,
        full_source_scan: completion.reached_file_end
            && completion.canonical_seen
            && source_stable
            && completion.malformed_records == 0
            && stored_count == completion.stored_scope_records,
        draft_only: true,
        migration_authorized: false,
        source_revalidated: false,
        ledger_rows_revalidated: store.is_some(),
        categories,
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn validate_record(header: &ManifestHeader, row: &ManifestRecord) -> Result<()> {
    if header
        .captured_prefix_bytes
        .is_some_and(|end| row.byte_offset >= end)
    {
        return Err(anyhow!("manifest record outside captured prefix"));
    }
    let expected = stable_event_id(
        &header.machine_id,
        &header.stored_file_identity,
        &header.thread,
        row.byte_offset,
    );
    let key = source_record_key(
        &header.machine_id,
        &header.stored_file_identity,
        &header.thread,
        row.byte_offset,
        &row.source_json_digest,
    );
    for (stored, fact) in [(true, &row.stored), (false, &row.proposed)] {
        if let Some(fact) = fact {
            fact.usage.validate()?;
            if fact.event_id != expected
                || fact.thread.as_deref() != Some(header.thread.as_str())
                || (stored && !fact.stored_hash.as_deref().is_some_and(valid_digest))
                || (!stored
                    && (fact.stored_hash.is_some()
                        || fact.record_key.as_deref() != Some(key.as_str())))
            {
                return Err(anyhow!("manifest fact identity/usage is inconsistent"));
            }
        }
    }
    let change = match (&row.stored, &row.proposed) {
        (None, None) => "not_emitted",
        (None, Some(_)) => "new_candidate",
        (Some(_), None) => "suppressed_candidate",
        (Some(a), Some(b))
            if a.usage == b.usage
                && a.at == b.at
                && a.thread == b.thread
                && a.model == b.model
                && a.account == b.account
                && a.project == b.project =>
        {
            "unchanged"
        }
        _ => "changed_candidate",
    };
    let rule_valid = if change == "suppressed_candidate" {
        row.suppression_rule.as_deref().is_some_and(|rule| {
            [
                "foreign_history_guard",
                "awaiting_canonical",
                "child_prefix_guard",
                "unchanged_counter",
                "initial_counter_without_last",
                "counter_reset_without_last",
                "unemitted_unknown_or_zero",
            ]
            .contains(&rule)
                || (header.policy == CURRENT_RECONSTRUCTION_POLICY
                    && [
                        "invalid_or_missing_cumulative_usage",
                        "missing_usage_timestamp",
                        "counter_continuity_gap",
                    ]
                    .contains(&rule))
        })
    } else {
        row.suppression_rule.is_none()
    };
    if row.change != change || !rule_valid {
        return Err(anyhow!("manifest action/reason contradicts its facts"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("draft.jsonl");
        let mut sink = DraftWriter {
            path: path.clone(),
            writer: None,
            digest: Sha256::new(),
            records: 0,
            bytes: 0,
        };
        let header = ManifestHeader {
            version: 1,
            policy: "reconstruction_uuid7_strict_v1".into(),
            ledger_schema: 38,
            machine_id: "machine".into(),
            source_id: source_id("thread"),
            thread: "thread".into(),
            stored_file_identity: "file".into(),
            observed_file_identity: "file".into(),
            file_bytes_at_start: 1000,
            created_at: Utc::now(),
            captured_prefix_bytes: None,
            captured_prefix_sha256: None,
        };
        sink.begin(header).unwrap();
        let fact = ReconstructionAuditFact {
            event_id: stable_event_id("machine", "file", "thread", 100),
            stored_hash: Some("a".repeat(64)),
            at: Utc::now(),
            thread: Some("thread".into()),
            model: Some("model".into()),
            account: None,
            project: None,
            record_key: None,
            usage: TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 40,
                cache_write_input_tokens: 10,
                cache_write_observed_input_tokens: 100,
                output_tokens: 20,
                reasoning_output_tokens: 5,
                total_tokens: 120,
            },
        };
        sink.record(ManifestRecord {
            byte_offset: 100,
            source_json_digest: "b".repeat(64),
            change: "suppressed_candidate".into(),
            suppression_rule: Some("foreign_history_guard".into()),
            stored: Some(fact),
            proposed: None,
        })
        .unwrap();
        sink.finish(ManifestCompletion {
            token_records: 1,
            stored_records_seen: 1,
            stored_scope_records: 1,
            reached_file_end: true,
            canonical_seen: true,
            source_changed: false,
            malformed_records: 0,
            processed_records_digest: "c".repeat(64),
            captured_prefix_revalidated: None,
        })
        .unwrap();
        drop(sink);
        (temp, path)
    }
    fn rows(path: &Path) -> Vec<Entry> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn reseal(path: &Path, rows: &[Entry]) {
        let mut bytes = Vec::new();
        let mut digest = Sha256::new();
        let mut records = 0;
        for row in rows {
            if matches!(row, Entry::Seal { .. }) {
                continue;
            }
            if matches!(row, Entry::Record(_)) {
                records += 1;
            }
            let mut line = serde_json::to_vec(row).unwrap();
            line.push(b'\n');
            digest.update(&line);
            bytes.extend(line);
        }
        bytes.extend(
            serde_json::to_vec(&Entry::Seal {
                sha256: hex::encode(digest.finalize()),
                records,
            })
            .unwrap(),
        );
        bytes.push(b'\n');
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn captured_manifests_require_versioned_bounded_revalidation() {
        let (_temp, path) = fixture();
        let mut entries = rows(&path);
        if let Entry::Header(header) = &mut entries[0] {
            header.version = 2;
            header.captured_prefix_bytes = Some(1000);
            header.captured_prefix_sha256 = Some("d".repeat(64));
        }
        if let Entry::Completion(completion) = &mut entries[2] {
            completion.source_changed = true;
            completion.captured_prefix_revalidated = Some(true);
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).unwrap().full_source_scan);
        if let Entry::Completion(completion) = &mut entries[2] {
            completion.captured_prefix_revalidated = Some(false);
        }
        reseal(&path, &entries);
        assert!(!verify_correction_manifest(&path).unwrap().full_source_scan);
        if let Entry::Header(header) = &mut entries[0] {
            header.captured_prefix_bytes = Some(100);
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).is_err());
        if let Entry::Header(header) = &mut entries[0] {
            header.version = 1;
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).is_err());
    }
    #[test]
    fn gap_reasons_require_the_new_parser_policy_without_rewriting_old_drafts() {
        let (_temp, path) = fixture();
        assert!(verify_correction_manifest(&path).is_ok());
        let mut entries = rows(&path);
        for entry in &mut entries {
            if let Entry::Record(row) = entry {
                row.suppression_rule = Some("counter_continuity_gap".into());
            }
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).is_err());
        if let Entry::Header(header) = &mut entries[0] {
            header.policy = CURRENT_RECONSTRUCTION_POLICY.into();
        }
        reseal(&path, &entries);
        let report = verify_correction_manifest(&path).unwrap();
        assert!(!report.migration_authorized);
        if let Entry::Header(header) = &mut entries[0] {
            header.policy = "unknown-future-policy".into();
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).is_err());
    }

    #[test]
    fn intact_draft_conserves_components_without_authorizing_migration() {
        let (_temp, path) = fixture();
        let report = verify_correction_manifest(&path).unwrap();
        assert_eq!(report.records, 1);
        assert!(report.full_source_scan && report.draft_only);
        assert!(!report.migration_authorized && !report.source_revalidated);
        let cat = &report.categories["suppressed_candidate"];
        assert!(cat.proposed_usage.is_none());
        let usage = cat.stored_usage.unwrap();
        assert_eq!(
            (
                usage.input_tokens,
                usage.cached_input_tokens,
                usage.cache_write_input_tokens,
                usage.output_tokens
            ),
            (100, 40, 10, 20)
        );
        assert_eq!(
            (
                usage.cache_write_observed_input_tokens,
                usage.reasoning_output_tokens,
                usage.total_tokens
            ),
            (100, 5, 120)
        );
    }
    #[test]
    fn missing_seal_corruption_trailing_data_and_long_lines_are_rejected() {
        let (_temp, path) = fixture();
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
        assert!(verify_correction_manifest(&path).is_err());
        let text = String::from_utf8(bytes.clone()).unwrap();
        fs::write(
            &path,
            text.lines().take(2).collect::<Vec<_>>().join("\n") + "\n",
        )
        .unwrap();
        assert!(verify_correction_manifest(&path).is_err());
        fs::write(
            &path,
            text.replace("foreign_history_guard", "unchanged_counter"),
        )
        .unwrap();
        assert!(verify_correction_manifest(&path).is_err());
        let mut trailing = bytes;
        trailing.extend_from_slice(b"\n");
        fs::write(&path, trailing).unwrap();
        assert!(verify_correction_manifest(&path).is_err());
        fs::write(&path, vec![b'x'; MAX_LINE + 1]).unwrap();
        assert!(verify_correction_manifest(&path).is_err());
    }
    #[test]
    fn a_new_checksum_cannot_hide_conflicting_fact_identity_action_or_counts() {
        for alteration in ["identity", "action", "count", "usage", "order"] {
            let (_temp, path) = fixture();
            let mut entries = rows(&path);
            match alteration {
                "identity" => {
                    if let Entry::Record(row) = &mut entries[1] {
                        row.stored.as_mut().unwrap().event_id = "wrong-event".into();
                    }
                }
                "action" => {
                    if let Entry::Record(row) = &mut entries[1] {
                        row.change = "unchanged".into();
                    }
                }
                "count" => {
                    if let Entry::Completion(row) = &mut entries[2] {
                        row.token_records += 1;
                    }
                }
                "usage" => {
                    if let Entry::Record(row) = &mut entries[1] {
                        row.stored.as_mut().unwrap().usage.total_tokens += 1;
                    }
                }
                "order" => entries.swap(0, 1),
                _ => unreachable!(),
            }
            reseal(&path, &entries);
            assert!(verify_correction_manifest(&path).is_err(), "{alteration}");
        }
    }
    #[test]
    fn duplicate_positions_and_rehashed_incomplete_coverage_are_distinguished() {
        let (_temp, path) = fixture();
        let mut entries = rows(&path);
        let duplicate: Entry =
            serde_json::from_value(serde_json::to_value(&entries[1]).unwrap()).unwrap();
        entries.insert(2, duplicate);
        if let Entry::Completion(value) = &mut entries[3] {
            value.token_records = 2;
            value.stored_records_seen = 2;
            value.stored_scope_records = 2;
        }
        reseal(&path, &entries);
        assert!(verify_correction_manifest(&path).is_err());
        let (_temp, path) = fixture();
        let mut entries = rows(&path);
        if let Entry::Completion(value) = &mut entries[2] {
            value.source_changed = true;
        }
        reseal(&path, &entries);
        let report = verify_correction_manifest(&path).unwrap();
        assert!(!report.full_source_scan && !report.migration_authorized);
    }
}
