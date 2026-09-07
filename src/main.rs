use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header, uri::Authority},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use clap::{Parser, Subcommand};
use codex_usage_ledger::{
    AttributionConfidence, OfficialUsageScope,
    api::{self, ApiState, UsageQuery},
    cli_support::{
        AccountBinding, AggregateDimension, AggregateFilter, CollectorStatus,
        CorrectionPreviewFilter, CorrectionPreviewGrain, LedgerStore, POST_SAMPLING_SOURCE_ID,
        RetainedRequestCursor, RetainedRequestScope, SourceUnionGrain, SourceUnionQuery,
        audit_inherited_prefix, audit_reconstruction_file, audit_reconstruction_prefix,
        compact_expired_raw_events, compare_preview_sampling, create_correction_preview,
        discover_rollouts, ingest_post_sampling, ingest_quota_tails, ingest_reconstruction_batch,
        ingest_reconstruction_batch_for_project, load_or_create_hmac_key,
        load_or_create_machine_id, observe_auth, prepare_fast_ledger, prepare_store,
        read_correction_preview, sync_account_history, sync_native_catalog,
        verify_correction_against_ledger, verify_correction_manifest, write_correction_manifest,
    },
};
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(name = "codex-usage-ledger", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Requalify persisted sampling anchors against a complete rollout prefix; never writes.
    AuditLegacySampling {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long)]
        start: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        end: chrono::DateTime<chrono::Utc>,
        #[arg(long, default_value_t = 100000)]
        limit: usize,
        #[arg(long, default_value_t = 1073741824)]
        max_bytes: u64,
        #[arg(long)]
        include_links: bool,
    },
    /// Read the actual dashboard bundle through the resolved union, without promoting policy.
    PreviewUnionBundle {
        #[arg(long)]
        db: PathBuf,
        /// UsageQuery JSON, with the same account/project/session/time filters as the app.
        #[arg(long, default_value = "{}")]
        query: String,
    },
    /// Read all retained quota observation intervals by stable seek pages; never migrates or writes.
    QuotaHistory {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        account: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t=20, value_parser=clap::value_parser!(u16).range(1..=100))]
        limit: u16,
    },
    /// Read-only paired raw/retained hash audit. Does not authorize repair.
    AuditRetainedHashes {
        #[arg(long)]
        db: PathBuf,
        #[arg(long, default_value_t = 0)]
        after_rowid: i64,
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },
    /// Compare a bounded retained rollout prefix with existing reconstructed facts; never writes.
    AuditReconstruction {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long, default_value_t = 4194304)]
        max_bytes: usize,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        /// Diagnostic only: compare under the old namespace when only Unix device differs.
        #[arg(long)]
        allow_device_drift: bool,
    },
    /// Stream one source file for comparison; never migrates or updates the ledger.
    AuditReconstructionFile {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long, default_value_t = 1_073_741_824)]
        max_bytes: usize,
        #[arg(long, default_value_t = 1_000_000)]
        max_token_rows: usize,
        #[arg(long)]
        allow_device_drift: bool,
    },
    /// Compare an explicit fork's inherited Token-info prefix with its indexed parent.
    AuditInheritedPrefix {
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long, default_value_t = 1_073_741_824)]
        max_bytes: usize,
        #[arg(long, default_value_t = 1_000_000)]
        max_tokens: usize,
    },
    /// Export a new private review-only JSONL draft; never applies corrections.
    DraftReconstructionCorrection {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1_073_741_824)]
        max_bytes: usize,
        #[arg(long, default_value_t = 1_000_000)]
        max_token_rows: usize,
        #[arg(long)]
        allow_device_drift: bool,
    },
    /// Validate a draft's structure/checksum; not source validation or approval.
    VerifyReconstructionCorrection {
        #[arg(long)]
        manifest: PathBuf,
        /// Also compare expected old records with one read-only ledger snapshot.
        #[arg(long)]
        against_db: Option<PathBuf>,
    },
    /// Build a separate review-only SQLite preview from a revalidated full draft.
    CreateCorrectionPreview {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        against_db: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Read old/candidate dimensions from a preview; never opens the source ledger.
    ReadCorrectionPreview {
        #[arg(long)]
        preview: PathBuf,
        #[arg(long)]
        start: Option<chrono::DateTime<chrono::Utc>>,
        #[arg(long)]
        end: Option<chrono::DateTime<chrono::Utc>>,
        #[arg(long, default_value = "Asia/Shanghai")]
        timezone: String,
        #[arg(long, default_value = "day")]
        grain: CorrectionPreviewGrain,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thread: Option<String>,
    },
    /// Compare sampling with a candidate preview without deduplicating by proximity.
    CompareCorrectionSources {
        #[arg(long)]
        preview: PathBuf,
        #[arg(long)]
        against_db: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long)]
        start: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        end: chrono::DateTime<chrono::Utc>,
        #[arg(long, default_value_t = 10000)]
        limit: usize,
        #[arg(long)]
        include_rows: bool,
    },
    /// Resume retained request detail backfill in an existing current-schema ledger.
    /// Writes derived details, but never imports sources or changes token rollups.
    BackfillRequests {
        #[arg(long)]
        db: PathBuf,
        #[arg(long, default_value_t=1, value_parser=clap::value_parser!(u16).range(1..=100))]
        batches: u16,
    },
    /// Shadow local source-record union; never changes the active accounting policy.
    ShadowUnion {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long)]
        start: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        end: chrono::DateTime<chrono::Utc>,
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },
    /// Inspect the staged request-level union; does not switch production policy.
    UnionProjection {
        #[arg(long)]
        db: PathBuf,
        /// Explicitly write bounded candidate batches in an existing current-schema ledger.
        #[arg(long)]
        advance: bool,
        #[arg(long, requires="advance", value_parser=clap::value_parser!(u16).range(1..=100))]
        batches: Option<u16>,
    },
    /// Query staged union dimensions in one read snapshot. Never changes active policy.
    ReadUnionProjection {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        start: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        end: chrono::DateTime<chrono::Utc>,
        #[arg(long, default_value = "Asia/Shanghai")]
        timezone: String,
        #[arg(long, default_value = "day")]
        grain: SourceUnionGrain,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thread: Option<String>,
    },
    /// Read one retained-request candidate audit page. Never migrates or imports.
    AuditOverlap {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        thread: String,
        #[arg(long)]
        start: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        end: chrono::DateTime<chrono::Utc>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value_t=100, value_parser=clap::value_parser!(u16).range(1..=500))]
        limit: u16,
        #[arg(long, requires = "after_id")]
        after_time: Option<String>,
        #[arg(long, requires = "after_time")]
        after_id: Option<String>,
    },
    /// Continuously ingest changes and serve the loopback dashboard.
    Daemon {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1:47127")]
        listen: SocketAddr,
        #[arg(long)]
        web_root: Option<PathBuf>,
        #[arg(long, default_value_t = 5)]
        reconcile_seconds: u64,
    },
    /// Serve an existing ledger without collecting new events.
    Serve {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1:47127")]
        listen: SocketAddr,
        #[arg(long)]
        web_root: Option<PathBuf>,
    },
    /// Print one filtered replay-safe snapshot as JSON.
    Summary {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value = "lifetime")]
        period: String,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value = "Asia/Shanghai")]
        timezone: String,
    },
    /// Run bounded local consistency checks without changing Codex data.
    Doctor {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
    },
    /// Verify daily aggregates, remove expired raw details, and optionally
    /// reclaim the released SQLite pages from disk.
    OptimizeStorage {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        vacuum: bool,
    },
    /// Import only sampling calls independently confirmed by logs_2 and a
    /// same-thread last_token_usage event within 250 milliseconds.
    ImportPostSampling {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        vacuum: bool,
    },
    /// Advance a bounded number of replay-safe rollout reconstruction slices.
    ReconstructRollouts {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long, default_value_t = 8)]
        max_files: usize,
        #[arg(long, default_value_t = 1)]
        batches: usize,
        #[arg(long)]
        project: Option<String>,
    },
    /// Fetch the signed-in account's official Codex usage profile once.
    SyncOfficialUsage {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codex_usage_ledger=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::AuditRetainedHashes {
            db,
            after_rowid,
            limit,
        } => {
            let store = LedgerStore::open_read_only(db)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&store.audit_retained_hashes(after_rowid, limit)?)?
            );
        }
        Command::AuditReconstruction {
            db,
            codex_home,
            thread,
            max_bytes,
            limit,
            allow_device_drift,
        } => {
            let store = LedgerStore::open_read_only(db)?;
            let report = audit_reconstruction_prefix(
                &store,
                &codex_home,
                &thread,
                max_bytes,
                limit,
                allow_device_drift,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditLegacySampling {
            db,
            codex_home,
            thread,
            start,
            end,
            limit,
            max_bytes,
            include_links,
        } => {
            let report = codex_usage_ledger::cli_support::audit_legacy_sampling(
                &db,
                &codex_home,
                &thread,
                codex_usage_ledger::cli_support::LegacySamplingAuditOptions {
                    start,
                    end,
                    limit,
                    max_bytes,
                    include_links,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::PreviewUnionBundle { db, query } => {
            anyhow::ensure!(query.len() <= 8192, "query is too large");
            let query: UsageQuery = serde_json::from_str(&query)?;
            let store = LedgerStore::open_source_union_main_preview(db)?;
            let bundle = ApiState::with_store(store).bundle_json(query).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "policy":"request_union_preview", "productionPolicyChanged":false,
                    "historyComplete":false, "bundle":bundle
                }))?
            );
        }
        Command::AuditReconstructionFile {
            db,
            codex_home,
            thread,
            max_bytes,
            max_token_rows,
            allow_device_drift,
        } => {
            let report = audit_reconstruction_file(
                &db,
                &codex_home,
                &thread,
                max_bytes,
                max_token_rows,
                allow_device_drift,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditInheritedPrefix {
            codex_home,
            thread,
            max_bytes,
            max_tokens,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&audit_inherited_prefix(
                    &codex_home,
                    &thread,
                    max_bytes,
                    max_tokens
                )?)?
            );
        }
        Command::DraftReconstructionCorrection {
            db,
            codex_home,
            thread,
            output,
            max_bytes,
            max_token_rows,
            allow_device_drift,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&write_correction_manifest(
                    &db,
                    &codex_home,
                    &thread,
                    max_bytes,
                    max_token_rows,
                    allow_device_drift,
                    &output
                )?)?
            );
        }
        Command::VerifyReconstructionCorrection {
            manifest,
            against_db,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&match against_db {
                    Some(db) => verify_correction_against_ledger(&manifest, &db)?,
                    None => verify_correction_manifest(&manifest)?,
                })?
            );
        }
        Command::CreateCorrectionPreview {
            manifest,
            against_db,
            output,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&create_correction_preview(
                    &manifest,
                    &against_db,
                    &output
                )?)?
            );
        }
        Command::ReadCorrectionPreview {
            preview,
            start,
            end,
            timezone,
            grain,
            account,
            project,
            model,
            thread,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&read_correction_preview(
                    &preview,
                    &CorrectionPreviewFilter {
                        start,
                        end,
                        timezone,
                        grain,
                        account,
                        project,
                        model,
                        thread
                    }
                )?)?
            );
        }
        Command::CompareCorrectionSources {
            preview,
            against_db,
            thread,
            start,
            end,
            limit,
            include_rows,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&compare_preview_sampling(
                    &preview,
                    &against_db,
                    &thread,
                    start,
                    end,
                    limit,
                    include_rows
                )?)?
            );
        }
        Command::QuotaHistory {
            db,
            account,
            cursor,
            limit,
        } => {
            anyhow::ensure!(
                cursor.as_ref().is_none_or(|value| value.len() <= 8192),
                "quota cursor is too large"
            );
            let cursor: Option<codex_usage_ledger::cli_support::QuotaHistoryCursor> =
                cursor.as_deref().map(serde_json::from_str).transpose()?;
            let store = LedgerStore::open_read_only(&db)?;
            let page = store.quota_history_page(&account, cursor.as_ref(), usize::from(limit))?;
            println!("{}", serde_json::to_string_pretty(&page)?);
        }
        Command::BackfillRequests { db, batches } => {
            // Reject missing/old ledgers before opening for writes. Migration
            // acceptance must be a separate explicit step, not a side effect.
            drop(LedgerStore::open_read_only(&db)?);
            let mut store = LedgerStore::open(&db)?;
            let mut complete = store.request_evidence_backfill_complete()?;
            let mut attempted = 0;
            while !complete && attempted < batches {
                complete = store.backfill_request_evidence_chunk(1000)?;
                attempted += 1;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schemaVersion":store.schema_version()?,"batchesAttempted":attempted,
                    "maxRowsPerBatch":1000,"backfillComplete":complete,
                    "historyComplete":false,"sourceImport":false,"rollupRecount":false
                }))?
            );
        }
        Command::ShadowUnion {
            db,
            thread,
            start,
            end,
            limit,
        } => {
            let store = LedgerStore::open_read_only(db)?;
            let report = store.shadow_source_union(&thread, start, end, limit)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::UnionProjection {
            db,
            advance,
            batches,
        } => {
            // Require an existing, current-schema database even for explicit writes.
            // This diagnostic must not create or migrate an installed ledger.
            let reader = LedgerStore::open_read_only(&db)?;
            let mut report = reader.source_union_projection_progress()?;
            if advance {
                drop(reader);
                let mut store = LedgerStore::open(&db)?;
                for _ in 0..batches.unwrap_or(1) {
                    if report.projection_ready {
                        break;
                    }
                    report = store.stage_source_union_batch(200, 200, 10000)?;
                }
            }
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ReadUnionProjection {
            db,
            start,
            end,
            timezone,
            grain,
            account,
            project,
            model,
            thread,
        } => {
            let store = LedgerStore::open_read_only(db)?;
            let report = store.read_source_union_projection(&SourceUnionQuery {
                start,
                end,
                timezone,
                grain,
                account,
                project,
                model,
                thread,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::AuditOverlap {
            db,
            thread,
            start,
            end,
            account,
            model,
            limit,
            after_time,
            after_id,
        } => {
            let store = LedgerStore::open_read_only(db)?;
            let after =
                after_time
                    .zip(after_id)
                    .map(|(effective_at, event_id)| RetainedRequestCursor {
                        effective_at,
                        event_id,
                    });
            let report = store.audit_candidate_page(
                RetainedRequestScope {
                    thread_id: &thread,
                    start,
                    end,
                    account: account.as_deref(),
                    model: model.as_deref(),
                },
                after.as_ref(),
                usize::from(limit),
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Daemon {
            db,
            codex_home,
            listen,
            web_root,
            reconcile_seconds,
        } => {
            ensure_loopback(listen)?;
            let paths = RuntimePaths::resolve(codex_home, db, web_root)?;
            run_daemon(paths, listen, reconcile_seconds).await?;
        }
        Command::Serve {
            db,
            codex_home,
            listen,
            web_root,
        } => {
            ensure_loopback(listen)?;
            let paths = RuntimePaths::resolve(codex_home, db, web_root)?;
            run_dashboard_only(paths, listen).await?;
        }
        Command::Summary {
            db,
            codex_home,
            period,
            account,
            project,
            model,
            timezone,
        } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let mut store = prepare_store(&paths.db)?;
            prepare_fast_ledger(&mut store, "summary")?;
            let snapshot = api::snapshot_from_store(
                &store,
                &UsageQuery {
                    period: Some(period),
                    account,
                    project,
                    model,
                    timezone: Some(timezone),
                    dimension: None,
                    session: None,
                    grain: None,
                    metric: None,
                    ranking_period: None,
                    ranking_sort: None,
                    ..UsageQuery::default()
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        Command::Doctor { db, codex_home } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let store = prepare_store(&paths.db)?;
            let rollout_count = discover_rollouts(&paths.codex_home)?.len();
            let table_counts = store.ledger_table_counts()?;
            let quality = store.aggregate_rollup_by(
                AggregateDimension::Quality,
                &AggregateFilter {
                    quality: None,
                    ..Default::default()
                },
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schemaVersion": store.schema_version()?,
                    "codexHomeReadable": paths.codex_home.is_dir(),
                    "rolloutFilesDiscovered": rollout_count,
                    "cursorCount": table_counts.file_cursors,
                    "rawEventCount": table_counts.raw_events,
                    "compactedEventCount": table_counts.compacted_event_keys,
                    "rollup": store.rollup_progress()?,
                    "quality": quality.into_iter().map(|bucket| json!({
                        "state": bucket.key,
                        "events": bucket.event_count,
                        "tokens": bucket.usage.total_tokens,
                    })).collect::<Vec<_>>(),
                    "authFilePresent": paths.codex_home.join("auth.json").is_file(),
                    "writesCodexAuth": false,
                    "oauthRefresh": false,
                }))?
            );
        }
        Command::OptimizeStorage {
            db,
            codex_home,
            vacuum,
        } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let mut store = prepare_store(&paths.db)?;
            sync_native_catalog(&mut store, &paths.codex_home)?;
            prepare_fast_ledger(&mut store, "optimize-storage")?;
            let deleted = compact_expired_raw_events(&mut store, "optimize-storage")?;
            store.checkpoint_wal()?;
            if vacuum && deleted > 0 {
                store.vacuum()?;
                store.checkpoint_wal()?;
            }
            let table_counts = store.ledger_table_counts()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "deletedRawEvents": deleted,
                    "remainingRawEvents": table_counts.raw_events,
                    "compactedEventKeys": table_counts.compacted_event_keys,
                    "vacuumed": vacuum && deleted > 0,
                    "rollup": store.rollup_progress()?,
                }))?
            );
        }
        Command::ImportPostSampling {
            db,
            codex_home,
            vacuum,
        } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let mut store = prepare_store(&paths.db)?;
            let machine_id = load_or_create_machine_id(&paths.data_dir)?;
            let hmac_key = load_or_create_hmac_key(&paths.data_dir)?;
            let _ = observe_auth(
                &mut store,
                &paths.codex_home.join("auth.json"),
                &hmac_key,
                &machine_id,
            )?;
            sync_account_history(&mut store, &paths.codex_home, &machine_id, &hmac_key)?;
            sync_native_catalog(&mut store, &paths.codex_home)?;
            store.set_collector_status(&CollectorStatus {
                mode: "import-post-sampling".to_owned(),
                phase: "optimizing".to_owned(),
                items_total: 0,
                items_completed: 0,
                bytes_read: 0,
                events_inserted: 0,
                message: Some("首次建立 post-sampling 可信账本".to_owned()),
                updated_at: chrono::Utc::now(),
            })?;
            let report = ingest_post_sampling(&mut store, &paths.codex_home, &machine_id)?;
            store.reproject_usage_from_catalog()?;
            prepare_fast_ledger(&mut store, "import-post-sampling")?;
            let compacted = compact_expired_raw_events(&mut store, "import-post-sampling")?;
            store.checkpoint_wal()?;
            if vacuum && compacted > 0 {
                store.vacuum()?;
                store.checkpoint_wal()?;
            }
            store.set_collector_status(&CollectorStatus {
                mode: "import-post-sampling".to_owned(),
                phase: "live".to_owned(),
                items_total: report.observations,
                items_completed: report.matched.saturating_add(report.unmatched),
                bytes_read: report.bytes_read,
                events_inserted: report.inserted_events,
                message: None,
                updated_at: chrono::Utc::now(),
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ReconstructRollouts {
            db,
            codex_home,
            max_files,
            batches,
            project,
        } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let mut store = prepare_store(&paths.db)?;
            let machine_id = load_or_create_machine_id(&paths.data_dir)?;
            let hmac_key = load_or_create_hmac_key(&paths.data_dir)?;
            let _ = observe_auth(
                &mut store,
                &paths.codex_home.join("auth.json"),
                &hmac_key,
                &machine_id,
            )?;
            sync_account_history(&mut store, &paths.codex_home, &machine_id, &hmac_key)?;
            sync_native_catalog(&mut store, &paths.codex_home)?;
            let mut reports = Vec::new();
            for _ in 0..batches.max(1) {
                reports.push(ingest_reconstruction_batch_for_project(
                    &mut store,
                    &paths.codex_home,
                    &machine_id,
                    max_files,
                    project.as_deref(),
                )?);
            }
            store.checkpoint_wal()?;
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        Command::SyncOfficialUsage { db, codex_home } => {
            let paths = RuntimePaths::resolve(codex_home, db, None)?;
            let mut store = prepare_store(&paths.db)?;
            let machine_id = load_or_create_machine_id(&paths.data_dir)?;
            let hmac_key = load_or_create_hmac_key(&paths.data_dir)?;
            let binding = observe_auth(
                &mut store,
                &paths.codex_home.join("auth.json"),
                &hmac_key,
                &machine_id,
            )?
            .context("no active Codex authentication")?;
            let scope = OfficialUsageScope::new(&paths.codex_home)?;
            update_official_scope(&scope, Some(&binding))?;
            let bound = tokio::task::spawn_blocking(move || scope.fetch(None))
                .await
                .context("official usage worker stopped")??;
            let account = bound.account;
            let usage = bound.usage;
            let observed_at = chrono::Utc::now();
            store.upsert_official_account_usage(&account, observed_at, &usage)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "source": "codex account/usage/read",
                    "account": account_label_for_cli(&account),
                    "observedAt": observed_at,
                    "summary": usage.summary,
                    "dailyBuckets": usage.daily_usage_buckets.len(),
                    "coverageStart": usage.daily_usage_buckets.first().map(|bucket| &bucket.start_date),
                    "coverageThrough": usage.daily_usage_buckets.last().map(|bucket| &bucket.start_date),
                }))?
            );
        }
    }
    Ok(())
}

fn account_label_for_cli(account: &str) -> String {
    format!("{}…", account.chars().take(8).collect::<String>())
}

fn startup_collector_status(mode: &str) -> CollectorStatus {
    CollectorStatus {
        mode: mode.to_owned(),
        phase: if mode == "daemon" { "live" } else { "idle" }.to_owned(),
        items_total: 0,
        items_completed: 0,
        bytes_read: 0,
        events_inserted: 0,
        message: None,
        updated_at: chrono::Utc::now(),
    }
}

async fn run_daemon(paths: RuntimePaths, listen: SocketAddr, reconcile_seconds: u64) -> Result<()> {
    let mut writer = prepare_store(&paths.db)?;
    let machine_id = load_or_create_machine_id(&paths.data_dir)?;
    let hmac_key = load_or_create_hmac_key(&paths.data_dir)?;
    let mut account_binding = observe_auth(
        &mut writer,
        &paths.codex_home.join("auth.json"),
        &hmac_key,
        &machine_id,
    )?;
    let official_scope = OfficialUsageScope::new(&paths.codex_home)?;
    update_official_scope(&official_scope, account_binding.as_ref())?;
    let reader = prepare_store(&paths.db)?;
    let http = tokio::spawn(serve_http(
        ApiState::with_store(reader).with_official_scope(official_scope.clone()),
        listen,
        paths.web_root.clone(),
    ));
    writer.set_collector_status(&startup_collector_status("daemon"))?;
    let history = sync_account_history(&mut writer, &paths.codex_home, &machine_id, &hmac_key)?;
    info!(
        accounts = history.accounts_observed,
        epochs = history.inferred_epochs,
        reassigned = history.events_reassigned,
        "historical account boundaries synchronized"
    );
    sync_native_catalog(&mut writer, &paths.codex_home)
        .context("refresh Codex project and session directory")?;

    info!(%listen, "dashboard started with post-sampling collector");

    if let Some(binding) = account_binding.as_ref() {
        refresh_official_usage(&mut writer, binding, &official_scope).await;
    }

    let first_sampling_import = writer
        .get_cursor(&machine_id, POST_SAMPLING_SOURCE_ID)?
        .is_none();
    if first_sampling_import {
        writer.set_collector_status(&CollectorStatus {
            mode: "daemon".to_owned(),
            phase: "optimizing".to_owned(),
            items_total: 0,
            items_completed: 0,
            bytes_read: 0,
            events_inserted: 0,
            message: Some("首次建立 post-sampling 可信账本".to_owned()),
            updated_at: chrono::Utc::now(),
        })?;
    }
    let initial_status = collect_daemon_sources(&mut writer, &paths.codex_home, &machine_id)?;
    writer.reproject_usage_from_catalog()?;
    prepare_fast_ledger(&mut writer, "daemon")?;
    if let Err(error) = writer.backfill_quota_window_index_chunk(200) {
        warn!(%error, "quota window index backfill deferred");
    }
    if let Err(error) = writer.backfill_quota_history_chunk(200) {
        warn!(%error, "quota boundary backfill deferred");
    }
    let compacted = compact_expired_raw_events(&mut writer, "daemon")?;
    writer.set_collector_status(&initial_status)?;
    info!(
        phase = initial_status.phase,
        compacted, "initial source collection finished"
    );
    let mut reconcile = tokio::time::interval(Duration::from_secs(reconcile_seconds.max(5)));
    reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    reconcile.tick().await;
    let mut official_refresh = tokio::time::interval(Duration::from_secs(600));
    official_refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    official_refresh.tick().await;
    let mut ticks = 0_u64;
    loop {
        tokio::select! {
            _ = reconcile.tick() => {
                ticks = ticks.saturating_add(1);
                if let Err(error) = writer.backfill_request_evidence_chunk(1000) {
                    warn!(%error, "retained request backfill deferred");
                }
                if let Err(error) = writer.backfill_quota_window_index_chunk(200) {
                    warn!(%error, "quota window index backfill deferred");
                }
                if let Err(error) = writer.backfill_quota_history_chunk(200) {
                    warn!(%error, "quota boundary backfill deferred");
                }
                if ticks.is_multiple_of(6) {
                    if let Err(error) = sync_native_catalog(&mut writer, &paths.codex_home) {
                        warn!(%error, "Codex project and session directory refresh failed");
                    }
                    match observe_auth(
                        &mut writer,
                        &paths.codex_home.join("auth.json"),
                        &hmac_key,
                        &machine_id,
                    ) {
                        Ok(next_binding) => {
                            let switched = account_binding.as_ref().and_then(|value| value.account_fingerprint.as_ref())
                                != next_binding.as_ref().and_then(|value| value.account_fingerprint.as_ref());
                            account_binding = next_binding;
                            update_official_scope(&official_scope, account_binding.as_ref())?;
                            if switched
                                && let Some(binding) = account_binding.as_ref()
                            {
                                refresh_official_usage(&mut writer, binding, &official_scope).await;
                            }
                        }
                        Err(error) => { official_scope.observe(None,None)?; warn!(%error, "auth observation was temporarily unavailable"); },
                    }
                    if let Err(error) = sync_account_history(
                        &mut writer,
                        &paths.codex_home,
                        &machine_id,
                        &hmac_key,
                    ) {
                        warn!(%error, "historical account boundary refresh failed");
                    }
                }
                let status = collect_daemon_sources(&mut writer, &paths.codex_home, &machine_id)?;
                publish_daemon_status(&mut writer, &status)?;
            }
            _ = official_refresh.tick() => {
                if let Some(binding) = account_binding.as_ref() {
                    refresh_official_usage(&mut writer, binding, &official_scope).await;
                }
            }
            _ = shutdown_signal() => {
                info!("shutdown requested");
                break;
            }
        }
    }
    http.abort();
    writer.checkpoint_wal()?;
    Ok(())
}

// Source failures are retryable operational state, not a reason to discard
// the HTTP service. Reading/publishing collector state can still fail fatally;
// an ingest failure never authorizes discarding evidence or resetting cursors.
fn collection_step<T>(
    source: &'static str,
    result: Result<T>,
    failures: &mut Vec<&'static str>,
) -> Option<T> {
    match result {
        Ok(report) => Some(report),
        Err(error) => {
            warn!(source, %error, "source collection deferred until next tick");
            failures.push(source);
            None
        }
    }
}

fn collect_daemon_sources(
    store: &mut LedgerStore,
    home: &Path,
    machine: &str,
) -> Result<CollectorStatus> {
    let mut status = store.collector_status()?;
    let mut failures = Vec::new();
    let sampling = collection_step(
        "sampling",
        ingest_post_sampling(store, home, machine),
        &mut failures,
    );
    if let Some(report) = sampling {
        status.items_total = report.observations;
        status.items_completed = report.matched.saturating_add(report.unmatched);
        status.bytes_read = report.bytes_read;
        status.events_inserted = report.inserted_events;
    }
    if let Some(report) = collection_step(
        "quota",
        ingest_quota_tails(store, home, machine),
        &mut failures,
    ) {
        if !report.issues.is_empty() {
            failures.push("quota");
        }
        if report.quota_snapshots > 0 {
            info!(
                snapshots = report.quota_snapshots,
                "quota snapshots synchronized"
            );
        }
    }
    if let Some(report) = collection_step(
        "reconstruction",
        ingest_reconstruction_batch(store, home, machine, 8),
        &mut failures,
    ) {
        if report.identity_review_sources > 0 {
            failures.push("reconstruction_identity_review");
        } else if !report.issues.is_empty() {
            failures.push("reconstruction");
        }
        if report.files_advanced > 0 {
            info!(
                files = report.files_advanced,
                events = report.inserted_events,
                "rollout reconstruction slice synchronized"
            );
        }
    }
    status.mode = "daemon".to_owned();
    status.phase = if failures.is_empty() {
        "live"
    } else {
        "degraded"
    }
    .to_owned();
    // Stable codes only: never expose a source path, database error, or body in
    // the dashboard. Each locale supplies the user-facing explanation.
    status.message = (!failures.is_empty()).then(|| failures.join(","));
    status.updated_at = chrono::Utc::now();
    Ok(status)
}

fn publish_daemon_status(store: &mut LedgerStore, status: &CollectorStatus) -> Result<()> {
    let previous = store.collector_status()?;
    if previous.phase != status.phase
        || previous.message != status.message
        || previous.items_total != status.items_total
        || previous.items_completed != status.items_completed
        || previous.bytes_read != status.bytes_read
        || previous.events_inserted != status.events_inserted
    {
        store.set_collector_status(status)?;
    }
    Ok(())
}

fn update_official_scope(
    scope: &OfficialUsageScope,
    binding: Option<&AccountBinding>,
) -> Result<()> {
    let binding = binding.filter(|binding| binding.confidence == AttributionConfidence::Verified);
    scope.observe(
        binding.and_then(|value| value.account_fingerprint.as_deref()),
        binding.and_then(|value| value.auth_file_stamp.as_ref()),
    )
}

async fn refresh_official_usage(
    writer: &mut LedgerStore,
    binding: &AccountBinding,
    scope: &OfficialUsageScope,
) {
    let Some(account_fingerprint) = binding.account_fingerprint.as_deref() else {
        return;
    };
    let observed_at = chrono::Utc::now();
    let scope = scope.clone();
    match tokio::task::spawn_blocking(move || scope.fetch(None)).await {
        Ok(Ok(bound)) => {
            if bound.account != account_fingerprint {
                return;
            }
            let usage = bound.usage;
            if let Err(error) =
                writer.upsert_official_account_usage(account_fingerprint, observed_at, &usage)
            {
                warn!(%error, "official Codex usage persistence failed");
            } else {
                info!(
                    account = %account_fingerprint.chars().take(8).collect::<String>(),
                    lifetime_tokens = usage.summary.lifetime_tokens.unwrap_or_default(),
                    daily_buckets = usage.daily_usage_buckets.len(),
                    "official Codex usage synchronized"
                );
            }
        }
        Ok(Err(error)) => {
            let message = error.to_string();
            let _ = writer.record_official_usage_error(account_fingerprint, observed_at, &message);
            warn!(%error, "official Codex usage temporarily unavailable");
        }
        Err(error) => {
            let message = error.to_string();
            let _ = writer.record_official_usage_error(account_fingerprint, observed_at, &message);
            warn!(%error, "official Codex usage worker stopped");
        }
    }
}

async fn run_dashboard_only(paths: RuntimePaths, listen: SocketAddr) -> Result<()> {
    let mut writer = prepare_store(&paths.db)?;
    let machine_id = load_or_create_machine_id(&paths.data_dir)?;
    let hmac_key = load_or_create_hmac_key(&paths.data_dir)?;
    let mut account_binding = observe_auth(
        &mut writer,
        &paths.codex_home.join("auth.json"),
        &hmac_key,
        &machine_id,
    )?;
    let official_scope = OfficialUsageScope::new(&paths.codex_home)?;
    update_official_scope(&official_scope, account_binding.as_ref())?;
    let reader = prepare_store(&paths.db)?;
    let mut http = tokio::spawn(serve_http(
        ApiState::with_store(reader).with_official_scope(official_scope.clone()),
        listen,
        paths.web_root.clone(),
    ));
    writer.set_collector_status(&startup_collector_status("serve"))?;
    sync_account_history(&mut writer, &paths.codex_home, &machine_id, &hmac_key)?;
    sync_native_catalog(&mut writer, &paths.codex_home)
        .context("refresh Codex project and session directory")?;
    writer.reproject_usage_from_catalog()?;
    let mut catalog_refresh = tokio::time::interval(Duration::from_secs(10));
    catalog_refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    catalog_refresh.tick().await;
    let mut official_refresh = tokio::time::interval(Duration::from_secs(600));
    official_refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    official_refresh.tick().await;

    if let Some(binding) = account_binding.as_ref() {
        refresh_official_usage(&mut writer, binding, &official_scope).await;
    }

    prepare_fast_ledger(&mut writer, "serve")?;
    if let Err(error) = writer.backfill_quota_window_index_chunk(200) {
        warn!(%error, "quota window index backfill deferred");
    }
    if let Err(error) = writer.backfill_quota_history_chunk(200) {
        warn!(%error, "quota boundary backfill deferred");
    }
    // Opening the dashboard is not a request to delete historical raw details.
    // Keep retention in explicit optimize/collection workflows with its guards.
    writer.set_collector_status(&CollectorStatus {
        mode: "serve".to_owned(),
        phase: "idle".to_owned(),
        items_total: 0,
        items_completed: 0,
        bytes_read: 0,
        events_inserted: 0,
        message: None,
        updated_at: chrono::Utc::now(),
    })?;

    let mut catalog_ticks = 0_u64;
    loop {
        tokio::select! {
            _ = catalog_refresh.tick() => {
                catalog_ticks = catalog_ticks.saturating_add(1);
                if let Err(error) = writer.backfill_quota_window_index_chunk(200) {
                    warn!(%error, "quota window index backfill deferred");
                }
                if let Err(error) = writer.backfill_quota_history_chunk(200) {
                    warn!(%error, "quota boundary backfill deferred");
                }
                if let Err(error) = writer.backfill_request_evidence_chunk(1000) {
                    warn!(%error, "retained request backfill deferred");
                }
                if let Err(error) = sync_native_catalog(&mut writer, &paths.codex_home) {
                    warn!(%error, "Codex project and session directory refresh failed");
                }
                if catalog_ticks.is_multiple_of(3) {
                    match observe_auth(
                        &mut writer,
                        &paths.codex_home.join("auth.json"),
                        &hmac_key,
                        &machine_id,
                    ) {
                        Ok(next_binding) => {
                            let switched = account_binding.as_ref().and_then(|value| value.account_fingerprint.as_ref())
                                != next_binding.as_ref().and_then(|value| value.account_fingerprint.as_ref());
                            account_binding = next_binding;
                            update_official_scope(&official_scope, account_binding.as_ref())?;
                            if switched
                                && let Some(binding) = account_binding.as_ref()
                            {
                                refresh_official_usage(&mut writer, binding, &official_scope).await;
                            }
                        }
                        Err(error) => { official_scope.observe(None,None)?; warn!(%error, "auth observation was temporarily unavailable"); },
                    }
                    if let Err(error) = sync_account_history(
                        &mut writer,
                        &paths.codex_home,
                        &machine_id,
                        &hmac_key,
                    ) {
                        warn!(%error, "historical account boundary refresh failed");
                    }
                }
            }
            _ = official_refresh.tick() => {
                if let Some(binding) = account_binding.as_ref() {
                    refresh_official_usage(&mut writer, binding, &official_scope).await;
                }
            }
            result = &mut http => {
                writer.checkpoint_wal()?;
                return result.context("dashboard server task stopped")?;
            }
        }
    }
}

async fn serve_http(state: ApiState, listen: SocketAddr, web_root: PathBuf) -> Result<()> {
    let index = web_root.join("index.html");
    let app = Router::new()
        .merge(api::router(state))
        .fallback_service(ServeDir::new(&web_root).fallback(ServeFile::new(index)))
        .layer(middleware::from_fn(enforce_local_http_identity));
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn enforce_local_http_identity(request: Request<Body>, next: Next) -> Response {
    if !headers_are_local(request.headers()) {
        return (
            StatusCode::FORBIDDEN,
            [(header::CONTENT_TYPE, "application/json")],
            r#"{"error":"loopback host or origin required"}"#,
        )
            .into_response();
    }
    next.run(request).await
}

fn headers_are_local(headers: &HeaderMap) -> bool {
    let host_is_local = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Authority>().ok())
        .is_some_and(|authority| matches!(authority.host(), "127.0.0.1" | "localhost"));
    if !host_is_local {
        return false;
    }

    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(origin) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    origin.scheme_str() == Some("http")
        && matches!(origin.host(), Some("127.0.0.1") | Some("localhost"))
}

fn ensure_loopback(address: SocketAddr) -> Result<()> {
    if !address.ip().is_loopback() {
        bail!("refusing non-loopback listener {address}; the dashboard is local-only");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct RuntimePaths {
    codex_home: PathBuf,
    db: PathBuf,
    data_dir: PathBuf,
    web_root: PathBuf,
}

impl RuntimePaths {
    fn resolve(
        codex_home: Option<PathBuf>,
        db: Option<PathBuf>,
        web_root: Option<PathBuf>,
    ) -> Result<Self> {
        let codex_home = codex_home
            .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .context("CODEX_HOME and HOME are unset; pass --codex-home")?;
        let db = db
            .or_else(|| std::env::var_os("CODEX_USAGE_LEDGER_DB").map(PathBuf::from))
            .unwrap_or_else(|| codex_home.join("usage-ledger.sqlite3"));
        let data_dir = db
            .parent()
            .map(Path::to_path_buf)
            .context("database path has no parent directory")?;
        let web_root = web_root
            .or_else(|| std::env::var_os("CODEX_USAGE_LEDGER_WEB_ROOT").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("web/dist"));
        Ok(Self {
            codex_home,
            db,
            data_dir,
            web_root,
        })
    }
}

#[cfg(test)]
mod local_http_tests {
    use super::*;

    #[test]
    fn unavailable_sources_are_retryable_and_preserve_previous_evidence() {
        let home = tempfile::tempdir().unwrap();
        let mut store = LedgerStore::open_in_memory().unwrap();
        let mut previous = startup_collector_status("daemon");
        previous.items_total = 12;
        previous.items_completed = 12;
        previous.events_inserted = 10;
        store.set_collector_status(&previous).unwrap();
        let before = store.aggregate_usage(&AggregateFilter::default()).unwrap();
        for _ in 0..2 {
            let failed =
                collect_daemon_sources(&mut store, home.path(), "synthetic-machine").unwrap();
            assert_eq!(failed.phase, "degraded");
            assert!(
                failed
                    .message
                    .as_deref()
                    .unwrap()
                    .split(',')
                    .any(|source| source == "sampling")
            );
            assert_eq!(failed.events_inserted, 10);
            publish_daemon_status(&mut store, &failed).unwrap();
            assert_eq!(
                store.aggregate_usage(&AggregateFilter::default()).unwrap(),
                before
            );
        }
        let saved = store.collector_status().unwrap();
        let mut repeated = saved.clone();
        repeated.updated_at += chrono::Duration::seconds(5);
        publish_daemon_status(&mut store, &repeated).unwrap();
        assert_eq!(
            store.collector_status().unwrap().updated_at,
            saved.updated_at
        );
        rusqlite::Connection::open(home.path().join("logs_2.sqlite")).unwrap().execute_batch(
            "CREATE TABLE logs(id INTEGER PRIMARY KEY, ts INTEGER, ts_nanos INTEGER, thread_id TEXT, feedback_log_body TEXT, target TEXT, process_uuid TEXT);"
        ).unwrap();
        let recovered =
            collect_daemon_sources(&mut store, home.path(), "synthetic-machine").unwrap();
        assert_eq!(recovered.phase, "live");
        publish_daemon_status(&mut store, &recovered).unwrap();
        assert_eq!(store.collector_status().unwrap().phase, "live");
        assert!(store.collector_status().unwrap().message.is_none());
    }

    #[test]
    fn failed_source_does_not_prevent_other_steps_or_leak_error_text() {
        let mut failures = Vec::new();
        let failed: Option<()> = collection_step(
            "sampling",
            Err(anyhow::anyhow!("private source details")),
            &mut failures,
        );
        assert!(failed.is_none());
        assert_eq!(collection_step("quota", Ok(7), &mut failures), Some(7));
        assert_eq!(failures, ["sampling"]);
    }

    #[test]
    fn rejects_dns_rebinding_and_cross_site_origins() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "evil.example:47127".parse().unwrap());
        assert!(!headers_are_local(&headers));

        headers.insert(header::HOST, "127.0.0.1:47127".parse().unwrap());
        headers.insert(header::ORIGIN, "https://evil.example".parse().unwrap());
        assert!(!headers_are_local(&headers));

        headers.insert(header::ORIGIN, "http://127.0.0.1:47127".parse().unwrap());
        assert!(headers_are_local(&headers));
    }

    #[test]
    fn allows_loopback_requests_without_browser_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "localhost:47127".parse().unwrap());
        assert!(headers_are_local(&headers));
    }

    #[test]
    fn ordinary_startup_never_claims_a_history_backfill() {
        let daemon = startup_collector_status("daemon");
        let serve = startup_collector_status("serve");
        assert_eq!(daemon.phase, "live");
        assert_eq!(serve.phase, "idle");
        for status in [daemon, serve] {
            assert!(!matches!(
                status.phase.as_str(),
                "backfill" | "optimizing" | "compacting" | "syncing"
            ));
            assert!(status.message.is_none());
        }
    }
}
