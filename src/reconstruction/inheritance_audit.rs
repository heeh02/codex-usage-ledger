//! Verbatim Token-info prefix evidence across an explicitly declared fork.
use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InheritanceAudit {
    version: u32,
    scope: &'static str,
    child_thread: String,
    declared_parent: String,
    child_records: usize,
    parent_records: usize,
    matched_records: usize,
    zero_write_default_compatible_records: usize,
    zero_write_default_compatible_prefix: bool,
    first_mismatch: Option<usize>,
    first_zero_write_incompatible_record: Option<usize>,
    first_zero_write_incompatible_child_offset: Option<u64>,
    first_zero_write_incompatible_parent_offset: Option<u64>,
    canonical_task_boundary: bool,
    child_first_offset: Option<u64>,
    child_last_offset: Option<u64>,
    parent_first_offset: Option<u64>,
    parent_last_offset: Option<u64>,
    child_bytes_read: usize,
    parent_bytes_read: usize,
    source_changed: bool,
    identical_declared_prefix: bool,
    child_sequence_digest: String,
    parent_sequence_digest: String,
    migration_ready: bool,
    independent_inference_proven: bool,
}

struct Fingerprint {
    offset: u64,
    digest: String,
    zero_write_elided_digest: String,
}

pub fn audit_inherited_prefix(
    home: &Path,
    child: &str,
    max_bytes: usize,
    max_tokens: usize,
) -> Result<InheritanceAudit> {
    if child.is_empty()
        || !(1..=2_147_483_648).contains(&max_bytes)
        || !(1..=1_000_000).contains(&max_tokens)
    {
        return Err(anyhow!(
            "require child, byte limit 1..2147483648 per file and token limit 1..1000000"
        ));
    }
    let target = audit::resolve_target(home, child)?;
    let child_before = fs::metadata(&target.path)?;
    let child_identity = physical_file_identity(&target.path, &child_before)?;
    let mut declared_parent = None;
    let mut canonical_at = None;
    let mut boundary = false;
    let mut first_meta = true;
    let mut child_tokens = Vec::new();
    let (child_bytes, child_changed) = scan(&target.path, max_bytes, |line, record| {
        if first_meta && record.get("type").and_then(Value::as_str) == Some("session_meta") {
            if record.pointer("/payload/id").and_then(Value::as_str) != Some(child) {
                return Err(anyhow!("child canonical metadata does not match index"));
            }
            declared_parent = record
                .pointer("/payload/forked_from_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if declared_parent
                .as_deref()
                .is_none_or(|id| id.is_empty() || id == child)
            {
                return Err(anyhow!("a distinct explicit forked_from_id is required"));
            }
            canonical_at = source_timestamp(record);
            first_meta = false;
        }
        if first_meta {
            return Ok(false);
        }
        if record.get("type").and_then(Value::as_str) == Some("event_msg")
            && record.pointer("/payload/type").and_then(Value::as_str) == Some("task_started")
            && crate::replay::task_belongs_to_canonical_stream(record, child, canonical_at)
        {
            boundary = true;
            return Ok(true);
        }
        if let Some((digest, zero_write_elided_digest)) = fingerprint(record)? {
            if child_tokens.len() == max_tokens {
                return Err(anyhow!("inherited prefix exceeds token budget"));
            }
            child_tokens.push(Fingerprint {
                offset: line.byte_offset,
                digest,
                zero_write_elided_digest,
            });
        }
        Ok(false)
    })?;
    let parent =
        declared_parent.ok_or_else(|| anyhow!("missing declared parent in examined prefix"))?;
    let parent_target = audit::resolve_target(home, &parent)?;
    if parent_target.path == target.path {
        return Err(anyhow!("parent and child resolve to one file"));
    }
    let mut parent_tokens = Vec::new();
    let mut parent_meta_seen = false;
    let (parent_bytes, parent_changed) = scan(&parent_target.path, max_bytes, |line, record| {
        if !parent_meta_seen && record.get("type").and_then(Value::as_str) == Some("session_meta") {
            if record.pointer("/payload/id").and_then(Value::as_str) != Some(parent.as_str()) {
                return Err(anyhow!(
                    "parent canonical metadata does not match declaration"
                ));
            }
            parent_meta_seen = true;
            if child_tokens.is_empty() {
                return Ok(true);
            }
        }
        if parent_meta_seen && let Some((digest, zero_write_elided_digest)) = fingerprint(record)? {
            parent_tokens.push(Fingerprint {
                offset: line.byte_offset,
                digest,
                zero_write_elided_digest,
            });
            return Ok(parent_tokens.len() == child_tokens.len());
        }
        Ok(false)
    })?;
    let mut first_mismatch = None;
    let mut matched = 0;
    let mut compatible = 0;
    let mut incompatible = None;
    for (i, (a, b)) in child_tokens.iter().zip(&parent_tokens).enumerate() {
        compatible += usize::from(a.zero_write_elided_digest == b.zero_write_elided_digest);
        if a.zero_write_elided_digest != b.zero_write_elided_digest {
            incompatible.get_or_insert(i);
        }
        if a.digest == b.digest {
            matched += 1;
        } else {
            first_mismatch.get_or_insert(i);
        }
    }
    if parent_tokens.len() != child_tokens.len() {
        first_mismatch.get_or_insert(parent_tokens.len());
        incompatible.get_or_insert(parent_tokens.len());
    }
    let child_after = fs::metadata(&target.path)?;
    let changed = child_changed
        || parent_changed
        || child_before.len() != child_after.len()
        || child_before.modified()? != child_after.modified()?
        || child_identity != physical_file_identity(&target.path, &child_after)?;
    Ok(InheritanceAudit {
        version: 1,
        scope: "declared_fork_token_info_prefix_not_inference_usage",
        child_thread: child.into(),
        declared_parent: parent,
        child_records: child_tokens.len(),
        parent_records: parent_tokens.len(),
        matched_records: matched,
        zero_write_default_compatible_records: compatible,
        zero_write_default_compatible_prefix: boundary
            && parent_meta_seen
            && !changed
            && !child_tokens.is_empty()
            && parent_tokens.len() == child_tokens.len()
            && compatible == child_tokens.len(),
        first_mismatch,
        first_zero_write_incompatible_record: incompatible,
        first_zero_write_incompatible_child_offset: incompatible
            .and_then(|i| child_tokens.get(i))
            .map(|r| r.offset),
        first_zero_write_incompatible_parent_offset: incompatible
            .and_then(|i| parent_tokens.get(i))
            .map(|r| r.offset),
        canonical_task_boundary: boundary,
        child_first_offset: child_tokens.first().map(|r| r.offset),
        child_last_offset: child_tokens.last().map(|r| r.offset),
        parent_first_offset: parent_tokens.first().map(|r| r.offset),
        parent_last_offset: parent_tokens.last().map(|r| r.offset),
        child_bytes_read: child_bytes,
        parent_bytes_read: parent_bytes,
        source_changed: changed,
        identical_declared_prefix: boundary
            && parent_meta_seen
            && !changed
            && !child_tokens.is_empty()
            && first_mismatch.is_none(),
        child_sequence_digest: sequence_digest(&child_tokens),
        parent_sequence_digest: sequence_digest(&parent_tokens),
        migration_ready: false,
        independent_inference_proven: false,
    })
}

fn fingerprint(record: &Value) -> Result<Option<(String, String)>> {
    if record.get("type").and_then(Value::as_str) != Some("event_msg")
        || record.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
    {
        return Ok(None);
    }
    // Missing or partial token payloads are not strong fingerprints. Do not
    // silently discard them and compare a different, apparently matching stream.
    let info = record
        .pointer("/payload/info")
        .filter(|value| value.is_object())
        .ok_or_else(|| anyhow!("token info missing; exact prefix comparison unavailable"))?;
    if !info.get("total_token_usage").is_some_and(Value::is_object) {
        return Err(anyhow!(
            "token total payload missing; exact prefix comparison unavailable"
        ));
    }
    let mut elided = info.clone();
    for usage in ["total_token_usage", "last_token_usage"] {
        if let Some(usage) = elided.get_mut(usage).and_then(Value::as_object_mut) {
            for key in [
                "cache_write_input_tokens",
                "cache_write_tokens",
                "input_cache_write_tokens",
            ] {
                if usage.get(key).and_then(Value::as_u64) == Some(0) {
                    usage.remove(key);
                }
            }
        }
    }
    // Compatibility diagnostic ONLY: absence is still not an observed zero.
    // The strict digest and identical-prefix decision retain field presence.
    Ok(Some((
        source_record_digest(info),
        source_record_digest(&elided),
    )))
}

fn sequence_digest(rows: &[Fingerprint]) -> String {
    let mut digest = Sha256::new();
    for row in rows {
        digest.update(row.digest.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn scan(
    path: &Path,
    max_bytes: usize,
    mut visit: impl FnMut(&JsonlLine, &Value) -> Result<bool>,
) -> Result<(usize, bool)> {
    let before = fs::metadata(path)?;
    let identity = physical_file_identity(path, &before)?;
    let mut checkpoint = TailCheckpoint::default();
    let mut bytes = 0;
    'scan: while bytes < max_bytes {
        let mut tail = IncrementalJsonlTailer::with_limits(
            checkpoint,
            TailLimits {
                read_chunk_bytes: (max_bytes - bytes).min(4 * 1024 * 1024),
                max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            },
        )?;
        let batch = tail.poll_path(path)?;
        if batch.reset.is_some() {
            return Err(anyhow!("source reset during inherited-prefix audit"));
        }
        bytes += batch.bytes_read;
        for line in &batch.lines {
            if line.is_blank() {
                continue;
            }
            let record = line.parse_json()?;
            if visit(line, &record)? {
                break 'scan;
            }
        }
        if !batch.has_more {
            break;
        }
        checkpoint = batch.checkpoint;
    }
    let after = fs::metadata(path)?;
    Ok((
        bytes,
        before.len() != after.len()
            || before.modified()? != after.modified()?
            || identity != physical_file_identity(path, &after)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn meta(id: &str, parent: Option<&str>) -> Value {
        serde_json::json!({"type":"session_meta","timestamp":"2026-01-02T00:00:00Z",
            "payload":{"id":id,"forked_from_id":parent}})
    }
    fn token(total: u64) -> Value {
        serde_json::json!({"type":"event_msg","timestamp":"2026-01-01T00:00:00Z",
            "payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":total,"total_tokens":total},
            "last_token_usage":{"input_tokens":100,"total_tokens":100}}}})
    }
    fn write(path: &Path, rows: &[Value]) {
        let mut file = fs::File::create(path).unwrap();
        for row in rows {
            writeln!(file, "{row}").unwrap();
        }
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let sessions = temp.path().join("sessions");
        fs::create_dir(&sessions).unwrap();
        let parent = sessions.join("parent.jsonl");
        let child = sessions.join("child.jsonl");
        write(
            &parent,
            &[meta("parent", None), token(100), token(200), token(300)],
        );
        let mut a = token(100);
        a["timestamp"] = serde_json::json!("2026-01-02T00:00:00Z");
        let mut b = token(200);
        b["timestamp"] = a["timestamp"].clone();
        let started = "2026-01-02T00:00:01Z"
            .parse::<DateTime<Utc>>()
            .unwrap()
            .timestamp();
        write(
            &child,
            &[
                meta("child", Some("parent")),
                meta("parent", None),
                a,
                serde_json::json!({"type":"event_msg","payload":{"type":"task_started","started_at":1}}),
                b,
                serde_json::json!({"type":"event_msg","payload":{"type":"task_started","started_at":started}}),
                token(300),
            ],
        );
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        for (id, path) in [("parent", &parent), ("child", &child)] {
            index
                .execute(
                    "INSERT INTO threads VALUES(?1,?2,NULL,'model','{}')",
                    rusqlite::params![id, path.to_str().unwrap()],
                )
                .unwrap();
        }
        (temp, parent, child)
    }
    #[test]
    fn declared_ordered_prefix_matches_despite_outer_timestamp_rewrite() {
        let (temp, parent, child) = fixture();
        let before = [
            fs::read(&parent).unwrap(),
            fs::read(&child).unwrap(),
            fs::read(temp.path().join("state_5.sqlite")).unwrap(),
        ];
        let report = audit_inherited_prefix(temp.path(), "child", 1024 * 1024, 100).unwrap();
        assert!(report.identical_declared_prefix && report.canonical_task_boundary);
        assert_eq!(
            (
                report.child_records,
                report.parent_records,
                report.matched_records
            ),
            (2, 2, 2)
        );
        assert_eq!(report.child_sequence_digest, report.parent_sequence_digest);
        assert!(!report.migration_ready && !report.independent_inference_proven);
        assert_eq!(
            before,
            [
                fs::read(parent).unwrap(),
                fs::read(child).unwrap(),
                fs::read(temp.path().join("state_5.sqlite")).unwrap()
            ]
        );
    }
    #[test]
    fn mismatch_short_parent_and_limits_never_become_a_verified_prefix() {
        let (temp, parent, child) = fixture();
        let mut different = token(200);
        different["payload"]["info"]["last_token_usage"]["input_tokens"] = serde_json::json!(99);
        write(&parent, &[meta("parent", None), token(100), different]);
        let report = audit_inherited_prefix(temp.path(), "child", 1048576, 100).unwrap();
        assert!(!report.identical_declared_prefix);
        assert_eq!(report.first_mismatch, Some(1));
        write(&parent, &[meta("parent", None), token(100)]);
        let report = audit_inherited_prefix(temp.path(), "child", 1048576, 100).unwrap();
        assert!(!report.identical_declared_prefix);
        assert_eq!(report.parent_records, 1);
        assert!(audit_inherited_prefix(temp.path(), "child", 1048576, 1).is_err());
        assert!(audit_inherited_prefix(temp.path(), "child", 1, 100).is_err());
        write(&child, &[meta("child", Some("parent")), token(100)]);
        let report = audit_inherited_prefix(temp.path(), "child", 1048576, 100).unwrap();
        assert!(!report.identical_declared_prefix && !report.canonical_task_boundary);
        write(&child, &[meta("child", None), token(100)]);
        assert!(audit_inherited_prefix(temp.path(), "child", 1048576, 100).is_err());
    }
    #[test]
    fn source_append_is_visible_and_incomplete_payloads_are_not_silently_skipped() {
        let (temp, _, child) = fixture();
        let (_, changed) = scan(&child, 1048576, |_, _| {
            fs::OpenOptions::new()
                .append(true)
                .open(&child)?
                .write_all(b"\n")?;
            Ok(true)
        })
        .unwrap();
        assert!(changed);
        write(
            &child,
            &[
                meta("child", Some("parent")),
                serde_json::json!({"type":"event_msg","payload":{"type":"token_count","info":null}}),
            ],
        );
        assert!(audit_inherited_prefix(temp.path(), "child", 1048576, 100).is_err());
    }

    #[test]
    fn added_zero_cache_write_fields_are_compatible_but_not_exact_evidence() {
        let (temp, parent, _) = fixture();
        let mut a = token(100);
        let mut b = token(200);
        for row in [&mut a, &mut b] {
            for key in ["total_token_usage", "last_token_usage"] {
                row["payload"]["info"][key]["cache_write_input_tokens"] = serde_json::json!(0);
            }
        }
        write(&parent, &[meta("parent", None), a.clone(), b.clone()]);
        let report = audit_inherited_prefix(temp.path(), "child", 1048576, 100).unwrap();
        assert_eq!(report.matched_records, 0);
        assert!(report.zero_write_default_compatible_prefix);
        assert!(!report.identical_declared_prefix);
        b["payload"]["info"]["total_token_usage"]["cache_write_input_tokens"] =
            serde_json::json!(1);
        write(&parent, &[meta("parent", None), a, b]);
        assert!(
            !audit_inherited_prefix(temp.path(), "child", 1048576, 100)
                .unwrap()
                .zero_write_default_compatible_prefix
        );
    }
}
