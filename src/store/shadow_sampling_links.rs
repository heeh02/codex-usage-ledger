//! Attach verified retained anchors to already corrected historical occurrences.
use super::*;
use crate::{
    sampling::{LegacySamplingAuditOptions, POST_SAMPLING_SOURCE_ID, audit_legacy_sampling},
    source_union::{EvidenceSide, same_token_amounts},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingLinkReceipt {
    manifest_sha256: String,
    linked: u64,
    unchanged: u64,
    metadata_conflicts: u64,
    skipped: BTreeMap<&'static str, u64>,
    token_facts_changed: bool,
    production_policy_changed: bool,
}

pub fn link_shadow_sampling(
    shadow: &Path,
    home: &Path,
    manifest: &Path,
    expected_sha: &str,
    mut options: LegacySamplingAuditOptions,
) -> Result<SamplingLinkReceipt> {
    let sealed = crate::reconstruction::verify_correction_manifest(manifest)?;
    if sealed.body_sha256 != expected_sha || !sealed.full_source_scan {
        return Err(anyhow!("complete correction seal does not match"));
    }
    let reader = LedgerStore::open_read_only(shadow)?;
    check_shadow(&reader.connection, expected_sha)?;
    options.include_links = true;
    let audit = audit_legacy_sampling(shadow, home, &sealed.binding.thread, options)?;
    if audit.source_identity != sealed.binding.observed_file_identity
        || audit.source_changed && !audit.source_extended
    {
        return Err(anyhow!(
            "rollout identity/non-append state changed; review required"
        ));
    }
    drop(reader);
    let connection =
        Connection::open_with_flags(shadow, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "trusted_schema", "OFF")?;
    let tx = rusqlite::Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
    check_shadow(&tx, expected_sha)?;
    // Canonical legacy IDs have no machine namespace. Require a unique saved
    // primary-source binding rather than inferring it from amounts or accounts.
    let machines = tx
        .prepare("SELECT DISTINCT machine_id FROM file_cursors WHERE source_id=?1")?
        .query_map([POST_SAMPLING_SOURCE_ID], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if machines != [sealed.binding.machine_id.clone()] {
        return Err(anyhow!(
            "legacy primary sampling machine binding is missing or ambiguous"
        ));
    }
    let version: i64 = tx.query_row(
        "SELECT version FROM review_shadow_meta WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    if version == 1 {
        tx.execute_batch("CREATE TABLE review_sampling_links(correction_sha256 TEXT NOT NULL,event_id TEXT NOT NULL,record_key TEXT NOT NULL,previous_key TEXT,retained_hash TEXT NOT NULL,linked_at TEXT NOT NULL,PRIMARY KEY(correction_sha256,event_id)); UPDATE review_shadow_meta SET version=2 WHERE id=1;")?;
    }
    let mut receipt = SamplingLinkReceipt {
        manifest_sha256: expected_sha.into(),
        linked: 0,
        unchanged: 0,
        metadata_conflicts: 0,
        skipped: BTreeMap::new(),
        token_facts_changed: false,
        production_policy_changed: false,
    };
    for link in audit.links.unwrap_or_default() {
        if link.status != "amounts_match" {
            *receipt.skipped.entry(link.status).or_default() += 1;
            continue;
        }
        let (Some(offset), Some(digest)) = (link.byte_offset, link.record_digest) else {
            return Err(anyhow!("association lacks source position"));
        };
        let id = crate::reconstruction::stable_event_id(
            &sealed.binding.machine_id,
            &sealed.binding.stored_file_identity,
            &sealed.binding.thread,
            offset,
        );
        let covered:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM review_old_reconstruction WHERE manifest_sha256=?1 AND event_id=?2)",params![expected_sha,id],|r|r.get(0))?;
        if !covered {
            *receipt
                .skipped
                .entry("outside_corrected_scope")
                .or_default() += 1;
            continue;
        }
        let sample =
            union_repository::load_measurement(&tx, EvidenceSide::Sampling, &link.event_id)?;
        if sample.at != link.at
            || sample.thread != sealed.binding.thread
            || sample.usage != Some(link.stored_usage)
        {
            return Err(anyhow!("retained anchor changed during requalification"));
        }
        let target = union_repository::load_measurement(&tx, EvidenceSide::Reconstruction, &id)?;
        let key = crate::reconstruction::source_record_key(
            &sealed.binding.machine_id,
            &sealed.binding.stored_file_identity,
            &sealed.binding.thread,
            offset,
            &digest,
        );
        if target.record_key.as_deref() != Some(&key)
            || !target
                .usage
                .is_some_and(|usage| same_token_amounts(usage, link.stored_usage))
            || (target.at - link.at)
                .num_nanoseconds()
                .is_none_or(|n| n.unsigned_abs() > 250_000_000)
        {
            return Err(anyhow!(
                "corrected occurrence digest/amount/time does not match"
            ));
        }
        if sample.record_key.as_deref().is_some_and(|old| old != key) {
            return Err(anyhow!(
                "existing sampling source key conflicts; not overwritten"
            ));
        }
        receipt.metadata_conflicts += u64::from(
            sample.model != target.model
                || sample.account != target.account
                || sample.project != target.project
                || !sample.assignment_available,
        );
        let hash: String = tx.query_row(
            "SELECT event_hash FROM retained_request_evidence WHERE event_id=?1",
            [&link.event_id],
            |r| r.get(0),
        )?;
        let previous:Option<(String,String)>=tx.query_row("SELECT record_key,retained_hash FROM review_sampling_links WHERE correction_sha256=?1 AND event_id=?2",params![expected_sha,link.event_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((old_key, old_hash)) = previous {
            if old_key != key || old_hash != hash || sample.record_key.as_deref() != Some(&key) {
                return Err(anyhow!("sampling link receipt no longer matches"));
            }
            receipt.unchanged += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO review_sampling_links VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                expected_sha,
                link.event_id,
                key,
                sample.record_key,
                hash,
                timestamp(Utc::now())
            ],
        )?;
        let count=tx.execute("INSERT INTO source_record_evidence VALUES('sampling',?1,?2) ON CONFLICT(evidence_source,event_id) DO NOTHING",params![link.event_id,key])?;
        if count == 0 {
            receipt.unchanged += 1;
        } else {
            receipt.linked += 1;
        }
    }
    tx.commit()?;
    Ok(receipt)
}

fn check_shadow(connection: &Connection, sha: &str) -> Result<()> {
    if connection.pragma_query_value(None, "application_id", |r| r.get::<_, i64>(0))? != APP_ID
        || connection.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?
            != migrations::CURRENT_SCHEMA_VERSION
    {
        return Err(anyhow!(
            "sampling links require a current generated review shadow"
        ));
    }
    let version: i64 = connection.query_row(
        "SELECT version FROM review_shadow_meta WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    if !(1..=2).contains(&version) {
        return Err(anyhow!("unsupported review shadow version"));
    }
    let expected: String = connection.query_row(
        "SELECT after_sha256 FROM review_correction_receipts WHERE manifest_sha256=?1",
        [sha],
        |r| r.get(0),
    )?;
    if scope_digest(connection, sha)? != expected {
        return Err(anyhow!("corrected reconstruction post-image changed"));
    }
    Ok(())
}
