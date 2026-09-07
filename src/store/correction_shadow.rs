//! Explicit, isolated correction application. Never accepts an ordinary ledger.
use super::*;
use anyhow::{Result, anyhow};

const APP_ID: i64 = 0x43554c53;
#[cfg(test)]
#[path = "correction_shadow_tests.rs"]
mod tests;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowReceipt {
    pub manifest_sha256: String,
    pub status: &'static str,
    pub archived_records: u64,
    pub corrected_records: u64,
    pub suppressed_records: u64,
    production_policy_changed: bool,
}

/// Reserve a new private output; VACUUM INTO takes a consistent source snapshot.
pub fn create_review_shadow(source: &Path, output: &Path) -> Result<()> {
    let reader = LedgerStore::open_read_only(source)?;
    if reader
        .connection
        .pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))?
        != 0
    {
        return Err(anyhow!("source is not an ordinary ledger baseline"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(output)?);
    // The main handle is still SQLITE_OPEN_READ_ONLY, even while creating the
    // separate output. Query-only would also prohibit writing that output.
    reader.connection.pragma_update(None, "query_only", "OFF")?;
    reader.connection.execute(
        "VACUUM main INTO ?1",
        [output
            .to_str()
            .ok_or_else(|| anyhow!("output path is not UTF-8"))?],
    )?;
    let copy = Connection::open_with_flags(output, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    copy.pragma_update(None, "trusted_schema", "OFF")?;
    let tx = rusqlite::Transaction::new_unchecked(&copy, TransactionBehavior::Immediate)?;
    // A clone of an already-marked shadow is intentionally refused. Receipts
    // must not silently be interpreted as a new original baseline.
    let marker: i64 = tx.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if marker != 0
        || tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?
            != migrations::CURRENT_SCHEMA_VERSION
    {
        return Err(anyhow!("source is not an ordinary ledger baseline"));
    }
    tx.execute_batch("CREATE TABLE review_shadow_meta(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL);
        INSERT INTO review_shadow_meta VALUES(1,1);
        CREATE TABLE review_correction_receipts(manifest_sha256 TEXT PRIMARY KEY,archived_records INTEGER NOT NULL,corrected_records INTEGER NOT NULL,suppressed_records INTEGER NOT NULL,applied_at TEXT NOT NULL,after_sha256 TEXT NOT NULL);
        CREATE TABLE review_old_reconstruction AS SELECT '' AS manifest_sha256,r.* FROM reconstruction_usage_events r WHERE 0;
        CREATE UNIQUE INDEX review_old_reconstruction_key ON review_old_reconstruction(manifest_sha256,event_id);
        CREATE TABLE review_old_record_keys(manifest_sha256 TEXT NOT NULL,evidence_source TEXT NOT NULL,event_id TEXT NOT NULL,record_key TEXT NOT NULL,PRIMARY KEY(manifest_sha256,evidence_source,event_id));")?;
    tx.pragma_update(None, "application_id", APP_ID)?;
    tx.commit()?;
    Ok(())
}

pub fn apply_shadow_correction(
    shadow: &Path,
    manifest: &Path,
    expected_sha: &str,
) -> Result<ShadowReceipt> {
    // Inspect before any writable/migrating opener is used.
    let reader = LedgerStore::open_read_only(shadow)?;
    let marker: i64 = reader
        .connection
        .pragma_query_value(None, "application_id", |r| r.get(0))?;
    if marker != APP_ID {
        return Err(anyhow!(
            "correction requires a generated review shadow, not an ordinary ledger"
        ));
    }
    let version: i64 = reader.connection.query_row(
        "SELECT version FROM review_shadow_meta WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    if version != 1 {
        return Err(anyhow!("unsupported shadow version"));
    }
    let sealed = crate::reconstruction::verify_correction_manifest(manifest)?;
    if sealed.body_sha256 != expected_sha || !sealed.full_source_scan {
        return Err(anyhow!("expected complete manifest seal does not match"));
    }
    drop(reader);
    // Do not use the normal migrating opener, even after the read-only check.
    let connection =
        Connection::open_with_flags(shadow, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "trusted_schema", "OFF")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let store = LedgerStore {
        connection,
        exact_series_memo: Default::default(),
        union_main_preview: false,
    };
    let tx =
        rusqlite::Transaction::new_unchecked(&store.connection, TransactionBehavior::Immediate)?;
    if tx.pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))? != APP_ID
        || store.schema_version()? != migrations::CURRENT_SCHEMA_VERSION
    {
        return Err(anyhow!("shadow identity/schema changed before application"));
    }
    if let Some((archived,corrected,suppressed,after_hash))=tx.query_row("SELECT archived_records,corrected_records,suppressed_records,after_sha256 FROM review_correction_receipts WHERE manifest_sha256=?1",[expected_sha],|r|Ok((u64_from_sql(r.get(0)?,0)?,u64_from_sql(r.get(1)?,1)?,u64_from_sql(r.get(2)?,2)?,r.get::<_,String>(3)?))).optional()? {
        if scope_digest(&tx,expected_sha)?!=after_hash {return Err(anyhow!("previously corrected rows changed; receipt needs review"));}
        return Ok(ShadowReceipt{manifest_sha256:expected_sha.into(),status:"already_applied",archived_records:archived,corrected_records:corrected,suppressed_records:suppressed,production_policy_changed:false});
    }
    tx.execute_batch(
        "CREATE TEMP TABLE correction_plan(event_id TEXT PRIMARY KEY,proposed_json TEXT);",
    )?;
    let checked = crate::reconstruction::visit_correction_records(manifest, &store, |row| {
        if row.stored.is_none() && row.proposed.is_some() {
            return Err(anyhow!(
                "new historical facts require a separate insertion review"
            ));
        }
        if let Some(old) = &row.stored {
            if let Some(new) = &row.proposed
                && (old.thread != new.thread
                    || old.account != new.account
                    || old.project != new.project)
            {
                return Err(anyhow!(
                    "account/project/thread reassignment requires a separate review"
                ));
            }
            tx.execute(
                "INSERT INTO correction_plan VALUES(?1,?2)",
                params![
                    old.event_id,
                    row.proposed
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?
                ],
            )?;
        }
        Ok(())
    })?;
    if checked.body_sha256 != expected_sha || !checked.full_source_scan {
        return Err(anyhow!("manifest changed or is incomplete"));
    }
    tx.execute("INSERT INTO review_old_reconstruction SELECT ?1,r.* FROM reconstruction_usage_events r JOIN correction_plan p USING(event_id)",[expected_sha])?;
    tx.execute("INSERT INTO review_old_record_keys SELECT ?1,k.* FROM source_record_evidence k JOIN correction_plan p USING(event_id) WHERE k.evidence_source='reconstruction'",[expected_sha])?;
    let rows = tx
        .prepare("SELECT event_id,proposed_json FROM correction_plan ORDER BY event_id")?
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut corrected = 0;
    let mut suppressed = 0;
    for (id, proposed) in &rows {
        let Some(json) = proposed else {
            tx.execute(
                "DELETE FROM reconstruction_usage_events WHERE event_id=?1",
                [id],
            )?;
            tx.execute("DELETE FROM source_record_evidence WHERE evidence_source='reconstruction' AND event_id=?1",[id])?;
            suppressed += 1;
            continue;
        };
        let fact: ReconstructionAuditFact = serde_json::from_str(json)?;
        let mut event=tx.query_row(&format!("SELECT {EVENT_SELECT_COLUMNS} FROM (SELECT r.*,'confirmed' AS quality,NULL AS quality_reason,thread_id AS rollout_id FROM reconstruction_usage_events r) WHERE event_id=?1"),[id],row_to_event)?;
        let epoch: u64 = tx.query_row(
            "SELECT counter_epoch FROM reconstruction_usage_events WHERE event_id=?1",
            [id],
            |r| u64_from_sql(r.get(0)?, 0),
        )?;
        event.source_timestamp = Some(fact.at);
        event.model = fact.model;
        event.usage = fact.usage;
        event.provenance.source_record_key = fact.record_key;
        // Reinsert through the normal hashing and source-key persistence path.
        tx.execute(
            "DELETE FROM reconstruction_usage_events WHERE event_id=?1",
            [id],
        )?;
        tx.execute("DELETE FROM source_record_evidence WHERE evidence_source='reconstruction' AND event_id=?1",[id])?;
        upsert_reconstruction_event_in(
            &tx,
            &ReconstructionEvent {
                event,
                counter_epoch: epoch,
            },
        )?;
        corrected += 1;
    }
    // Insert triggers do not cover correction deletes. Rebuild the derived
    // rollups inside the same transaction; raw original rows remain archived.
    rebuild_reconstruction_rollups_in(&tx)?;
    tx.execute(
        "INSERT INTO review_correction_receipts VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            expected_sha,
            rows.len() as i64,
            corrected as i64,
            suppressed as i64,
            timestamp(Utc::now()),
            scope_digest(&tx, expected_sha)?
        ],
    )?;
    tx.execute_batch("DROP TABLE temp.correction_plan;")?;
    tx.commit()?;
    Ok(ShadowReceipt {
        manifest_sha256: expected_sha.into(),
        status: "applied_to_review_shadow",
        archived_records: rows.len() as u64,
        corrected_records: corrected,
        suppressed_records: suppressed,
        production_policy_changed: false,
    })
}

fn scope_digest(connection: &Connection, receipt: &str) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(b"shadow-correction-scope-v1");
    for (tag, sql) in [
        (
            0u8,
            "SELECT r.* FROM reconstruction_usage_events r JOIN review_old_reconstruction old USING(event_id) WHERE old.manifest_sha256=?1 ORDER BY r.event_id",
        ),
        (
            1u8,
            "SELECT k.* FROM source_record_evidence k JOIN review_old_reconstruction old USING(event_id) WHERE old.manifest_sha256=?1 AND k.evidence_source='reconstruction' ORDER BY k.event_id",
        ),
    ] {
        digest.update([tag]);
        let mut statement = connection.prepare(sql)?;
        let columns = statement.column_count();
        let mut rows = statement.query([receipt])?;
        while let Some(row) = rows.next()? {
            digest.update((columns as u64).to_be_bytes());
            for index in 0..columns {
                use rusqlite::types::ValueRef;
                match row.get_ref(index)? {
                    ValueRef::Null => digest.update([0]),
                    ValueRef::Integer(value) => {
                        digest.update([1]);
                        digest.update(value.to_be_bytes());
                    }
                    ValueRef::Real(value) => {
                        digest.update([2]);
                        digest.update(value.to_bits().to_be_bytes());
                    }
                    ValueRef::Text(value) | ValueRef::Blob(value) => {
                        digest.update([if matches!(row.get_ref(index)?, ValueRef::Text(_)) {
                            3
                        } else {
                            4
                        }]);
                        digest.update((value.len() as u64).to_be_bytes());
                        digest.update(value);
                    }
                }
            }
        }
    }
    Ok(hex::encode(digest.finalize()))
}
