//! Build a separate migration candidate; never overwrite the installed ledger.
use super::*;
use anyhow::{Result, anyhow, ensure};
use std::{fs, io::Read};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReport {
    pub source_rows: u64,
    pub matched_rows: u64,
    pub replacement_rows: u64,
    pub replay_rows: u64,
    pub preserved_rows: u64,
    pub sampling_keys_added: u64,
    pub output_rows: u64,
    pub source_sha256: String,
    pub review_sha256: String,
    pub production_changed: bool,
}

fn fingerprint(path: &Path) -> Result<String> {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    match fs::metadata(PathBuf::from(wal)) {
        Ok(meta) => ensure!(
            meta.len() == 0,
            "checkpoint input WAL before candidate preparation"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finalize()))
}

fn readonly_uri(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| anyhow!("database path must be UTF-8"))?;
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    let mut uri = String::from("file:");
    for byte in text.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/:-._~".contains(byte) {
            uri.push(*byte as char);
        } else {
            use std::fmt::Write;
            write!(&mut uri, "%{byte:02X}")?;
        }
    }
    uri.push_str("?mode=ro");
    Ok(uri)
}

fn count(connection: &Connection, sql: &str) -> Result<u64> {
    let value: i64 = connection.query_row(sql, [], |r| r.get(0))?;
    Ok(value.try_into()?)
}

pub fn prepare_review_transfer(
    source: &Path,
    review: &Path,
    output: &Path,
    expected_source: &str,
    expected_review: &str,
) -> Result<TransferReport> {
    let source = source.canonicalize()?;
    let review = review.canonicalize()?;
    ensure!(source != review, "source and review must differ");
    let source_hash = fingerprint(&source)?;
    let review_hash = fingerprint(&review)?;
    ensure!(
        source_hash.eq_ignore_ascii_case(expected_source)
            && review_hash.eq_ignore_ascii_case(expected_review),
        "input fingerprint changed"
    );
    let reviewed = LedgerStore::open_read_only(&review)?;
    ensure!(
        reviewed
            .connection
            .pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))?
            == 0x43554c53,
        "review input must be a generated shadow"
    );
    ensure!(
        reviewed.source_union_projection_progress()?.pending_groups == 0,
        "finish review projection before transfer"
    );
    drop(reviewed);
    let reader = LedgerStore::open_reconstruction_audit(&source)?;
    ensure!(
        reader
            .connection
            .pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))?
            == 0,
        "source must be an ordinary ledger"
    );
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(output)?);
    reader.connection.pragma_update(None, "query_only", "OFF")?;
    reader.connection.execute(
        "VACUUM main INTO ?1",
        [output
            .to_str()
            .ok_or_else(|| anyhow!("output must be UTF-8"))?],
    )?;
    drop(reader);
    let mut candidate = LedgerStore::open(output)?;
    while !candidate.request_evidence_backfill_complete()? {
        candidate.backfill_request_evidence_chunk(1000)?;
    }
    candidate
        .connection
        .execute("ATTACH DATABASE ?1 AS reviewed", [readonly_uri(&review)?])?;
    let columns: Vec<String> = candidate
        .connection
        .prepare("PRAGMA main.table_info(reconstruction_usage_events)")?
        .query_map([], |r| r.get(1))?
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        !columns.is_empty()
            && columns
                .iter()
                .all(|c| c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')),
        "unsupported fact columns"
    );
    let names = columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(",");
    let equality = columns
        .iter()
        .map(|c| format!("f.\"{c}\" IS o.\"{c}\""))
        .collect::<Vec<_>>()
        .join(" AND ");
    let tx = candidate.connection.unchecked_transaction()?;
    let before = count(&tx, "SELECT COUNT(*) FROM main.reconstruction_usage_events")?;
    tx.execute_batch(&format!("CREATE TEMP TABLE transfer_old AS SELECT DISTINCT {names} FROM reviewed.review_old_reconstruction;
        CREATE INDEX temp.transfer_old_lookup ON transfer_old(event_id,event_hash);
        CREATE TEMP TABLE transfer_allowed_keys AS SELECT evidence_source,event_id,record_key FROM reviewed.review_old_record_keys
            UNION SELECT evidence_source,event_id,record_key FROM reviewed.source_record_evidence;
        CREATE INDEX temp.transfer_allowed_lookup ON transfer_allowed_keys(evidence_source,event_id,record_key);
        CREATE TEMP TABLE transfer_ids(event_id TEXT PRIMARY KEY);
        INSERT INTO transfer_ids SELECT DISTINCT f.event_id FROM main.reconstruction_usage_events f
            JOIN transfer_old o ON f.event_id=o.event_id AND f.event_hash=o.event_hash WHERE {equality}
            AND NOT EXISTS(SELECT 1 FROM main.source_record_evidence p WHERE p.evidence_source='reconstruction' AND p.event_id=f.event_id
                AND NOT EXISTS(SELECT 1 FROM transfer_allowed_keys a WHERE a.evidence_source=p.evidence_source AND a.event_id=p.event_id AND a.record_key=p.record_key));"))?;
    let matched = count(&tx, "SELECT COUNT(*) FROM transfer_ids")?;
    let replacements = count(
        &tx,
        "SELECT COUNT(*) FROM reviewed.reconstruction_usage_events r JOIN transfer_ids i USING(event_id)",
    )?;
    ensure!(count(&tx,"SELECT COUNT(*) FROM reviewed.reconstruction_usage_events r JOIN transfer_ids i USING(event_id)
        WHERE total_tokens<>input_tokens+output_tokens OR cached_input_tokens>input_tokens
        OR cache_write_input_tokens>input_tokens-cached_input_tokens OR reasoning_output_tokens>output_tokens
        OR cache_write_observed_input_tokens>input_tokens")? == 0, "reviewed replacement violates Token invariants");
    tx.execute_batch("CREATE TABLE transfer_original_reconstruction AS SELECT f.* FROM main.reconstruction_usage_events f JOIN transfer_ids i USING(event_id);
        CREATE TABLE transfer_original_keys AS SELECT p.* FROM main.source_record_evidence p JOIN transfer_ids i USING(event_id) WHERE evidence_source='reconstruction';
        DELETE FROM main.source_record_evidence WHERE evidence_source='reconstruction' AND event_id IN (SELECT event_id FROM transfer_ids);
        DELETE FROM main.reconstruction_usage_events WHERE event_id IN (SELECT event_id FROM transfer_ids);")?;
    tx.execute_batch(&format!("INSERT INTO main.reconstruction_usage_events({names}) SELECT {} FROM reviewed.reconstruction_usage_events r JOIN transfer_ids i USING(event_id);",
        columns.iter().map(|c|format!("r.\"{c}\"")).collect::<Vec<_>>().join(",")))?;
    tx.execute_batch("INSERT INTO main.source_record_evidence SELECT p.* FROM reviewed.source_record_evidence p
        JOIN transfer_ids i USING(event_id) JOIN main.reconstruction_usage_events r USING(event_id) WHERE p.evidence_source='reconstruction';")?;
    let sampling_keys = tx.execute("INSERT OR IGNORE INTO main.source_record_evidence
        SELECT p.* FROM reviewed.source_record_evidence p JOIN reviewed.retained_request_evidence r ON p.event_id=r.event_id
        JOIN main.retained_request_evidence f ON f.event_id=r.event_id AND f.event_hash=r.event_hash
        WHERE p.evidence_source='sampling' AND f.quality=r.quality AND f.thread_id IS r.thread_id
        AND f.effective_at=r.effective_at AND f.model IS r.model AND f.input_tokens=r.input_tokens
        AND f.cached_input_tokens=r.cached_input_tokens AND f.output_tokens=r.output_tokens
        AND f.reasoning_output_tokens=r.reasoning_output_tokens AND f.total_tokens=r.total_tokens
        AND f.cache_write_input_tokens=r.cache_write_input_tokens
        AND f.cache_write_observed_input_tokens=r.cache_write_observed_input_tokens", [])? as u64;
    let after = count(&tx, "SELECT COUNT(*) FROM main.reconstruction_usage_events")?;
    ensure!(
        after == before - matched + replacements,
        "candidate row conservation failed"
    );
    let report = TransferReport {
        source_rows: before,
        matched_rows: matched,
        replacement_rows: replacements,
        replay_rows: matched - replacements,
        preserved_rows: before - matched,
        sampling_keys_added: sampling_keys,
        output_rows: after,
        source_sha256: source_hash.clone(),
        review_sha256: review_hash.clone(),
        production_changed: false,
    };
    tx.execute_batch("CREATE TABLE transfer_receipt(id INTEGER PRIMARY KEY CHECK(id=1), report_json TEXT NOT NULL);")?;
    tx.execute(
        "INSERT INTO transfer_receipt VALUES(1,?1)",
        [serde_json::to_string(&report)?],
    )?;
    ensure!(
        fingerprint(&source)? == source_hash && fingerprint(&review)? == review_hash,
        "input changed during candidate build"
    );
    tx.commit()?;
    candidate
        .connection
        .execute_batch("DETACH DATABASE reviewed;")?;
    candidate.checkpoint_wal()?;
    Ok(report)
}

#[cfg(test)]
#[path = "review_transfer_tests.rs"]
mod tests;
