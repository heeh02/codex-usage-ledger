//! Incremental reconstruction of Codex account-switch boundaries.
//!
//! Codex logs account reloads, logouts and OAuth completion in `logs_2.sqlite`.
//! We retain only HMAC fingerprints and timestamps, never raw account IDs or
//! credentials. The durable log cursor makes subsequent runs tail-only.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;
use serde::Serialize;

use crate::{
    identity::{fingerprint_logged_workspace_id, provisional_account_fingerprint},
    store::{AuthLogMarkerRecord, FileCursor, HistoricalAuthEpochInput, LedgerStore},
    types::AttributionConfidence,
};

pub const AUTH_HISTORY_SOURCE_PREFIX: &str = "logs2-auth-history-v1";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountHistoryReport {
    pub sources: u64,
    pub markers_added: u64,
    pub inferred_epochs: u64,
    pub accounts_observed: u64,
    pub events_reassigned: u64,
    pub earliest_sampling_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthHistoryCursorState {
    version: u32,
    earliest_sampling_at: Option<DateTime<Utc>>,
}

pub fn sync_account_history(
    store: &mut LedgerStore,
    codex_home: &Path,
    machine_id: &str,
    hmac_key: &[u8],
) -> Result<AccountHistoryReport> {
    let sources = log_sources(codex_home);
    let safe_before = Utc::now() - Duration::seconds(5);
    let mut report = AccountHistoryReport {
        sources: sources.len() as u64,
        ..AccountHistoryReport::default()
    };
    for path in &sources {
        let source_id = source_id(codex_home, path);
        let stored_cursor = store.get_cursor(machine_id, &source_id)?;
        let after_id = stored_cursor
            .as_ref()
            .map(|cursor| cursor.byte_offset)
            .unwrap_or_default();
        let stored_state = stored_cursor
            .as_ref()
            .and_then(|cursor| cursor.parser_state_json.as_deref())
            .and_then(|value| serde_json::from_str::<AuthHistoryCursorState>(value).ok());
        let (markers, max_id, earliest_sampling) = read_increment(
            path,
            after_id,
            safe_before,
            hmac_key,
            stored_state
                .as_ref()
                .and_then(|state| state.earliest_sampling_at),
        )?;
        report.markers_added = report.markers_added.saturating_add(markers.len() as u64);
        report.earliest_sampling_at = match (report.earliest_sampling_at, earliest_sampling) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (None, value) | (value, None) => value,
        };
        if max_id > after_id {
            store.upsert_auth_log_markers_and_cursor(
                machine_id,
                &source_id,
                &markers,
                &FileCursor {
                    machine_id: machine_id.to_owned(),
                    source_id: source_id.clone(),
                    file_identity: format!(
                        "logs2-auth:{}:{}",
                        path.metadata().map(|value| value.len()).unwrap_or_default(),
                        path.display()
                    ),
                    byte_offset: max_id,
                    line_number: max_id,
                    parser_state_json: Some(serde_json::to_string(&AuthHistoryCursorState {
                        version: 1,
                        earliest_sampling_at: earliest_sampling,
                    })?),
                    updated_at: Utc::now(),
                },
            )?;
        }
    }

    let existing_epochs = store.list_auth_epochs(machine_id, AUTH_HISTORY_SOURCE_PREFIX)?;
    if report.markers_added == 0 && !existing_epochs.is_empty() {
        report.inferred_epochs = existing_epochs.len() as u64;
        report.accounts_observed = existing_epochs
            .iter()
            .filter_map(|epoch| epoch.workspace_fingerprint.as_deref())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64;
        return Ok(report);
    }

    let markers = store.auth_log_markers(machine_id)?;
    let mut epochs = infer_epochs(&markers, report.earliest_sampling_at);
    for epoch in &mut epochs {
        let account = if let Some(account) =
            store.canonical_account_for_workspace(&epoch.workspace_fingerprint)?
        {
            store.upsert_workspace_account_alias(
                &epoch.workspace_fingerprint,
                &account,
                true,
                epoch.observed_from,
            )?;
            account
        } else {
            let provisional =
                provisional_account_fingerprint(&epoch.workspace_fingerprint, hmac_key)?;
            store.upsert_workspace_account_alias(
                &epoch.workspace_fingerprint,
                &provisional,
                false,
                epoch.observed_from,
            )?;
            provisional
        };
        epoch.account_fingerprint = account;
    }
    report.accounts_observed = epochs
        .iter()
        .map(|epoch| epoch.workspace_fingerprint.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u64;
    report.inferred_epochs = epochs.len() as u64;
    report.events_reassigned =
        store.replace_historical_auth_epochs(machine_id, AUTH_HISTORY_SOURCE_PREFIX, &epochs)?
            as u64;
    Ok(report)
}

fn log_sources(codex_home: &Path) -> Vec<PathBuf> {
    let mut paths = [
        codex_home.join("sqlite/logs_2.sqlite"),
        codex_home.join("logs_2.sqlite"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn source_id(codex_home: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(codex_home).unwrap_or(path);
    format!("{AUTH_HISTORY_SOURCE_PREFIX}:{}", relative.display())
}

fn read_increment(
    path: &Path,
    after_id: u64,
    safe_before: DateTime<Utc>,
    hmac_key: &[u8],
    known_earliest_sampling: Option<DateTime<Utc>>,
) -> Result<(Vec<AuthLogMarkerRecord>, u64, Option<DateTime<Utc>>)> {
    let connection = open_logs(path)?;
    let max_id: i64 = connection.query_row(
        "SELECT COALESCE(MAX(id), ?1) FROM logs WHERE ts <= ?2",
        params![
            i64::try_from(after_id).unwrap_or(i64::MAX),
            safe_before.timestamp()
        ],
        |row| row.get(0),
    )?;
    let earliest_sampling = match known_earliest_sampling {
        Some(value) => Some(value),
        None => earliest_sampling_with(&connection)?,
    };
    let mut statement = connection.prepare(
        "SELECT id, ts, ts_nanos, target, feedback_log_body
         FROM logs
         WHERE id > ?1 AND id <= ?2 AND (
             (target = 'codex_login::auth::manager' AND (
                 instr(feedback_log_body, 'Reloading auth for account ') > 0 OR
                 instr(feedback_log_body, 'account/logout') > 0
             )) OR
             (target = 'codex_login::server' AND
                 instr(feedback_log_body, 'oauth token exchange succeeded') > 0)
         ) ORDER BY id",
    )?;
    let rows = statement.query_map(
        params![i64::try_from(after_id).unwrap_or(i64::MAX), max_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?;
    let mut markers = Vec::new();
    for row in rows {
        let (id, seconds, nanos, target, body) = row?;
        let Some(observed_at) =
            DateTime::<Utc>::from_timestamp(seconds, nanos.clamp(0, 999_999_999) as u32)
        else {
            continue;
        };
        let (kind, workspace_fingerprint) = if target == "codex_login::auth::manager"
            && body.contains("account/logout")
        {
            ("logout".to_owned(), None)
        } else if target == "codex_login::server" && body.contains("oauth token exchange succeeded")
        {
            ("login_success".to_owned(), None)
        } else if let Some(raw) = extract_account_id(&body) {
            (
                "account_seen".to_owned(),
                Some(fingerprint_logged_workspace_id(raw, hmac_key)?),
            )
        } else {
            continue;
        };
        markers.push(AuthLogMarkerRecord {
            log_id: u64::try_from(id).unwrap_or_default(),
            observed_at,
            kind,
            workspace_fingerprint,
        });
    }
    Ok((
        markers,
        u64::try_from(max_id).unwrap_or(after_id),
        earliest_sampling,
    ))
}

fn open_logs(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open Codex auth log {}", path.display()))?;
    connection.pragma_update(None, "query_only", "ON")?;
    Ok(connection)
}

fn earliest_sampling_with(connection: &Connection) -> Result<Option<DateTime<Utc>>> {
    let value = connection.query_row(
        "SELECT MIN(ts) FROM logs
         WHERE target = 'codex_core::session::turn'
           AND instr(feedback_log_body, ' post sampling token usage ') > 0",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(value.and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0)))
}

fn extract_account_id(body: &str) -> Option<&str> {
    let marker = "Reloading auth for account ";
    let tail = &body[body.rfind(marker)? + marker.len()..];
    let value = tail.split_whitespace().next()?.trim();
    (!value.is_empty() && value.len() <= 256).then_some(value)
}

fn infer_epochs(
    markers: &[AuthLogMarkerRecord],
    earliest_sampling_at: Option<DateTime<Utc>>,
) -> Vec<HistoricalAuthEpochInput> {
    let Some(earliest) = earliest_sampling_at else {
        return Vec::new();
    };
    let mut logouts = markers
        .iter()
        .filter(|marker| marker.kind == "logout")
        .map(|marker| marker.observed_at)
        .collect::<Vec<_>>();
    logouts.sort();
    logouts.dedup_by(|right, left| (*right - *left).num_seconds().abs() <= 1);
    let mut bounds = Vec::with_capacity(logouts.len() + 2);
    bounds.push(earliest);
    bounds.extend(logouts.iter().copied());
    bounds.push(Utc::now() + Duration::days(3650));

    let mut epochs = Vec::new();
    for window in bounds.windows(2) {
        let start = window[0];
        let end = window[1];
        let workspaces = markers
            .iter()
            .filter(|marker| {
                marker.kind == "account_seen"
                    && marker.observed_at >= start
                    && marker.observed_at < end
            })
            .filter_map(|marker| marker.workspace_fingerprint.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if workspaces.len() != 1 {
            continue;
        }
        let workspace_fingerprint = workspaces.into_iter().next().unwrap_or_default();
        let first_seen = markers
            .iter()
            .filter(|marker| {
                marker.workspace_fingerprint.as_deref() == Some(workspace_fingerprint.as_str())
                    && marker.observed_at >= start
                    && marker.observed_at < end
            })
            .map(|marker| marker.observed_at)
            .min()
            .unwrap_or(start);
        let login_start = markers
            .iter()
            .filter(|marker| {
                marker.kind == "login_success"
                    && marker.observed_at >= start
                    && marker.observed_at <= first_seen
            })
            .map(|marker| marker.observed_at)
            .max();
        epochs.push(HistoricalAuthEpochInput {
            observed_from: login_start.unwrap_or(start),
            observed_to: (end <= Utc::now()).then_some(end),
            account_fingerprint: String::new(),
            workspace_fingerprint,
            confidence: AttributionConfidence::Inferred,
        });
    }
    epochs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn logout_windows_reconstruct_two_accounts_and_login_gap() {
        let markers = vec![
            AuthLogMarkerRecord {
                log_id: 1,
                observed_at: at("2026-08-22T14:00:00Z"),
                kind: "account_seen".into(),
                workspace_fingerprint: Some("primary".into()),
            },
            AuthLogMarkerRecord {
                log_id: 2,
                observed_at: at("2026-08-24T18:07:10Z"),
                kind: "logout".into(),
                workspace_fingerprint: None,
            },
            AuthLogMarkerRecord {
                log_id: 3,
                observed_at: at("2026-08-26T02:59:48Z"),
                kind: "account_seen".into(),
                workspace_fingerprint: Some("secondary".into()),
            },
            AuthLogMarkerRecord {
                log_id: 4,
                observed_at: at("2026-08-27T15:58:05Z"),
                kind: "logout".into(),
                workspace_fingerprint: None,
            },
            AuthLogMarkerRecord {
                log_id: 5,
                observed_at: at("2026-08-27T17:25:40Z"),
                kind: "login_success".into(),
                workspace_fingerprint: None,
            },
            AuthLogMarkerRecord {
                log_id: 6,
                observed_at: at("2026-08-28T04:58:30Z"),
                kind: "account_seen".into(),
                workspace_fingerprint: Some("primary".into()),
            },
        ];
        let epochs = infer_epochs(&markers, Some(at("2026-06-13T01:25:02Z")));
        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs[0].workspace_fingerprint, "primary");
        assert_eq!(epochs[0].observed_from, at("2026-06-13T01:25:02Z"));
        assert_eq!(epochs[1].workspace_fingerprint, "secondary");
        assert_eq!(epochs[1].observed_from, at("2026-08-24T18:07:10Z"));
        assert_eq!(epochs[2].observed_from, at("2026-08-27T17:25:40Z"));
    }

    #[test]
    fn account_extraction_uses_only_the_terminal_auth_marker() {
        assert_eq!(
            extract_account_id("span: Reloading auth for account workspace-123"),
            Some("workspace-123")
        );
        assert_eq!(extract_account_id("Reloading auth"), None);
    }
}
