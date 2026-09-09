//! Bounded, resumable draft generation. Never applies corrections or imports.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    outstanding_isolated_sources: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct AutomaticProgress {
    version: u32,
    db: PathBuf,
    home: PathBuf,
    next_after: Option<String>,
    isolated_threads: BTreeSet<String>,
    #[serde(default)]
    isolated_errors: BTreeMap<String, String>,
    #[serde(default = "all_scope")]
    scope: String,
}

fn all_scope() -> String {
    "all".into()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticRunReport {
    batches_completed: usize,
    sources_processed: usize,
    sources_reconciled: usize,
    sources_isolated: usize,
    outstanding_isolated_sources: usize,
    has_more: bool,
    production_policy_changed: bool,
}

/// Run a bounded job without asking an operator to dispatch each source/page.
#[allow(clippy::too_many_arguments)]
pub fn run_history_reconciliation(
    db: &Path,
    home: &Path,
    output: &Path,
    after: Option<&str>,
    limit: usize,
    max_bytes: usize,
    allow_device_drift: bool,
    batches: usize,
    missing_identities_only: bool,
) -> Result<AutomaticRunReport> {
    if !(1..=1000).contains(&batches) {
        return Err(anyhow!("automatic job requires 1..1000 batches"));
    }
    let mut summary = AutomaticRunReport {
        batches_completed: 0,
        sources_processed: 0,
        sources_reconciled: 0,
        sources_isolated: 0,
        outstanding_isolated_sources: 0,
        has_more: true,
        production_policy_changed: false,
    };
    for index in 0..batches {
        let report = reconcile_history_batch_with_scope(
            db,
            home,
            output,
            if index == 0 { after } else { None },
            limit,
            max_bytes,
            allow_device_drift,
            missing_identities_only,
        )?;
        summary.batches_completed += 1;
        summary.sources_processed += report.items.len();
        summary.sources_reconciled += report
            .items
            .iter()
            .filter(|i| i.status == "reconciled")
            .count();
        summary.sources_isolated = summary.sources_processed - summary.sources_reconciled;
        summary.has_more = report.has_more;
        summary.outstanding_isolated_sources = report.outstanding_isolated_sources.unwrap_or(0);
        tracing::info!(
            batches = summary.batches_completed,
            sources = summary.sources_processed,
            isolated = summary.sources_isolated,
            outstanding = summary.outstanding_isolated_sources,
            "automatic history progress"
        );
        if !summary.has_more {
            break;
        }
    }
    Ok(summary)
}

/// Automatic rule-driven historical reconciliation. Per-source documents are
/// machine receipts, not a human approval loop. Ordinary ledgers stay protected
/// until the separately controlled promotion step.
#[allow(clippy::too_many_arguments)]
pub fn reconcile_history_batch(
    db: &Path,
    home: &Path,
    output: &Path,
    after: Option<&str>,
    limit: usize,
    max_bytes_per_source: usize,
    allow_device_drift: bool,
) -> Result<ReviewBatchReport> {
    reconcile_history_batch_with_scope(
        db,
        home,
        output,
        after,
        limit,
        max_bytes_per_source,
        allow_device_drift,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_history_batch_with_scope(
    db: &Path,
    home: &Path,
    output: &Path,
    after: Option<&str>,
    limit: usize,
    max_bytes_per_source: usize,
    allow_device_drift: bool,
    missing_identities_only: bool,
) -> Result<ReviewBatchReport> {
    if !(1..=10).contains(&limit) || !(1..=1_073_741_824).contains(&max_bytes_per_source) {
        return Err(anyhow!(
            "automatic batch requires 1..10 sources and bounded source bytes"
        ));
    }
    let output = output.canonicalize()?;
    let home = home.canonicalize()?;
    let db = db.canonicalize()?;
    if output.starts_with(&home) || !output.is_dir() {
        return Err(anyhow!("automatic progress must be outside Codex home"));
    }
    let mut lock_options = fs::OpenOptions::new();
    lock_options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        lock_options.mode(0o600);
    }
    let lock = lock_options.open(output.join("automatic-history.lock"))?;
    lock.try_lock()
        .map_err(|error| anyhow!("automatic history job is already running: {error}"))?;
    let progress_path = output.join("automatic-history-progress.json");
    let mut progress = if progress_path.try_exists()? {
        let saved: AutomaticProgress = serde_json::from_slice(&fs::read(&progress_path)?)?;
        if saved.version != 1 || saved.db != db || saved.home != home {
            return Err(anyhow!(
                "automatic progress belongs to another ledger or source home"
            ));
        }
        saved
    } else {
        AutomaticProgress {
            version: 1,
            db: db.clone(),
            home: home.clone(),
            next_after: None,
            isolated_threads: BTreeSet::new(),
            isolated_errors: BTreeMap::new(),
            scope: all_scope(),
        }
    };
    let scope = if missing_identities_only {
        "missing_identities"
    } else {
        "all"
    };
    if progress.scope != scope {
        progress.next_after = None;
        progress.scope = scope.into();
    }
    let after = after
        .map(str::to_owned)
        .or_else(|| progress.next_after.clone());
    let (db, home) = (db.as_path(), home.as_path());
    let reader = LedgerStore::open_reconstruction_audit(db)?;
    let marker: i64 = reader
        .connection()
        .pragma_query_value(None, "application_id", |r| r.get(0))?;
    if marker != 0x43554c53 {
        return Err(anyhow!(
            "automatic historical reconciliation requires an isolated shadow"
        ));
    }
    let selected = if missing_identities_only {
        Some(
            reader
                .missing_identity_threads()?
                .difference(&progress.isolated_threads)
                .cloned()
                .collect::<BTreeSet<_>>(),
        )
    } else {
        None
    };
    drop(reader);
    // Upgrade only the explicitly selected shadow, never the production store.
    drop(LedgerStore::open(db)?);
    let mut report = draft_selected_batch(
        db,
        home,
        &output,
        after.as_deref(),
        limit,
        max_bytes_per_source,
        allow_device_drift,
        selected.as_ref(),
    )?;
    for item in &mut report.items {
        if !matches!(item.status, "created" | "reused" | "applied") {
            continue;
        }
        let Some(seal) = item.seal.as_deref() else {
            continue;
        };
        let result = (|| -> Result<()> {
            crate::store::apply_shadow_correction_with_policy(db, &item.manifest, seal, true)?;
            crate::store::link_shadow_sampling(
                db,
                home,
                &item.manifest,
                seal,
                crate::sampling::LegacySamplingAuditOptions {
                    start: DateTime::<Utc>::UNIX_EPOCH,
                    end: Utc::now(),
                    limit: 100_000,
                    max_bytes: max_bytes_per_source.min(1_073_741_824) as u64,
                    include_links: true,
                },
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => item.status = "reconciled",
            Err(error) => {
                item.status = "isolated_error";
                item.error = Some(error.to_string());
            }
        }
    }
    for item in &report.items {
        if item.status == "reconciled" {
            progress.isolated_threads.remove(&item.thread);
            progress.isolated_errors.remove(&item.thread);
        } else {
            progress.isolated_threads.insert(item.thread.clone());
            progress.isolated_errors.insert(
                item.thread.clone(),
                item.error.clone().unwrap_or_else(|| item.status.into()),
            );
        }
    }
    if report.next_after.is_some() {
        progress.next_after.clone_from(&report.next_after);
    }
    let temporary = output.join(format!(".automatic-history-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(&progress)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &progress_path)?;
    report.outstanding_isolated_sources = Some(progress.isolated_threads.len());
    Ok(report)
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
    draft_selected_batch(
        db,
        home,
        output,
        after,
        limit,
        max_bytes_per_source,
        allow_device_drift,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn draft_selected_batch(
    db: &Path,
    home: &Path,
    output: &Path,
    after: Option<&str>,
    limit: usize,
    max_bytes_per_source: usize,
    allow_device_drift: bool,
    selected: Option<&BTreeSet<String>>,
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
        .filter(|s| selected.is_none_or(|selected| selected.contains(s)))
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
        outstanding_isolated_sources: None,
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
