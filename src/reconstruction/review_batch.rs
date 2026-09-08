//! Bounded, resumable draft generation. Never applies corrections or imports.
use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchItem {
    thread: String,
    manifest: PathBuf,
    status: &'static str,
    seal: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchReport {
    version: u32,
    items: Vec<ReviewBatchItem>,
    next_after: Option<String>,
    has_more: bool,
    source_import: bool,
    production_policy_changed: bool,
}

/// Existing complete drafts are verified against the ledger before reuse; no
/// source rescan or file replacement occurs. Failed/partial drafts stay intact.
pub fn draft_reconstruction_batch(
    db: &Path,
    home: &Path,
    output: &Path,
    after: Option<&str>,
    limit: usize,
    max_bytes_per_source: usize,
    allow_device_drift: bool,
) -> Result<ReviewBatchReport> {
    if !(1..=10).contains(&limit) || !(1..=2_147_483_648).contains(&max_bytes_per_source) {
        return Err(anyhow!(
            "review batch requires 1..10 sources and bounded source bytes"
        ));
    }
    // Require an existing private output directory: no default location inside
    // the source home and no implicit real-ledger creation/migration.
    let output = output.canonicalize()?;
    if output.starts_with(home.canonicalize()?) || !output.is_dir() {
        return Err(anyhow!(
            "review output must be a directory outside Codex home"
        ));
    }
    let store = LedgerStore::open_reconstruction_audit(db)?;
    let threads: BTreeSet<_> = store
        .reconstruction_sources()?
        .into_iter()
        .map(|s| s.thread_id)
        .filter(|s| after.is_none_or(|after| s.as_str() > after))
        .collect();
    drop(store);
    let has_more = threads.len() > limit;
    let mut items = Vec::new();
    for thread in threads.into_iter().take(limit) {
        let name = hex::encode(Sha256::digest(thread.as_bytes()));
        let manifest = output.join(format!("{name}.jsonl"));
        // symlink_metadata catches dangling links as well; never follow one.
        let metadata = fs::symlink_metadata(&manifest);
        let (status, verification) = match metadata {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => (
                "review_required",
                Err(anyhow!("existing draft is not a regular file")),
            ),
            Ok(_) => match reuse(db, &manifest) {
                Ok((status, value)) => (status, Ok(value)),
                Err(error) => ("review_required", Err(error)),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
                "created",
                correction_manifest::write_correction_manifest(
                    db,
                    home,
                    &thread,
                    max_bytes_per_source,
                    1_000_000,
                    allow_device_drift,
                    &manifest,
                ),
            ),
            Err(error) => ("review_required", Err(error.into())),
        };
        let (status, seal, error) = match verification {
            Ok(value)
                if value.binding.thread != thread
                    || value.binding.policy
                        != correction_manifest::CURRENT_RECONSTRUCTION_POLICY =>
            {
                (
                    "review_required",
                    None,
                    Some("draft target or parser policy does not match batch".into()),
                )
            }
            Ok(value) if value.full_source_scan => (status, Some(value.body_sha256), None),
            Ok(value) => ("incomplete", Some(value.body_sha256), None),
            Err(error) => ("review_required", None, Some(error.to_string())),
        };
        items.push(ReviewBatchItem {
            thread,
            manifest,
            status,
            seal,
            error,
        });
    }
    Ok(ReviewBatchReport {
        version: 1,
        next_after: items.last().map(|i| i.thread.clone()),
        items,
        has_more,
        source_import: false,
        production_policy_changed: false,
    })
}

fn reuse(
    db: &Path,
    manifest: &Path,
) -> Result<(&'static str, correction_manifest::ManifestVerification)> {
    let sealed = correction_manifest::verify_correction_manifest(manifest)?;
    if LedgerStore::open_reconstruction_audit(db)?.applied_review_is_current(&sealed.body_sha256)? {
        Ok(("applied", sealed))
    } else {
        Ok((
            "reused",
            correction_manifest::verify_correction_against_ledger(manifest, db)?,
        ))
    }
}
