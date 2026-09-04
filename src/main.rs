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
    api::{self, ApiState, UsageQuery},
    cli_support::{
        AccountBinding, AggregateDimension, AggregateFilter, CollectorStatus, LedgerStore,
        POST_SAMPLING_SOURCE_ID, compact_expired_raw_events, discover_rollouts,
        fetch_official_account_usage, ingest_post_sampling, ingest_quota_tails,
        ingest_reconstruction_batch, ingest_reconstruction_batch_for_project,
        load_or_create_hmac_key, load_or_create_machine_id, observe_auth, prepare_fast_ledger,
        prepare_store, sync_account_history, sync_native_catalog,
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
            let account = binding
                .account_fingerprint
                .context("active Codex account cannot be fingerprinted safely")?;
            let usage = tokio::task::spawn_blocking(fetch_official_account_usage)
                .await
                .context("official usage worker stopped")??;
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
    let reader = prepare_store(&paths.db)?;
    let http = tokio::spawn(serve_http(
        ApiState::with_store(reader),
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
        refresh_official_usage(&mut writer, binding).await;
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
    let initial = ingest_post_sampling(&mut writer, &paths.codex_home, &machine_id)?;
    let initial_quota = ingest_quota_tails(&mut writer, &paths.codex_home, &machine_id)?;
    writer.reproject_usage_from_catalog()?;
    if let Err(error) = ingest_reconstruction_batch(&mut writer, &paths.codex_home, &machine_id, 8)
    {
        warn!(%error, "initial rollout reconstruction slice failed");
    }
    prepare_fast_ledger(&mut writer, "daemon")?;
    let compacted = compact_expired_raw_events(&mut writer, "daemon")?;
    writer.set_collector_status(&CollectorStatus {
        mode: "daemon".to_owned(),
        phase: "live".to_owned(),
        items_total: initial.observations,
        items_completed: initial.matched.saturating_add(initial.unmatched),
        bytes_read: initial.bytes_read,
        events_inserted: initial.inserted_events,
        message: None,
        updated_at: chrono::Utc::now(),
    })?;
    info!(
        observations = initial.observations,
        matched = initial.matched,
        unmatched = initial.unmatched,
        compacted,
        quota_snapshots = initial_quota.quota_snapshots,
        quota_bytes_read = initial_quota.bytes_read,
        "post-sampling synchronization complete"
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
                            if switched
                                && let Some(binding) = account_binding.as_ref()
                            {
                                refresh_official_usage(&mut writer, binding).await;
                            }
                        }
                        Err(error) => warn!(%error, "auth observation was temporarily unavailable"),
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
                let report = ingest_post_sampling(&mut writer, &paths.codex_home, &machine_id)?;
                let quota_report = ingest_quota_tails(&mut writer, &paths.codex_home, &machine_id)?;
                match ingest_reconstruction_batch(
                    &mut writer,
                    &paths.codex_home,
                    &machine_id,
                    8,
                ) {
                    Ok(reconstruction) if reconstruction.files_advanced > 0 => info!(
                        files = reconstruction.files_advanced,
                        bytes = reconstruction.bytes_read,
                        events = reconstruction.inserted_events,
                        pending = reconstruction.pending_sources,
                        "rollout reconstruction slice synchronized"
                    ),
                    Ok(_) => {}
                    Err(error) => warn!(%error, "rollout reconstruction slice failed"),
                }
                if report.observations > 0 {
                    writer.set_collector_status(&CollectorStatus {
                        mode: "daemon".to_owned(),
                        phase: "live".to_owned(),
                        items_total: report.observations,
                        items_completed: report.matched.saturating_add(report.unmatched),
                        bytes_read: report.bytes_read,
                        events_inserted: report.inserted_events,
                        message: None,
                        updated_at: chrono::Utc::now(),
                    })?;
                }
                if quota_report.quota_snapshots > 0 {
                    info!(
                        snapshots = quota_report.quota_snapshots,
                        files = quota_report.files_advanced,
                        bytes_read = quota_report.bytes_read,
                        "official quota snapshots synchronized"
                    );
                }
            }
            _ = official_refresh.tick() => {
                if let Some(binding) = account_binding.as_ref() {
                    refresh_official_usage(&mut writer, binding).await;
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

async fn refresh_official_usage(writer: &mut LedgerStore, binding: &AccountBinding) {
    let Some(account_fingerprint) = binding.account_fingerprint.as_deref() else {
        return;
    };
    let observed_at = chrono::Utc::now();
    match tokio::task::spawn_blocking(fetch_official_account_usage).await {
        Ok(Ok(usage)) => {
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
    let reader = prepare_store(&paths.db)?;
    let mut http = tokio::spawn(serve_http(
        ApiState::with_store(reader),
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
        refresh_official_usage(&mut writer, binding).await;
    }

    prepare_fast_ledger(&mut writer, "serve")?;
    compact_expired_raw_events(&mut writer, "serve")?;
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
                            if switched
                                && let Some(binding) = account_binding.as_ref()
                            {
                                refresh_official_usage(&mut writer, binding).await;
                            }
                        }
                        Err(error) => warn!(%error, "auth observation was temporarily unavailable"),
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
                    refresh_official_usage(&mut writer, binding).await;
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
