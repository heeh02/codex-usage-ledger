use super::*;

pub(super) fn quota_views(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let selected_account = selected(&query.account);
    let accounts = store
        .quota_accounts()?
        .into_iter()
        .filter(|account| {
            selected_account
                .as_deref()
                .is_none_or(|selected| selected == account)
        })
        .collect::<BTreeSet<_>>();
    let mut views = Vec::new();
    for account in accounts {
        let snapshots = store.list_quota_snapshots(&account, 1_000)?;
        let mut seen_windows = BTreeSet::new();
        let mut credits_added = false;
        for stored in snapshots {
            let stale = Utc::now()
                .signed_duration_since(stored.observed_at)
                .num_seconds()
                > 600;
            for pool in &stored.snapshot.pools {
                if pool.windows.is_empty() {
                    let key = format!("{}:none", pool.pool_key);
                    if seen_windows.insert(key) {
                        views.push(quota_value(&account, &stored, pool, None, stale, None));
                    }
                } else {
                    for (index, window) in pool.windows.iter().enumerate() {
                        let key = format!(
                            "{}:{:?}:{}",
                            pool.limit_id.as_deref().unwrap_or(&pool.pool_key),
                            window.role,
                            window.server_name
                        );
                        if seen_windows.insert(key) {
                            views.push(quota_value(
                                &account,
                                &stored,
                                pool,
                                Some(window),
                                stale,
                                Some(index),
                            ));
                        }
                    }
                }
            }
            if !credits_added && let Some(credits) = &stored.snapshot.credits {
                let detail = if credits.unlimited == Some(true) {
                    "Credits 不限量".to_owned()
                } else if let Some(balance) = credits.balance.as_deref() {
                    format!("Credits 余额 {balance}")
                } else {
                    "Credits 余额未知".to_owned()
                };
                views.push(serde_json::json!({
                    "id": format!("{}:credits", account),
                    "accountId": account,
                    "accountLabel": account_label(&account),
                    "limitId": "credits",
                    "label": "额外 Credits",
                    "usedPercent": serde_json::Value::Null,
                    "windowMinutes": serde_json::Value::Null,
                    "resetsAt": serde_json::Value::Null,
                    "observedAt": stored.observed_at,
                    "status": "unknown",
                    "stale": stale,
                    "detail": detail,
                }));
                credits_added = true;
            }
        }
    }
    views.sort_by(|left, right| {
        left.get("accountLabel")
            .and_then(|value| value.as_str())
            .cmp(&right.get("accountLabel").and_then(|value| value.as_str()))
            .then_with(|| {
                let left_main =
                    left.get("limitId").and_then(|value| value.as_str()) == Some("codex");
                let right_main =
                    right.get("limitId").and_then(|value| value.as_str()) == Some("codex");
                right_main.cmp(&left_main)
            })
            .then_with(|| {
                right
                    .get("windowMinutes")
                    .and_then(|value| value.as_u64())
                    .cmp(&left.get("windowMinutes").and_then(|value| value.as_u64()))
            })
    });
    Ok(views)
}

pub(super) fn quota_display_label(
    limit_name: Option<&str>,
    limit_id: Option<&str>,
    pool_key: &str,
) -> String {
    if let Some(name) = limit_name.map(str::trim).filter(|value| !value.is_empty()) {
        return name.to_owned();
    }
    if limit_id == Some("codex") {
        return "Codex 主额度".to_owned();
    }
    if let Some(id) = pool_key.strip_prefix("dynamic:id:") {
        return id.to_owned();
    }
    "动态额度池".to_owned()
}

fn quota_value(
    account: &str,
    stored: &crate::store::StoredQuotaSnapshot,
    pool: &crate::quota::QuotaPool,
    window: Option<&crate::quota::QuotaWindow>,
    stale: bool,
    index: Option<usize>,
) -> serde_json::Value {
    let used = window.and_then(|window| window.used_percent);
    let status = match used {
        Some(value) if value >= 90.0 => "critical",
        Some(value) if value >= 72.0 => "warning",
        Some(_) => "healthy",
        None => "unknown",
    };
    let resets_at = window
        .and_then(|window| window.resets_at_unix)
        .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single());
    let role = window
        .map(|window| format!("{:?}", window.role).to_ascii_lowercase())
        .unwrap_or_else(|| "dynamic".into());
    let window_minutes = window
        .and_then(|window| window.window_seconds)
        .map(|seconds| seconds / 60);
    let role_label = match role.as_str() {
        "primary" => "主窗口",
        "secondary" => "次窗口",
        _ => "动态窗口",
    };
    let duration_label = match window_minutes {
        Some(10_080) => "7 天".to_owned(),
        Some(300) => "5 小时".to_owned(),
        Some(minutes) => quota_duration_label(minutes),
        None => "时长未知".to_owned(),
    };
    serde_json::json!({
        "id": format!("{}:{}:{}", account, pool.pool_key, index.unwrap_or(0)),
        "accountId": account,
        "accountLabel": account_label(account),
        "limitId": pool.limit_id.as_deref().unwrap_or(&pool.pool_key),
        "label": quota_display_label(pool.limit_name.as_deref(), pool.limit_id.as_deref(), &pool.pool_key),
        "usedPercent": used,
        "windowMinutes": window_minutes,
        "resetsAt": resets_at,
        "observedAt": stored.observed_at,
        "status": status,
        "stale": stale,
        "detail": format!("{role_label} · {duration_label}"),
    })
}

pub(super) fn quota_duration_label(minutes: u64) -> String {
    if minutes >= 1_440 && minutes.is_multiple_of(1_440) {
        format!("{} 天", minutes / 1_440)
    } else if minutes >= 60 && minutes.is_multiple_of(60) {
        format!("{} 小时", minutes / 60)
    } else {
        format!("{minutes} 分钟")
    }
}

#[derive(Debug, Clone)]
struct QuotaObservation {
    history_limited: bool,
    snapshot_id: String,
    account: String,
    window_key: String,
    limit_id: String,
    label: String,
    role: String,
    observed_at: DateTime<Utc>,
    used_percent: Option<f64>,
    window_seconds: Option<u64>,
    resets_at: Option<DateTime<Utc>>,
}

fn quota_observations(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<QuotaObservation>, StoreError> {
    let selected_account = selected(&query.account);
    let accounts = store
        .quota_accounts()?
        .into_iter()
        .filter(|account| {
            selected_account
                .as_deref()
                .is_none_or(|selected| selected == account)
        })
        .collect::<BTreeSet<_>>();
    let mut observations = Vec::new();
    for account in accounts {
        if let Some((windows, history_limited)) =
            store.indexed_quota_window_preview(&account, 1000)?
        {
            for window in windows {
                observations.push(QuotaObservation {
                    history_limited,
                    snapshot_id: window.snapshot_id,
                    account: account.clone(),
                    window_key: window.stream_key,
                    label: quota_display_label(
                        window.limit_name.as_deref(),
                        Some(&window.limit_id),
                        &window.pool_key,
                    ),
                    limit_id: window.limit_id,
                    role: window.role,
                    observed_at: window.observed_at,
                    used_percent: window.used_percent,
                    window_seconds: window.window_seconds,
                    resets_at: window
                        .resets_at_unix
                        .and_then(|at| Utc.timestamp_opt(at, 0).single()),
                });
            }
            continue;
        }
        let mut snapshots = store.list_quota_snapshots(&account, 1_000)?;
        let history_limited = snapshots.len() == 1_000;
        snapshots.reverse();
        for stored in snapshots {
            for pool in stored.snapshot.pools {
                let limit_id = pool
                    .limit_id
                    .clone()
                    .unwrap_or_else(|| pool.pool_key.clone());
                let label = quota_display_label(
                    pool.limit_name.as_deref(),
                    pool.limit_id.as_deref(),
                    &pool.pool_key,
                );
                for window in &pool.windows {
                    let role = window.role.as_str().to_owned();
                    let window_key = crate::quota::window_stream_key(&pool, window);
                    observations.push(QuotaObservation {
                        history_limited,
                        snapshot_id: stored.snapshot_id.clone(),
                        account: account.clone(),
                        window_key,
                        limit_id: limit_id.clone(),
                        label: label.clone(),
                        role,
                        observed_at: stored.observed_at,
                        used_percent: window.used_percent,
                        window_seconds: window.window_seconds,
                        resets_at: window
                            .resets_at_unix
                            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
                    });
                }
            }
        }
    }
    observations.sort_by_key(|observation| observation.observed_at);
    Ok(observations)
}

fn quota_window_kind(window_seconds: Option<u64>) -> &'static str {
    match window_seconds {
        Some(seconds) if (6 * 86_400..=8 * 86_400).contains(&seconds) => "weekly",
        Some(seconds) if seconds <= 86_400 => "short",
        Some(_) => "custom",
        None => "unknown",
    }
}

pub(super) fn quota_cycle_views(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    use crate::quota::cycles::{WindowSample, segments};
    let now = query.reference_time.unwrap_or_else(Utc::now);
    let observations = quota_observations(store, query)?;
    let mut by_window = BTreeMap::<(String, String), Vec<QuotaObservation>>::new();
    for observation in observations {
        if observation.observed_at > now {
            continue;
        }
        by_window
            .entry((observation.account.clone(), observation.window_key.clone()))
            .or_default()
            .push(observation);
    }
    let (filter, _) = filter_and_period(query, DataQuality::Confirmed);
    let mut candidates = Vec::new();
    for ((account, window_key), observations) in &by_window {
        let samples = observations
            .iter()
            .map(|item| WindowSample {
                at: item.observed_at.timestamp_millis(),
                reset: item.resets_at.map(|at| at.timestamp_millis()),
                seconds: item.window_seconds,
                used: item.used_percent,
            })
            .collect::<Vec<_>>();
        for segment in segments(&samples) {
            let first = &observations[segment.samples.start];
            let latest = &observations[segment.samples.end - 1];
            let planned_start =
                latest
                    .resets_at
                    .zip(latest.window_seconds)
                    .and_then(|(reset, seconds)| {
                        i64::try_from(seconds).ok().and_then(|seconds| {
                            ChronoDuration::try_seconds(seconds)
                                .and_then(|duration| reset.checked_sub_signed(duration))
                        })
                    });
            let mut start = planned_start.map_or(first.observed_at, |at| at.max(first.observed_at));
            let mut end = latest.resets_at.map_or(now, |at| at.min(now));
            if let Some(next) = observations.get(segment.samples.end) {
                // Only a reported deadline lying between these observations can
                // close the old interval beyond its last observation. Otherwise
                // preserve the uncertain boundary gap instead of assigning it.
                let boundary_end = latest
                    .resets_at
                    .filter(|at| *at >= latest.observed_at && *at <= next.observed_at)
                    .unwrap_or(latest.observed_at);
                end = end.min(boundary_end);
            }
            if let Some(at) = filter.start_inclusive {
                start = start.max(at);
            }
            if let Some(at) = filter.end_exclusive {
                end = end.min(at);
            }
            if start > end
                || filter.start_inclusive.is_some_and(|at| end <= at)
                || filter.end_exclusive.is_some_and(|at| start >= at)
            {
                continue;
            }
            candidates.push((
                account,
                window_key,
                first,
                latest,
                segment,
                planned_start,
                start,
                end,
            ));
        }
    }
    candidates.sort_by_key(|(_, _, first, _, _, _, _, _)| std::cmp::Reverse(first.observed_at));
    let more_segments = candidates.len() > 20;
    let mut cycles = Vec::new();
    for (account, window_key, first, latest, segment, planned_start, local_start, local_end) in
        candidates.into_iter().take(20)
    {
        // This is account activity in an observed interval, NOT pool ownership.
        // Preserve exact edges; never allocate hourly totals proportionally.
        let local_usage = super::queries::aggregate_exact_hour_window(
            store,
            &AggregateFilter {
                start_inclusive: Some(local_start),
                end_exclusive: Some(local_end),
                account_fingerprint: Some(account.clone()),
                project_id: selected(&query.project),
                model: selected(&query.model),
                quality: Some(DataQuality::Confirmed),
            },
        )?;
        cycles.push(serde_json::json!({
            "id": format!("{}:{}:{}", account, window_key, first.snapshot_id),
            "accountId": account,
            "accountLabel": account_label(&latest.account),
            "limitId": latest.limit_id,
            "label": latest.label,
            "role": latest.role,
            "windowKind": quota_window_kind(latest.window_seconds),
            "windowMinutes": latest.window_seconds.map(|seconds| seconds / 60),
            "cycleStart": planned_start,
            "cycleEnd": latest.resets_at,
            "firstObservedAt": first.observed_at,
            "lastObservedAt": latest.observed_at,
            "firstUsedPercent": first.used_percent,
            "usedPercent": latest.used_percent,
            "usedDeltaPercent": first.used_percent.zip(latest.used_percent).map(|(a,b)| b-a),
            "sampleCount": segment.samples.len(),
            "localObservationStart": local_start,
            "localObservationEnd": local_end,
            "boundaryKind": segment.boundary.code(),
            "boundaryAfter": segment.boundary_after.and_then(DateTime::<Utc>::from_timestamp_millis),
            "historyLimited": more_segments || latest.history_limited,
            "localCoverageRatio": null,
            "localUsage": token_value(local_usage.usage),
            "localEvents": local_usage.event_count,
            "localUsageResolution": "hour",
            "empiricalTokensPerUsedPercent": null,
            "empiricalRatioIsConversion": false,
        }));
    }
    Ok(cycles)
}

pub(super) fn quota_reset_events(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let observations = quota_observations(store, query)?;
    let mut by_window = BTreeMap::<(String, String), Vec<QuotaObservation>>::new();
    for observation in observations {
        by_window
            .entry((observation.account.clone(), observation.window_key.clone()))
            .or_default()
            .push(observation);
    }
    let mut events = Vec::new();
    for (_, mut observations) in by_window {
        observations.sort_by_key(|observation| observation.observed_at);
        let Some(mut high_water) = observations
            .iter()
            .find(|observation| observation.used_percent.is_some())
            .cloned()
        else {
            continue;
        };
        for (index, current) in observations.iter().enumerate().skip(1) {
            let (Some(before), Some(after)) = (high_water.used_percent, current.used_percent)
            else {
                continue;
            };
            if after >= before {
                high_water = current.clone();
                continue;
            }
            let drop = before - after;
            let boundary_changed = high_water.resets_at != current.resets_at;
            if drop < 5.0 || !boundary_changed {
                continue;
            }
            let confirmation_deadline = current.observed_at + ChronoDuration::minutes(10);
            let confirmed = observations.iter().skip(index + 1).any(|next| {
                next.observed_at <= confirmation_deadline
                    && next.resets_at == current.resets_at
                    && next.used_percent.is_some_and(|value| value <= after + 2.0)
            });
            if !confirmed {
                continue;
            }
            let near_scheduled = high_water.resets_at.is_some_and(|reset| {
                current.observed_at >= reset - ChronoDuration::minutes(10)
                    && current.observed_at <= reset + ChronoDuration::minutes(30)
            });
            events.push(serde_json::json!({
                "id": format!("quota-reset-observed-{}-{}-{}", current.account, current.window_key, current.observed_at.timestamp_millis()),
                "at": current.observed_at,
                "kind": "quota_reset",
                "accountId": current.account,
                "title": if near_scheduled { "额度周期到期重置" } else { "观察到官方提前重置" },
                "detail": format!("{} · {:.1}% → {:.1}% · {}", current.label, before, after, if near_scheduled { "符合计划窗口" } else { "触发来源无法由本机证明（可能来自官方或外部重置）" }),
                "confidence": "verified",
                "resetClass": if near_scheduled { "scheduled_rollover" } else { "observed_official_reset" },
                "limitId": current.limit_id,
                "windowKind": quota_window_kind(current.window_seconds),
                "previousResetAt": high_water.resets_at,
                "nextResetAt": current.resets_at,
                "beforeUsedPercent": before,
                "afterUsedPercent": after,
            }));
            high_water = current.clone();
        }
    }
    Ok(events)
}

pub(super) fn timeline_views(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let (_, period) = filter_and_period(query, DataQuality::Confirmed);
    let selected_account = selected(&query.account);
    let mut previous: BTreeMap<(String, String), Option<String>> = BTreeMap::new();
    let mut events = Vec::new();
    for row in store.auth_timeline_rows()? {
        let parsed_at = parse_timestamp(&row.observed_from);
        let in_period = parsed_at.is_none_or(|at| {
            period.start.is_none_or(|start| at >= start) && period.end.is_none_or(|end| at < end)
        });
        let key = (row.machine_id, row.source_id);
        let old = previous
            .insert(key, row.account_fingerprint.clone())
            .flatten();
        if old != row.account_fingerprint
            && old.is_some()
            && in_period
            && selected_account
                .as_deref()
                .is_none_or(|selected| row.account_fingerprint.as_deref() == Some(selected))
        {
            events.push(serde_json::json!({
                "id": format!("account-switch-{}", row.epoch_id),
                "at": row.observed_from,
                "kind": "account_switch",
                "accountId": row.account_fingerprint,
                "title": "账号切换",
                "detail": format!("{} → {}", old.as_deref().map(account_label).unwrap_or_else(|| "unknown".into()), row.account_fingerprint.as_deref().map(account_label).unwrap_or_else(|| "unknown".into())),
                "confidence": row.confidence,
            }));
        }
    }
    for event in quota_reset_events(store, query)? {
        let in_period = event
            .get("at")
            .and_then(|value| value.as_str())
            .and_then(parse_timestamp)
            .is_some_and(|at| {
                period.start.is_none_or(|start| at >= start)
                    && period.end.is_none_or(|end| at < end)
            });
        if in_period {
            events.push(event);
        }
    }
    for quota in quota_views(store, query)? {
        if let Some(resets_at) = quota
            .get("resetsAt")
            .and_then(|value| value.as_str())
            .and_then(parse_timestamp)
            .filter(|value| *value > Utc::now())
        {
            events.push(serde_json::json!({
                "id": format!("quota-reset-scheduled-{}", quota.get("id").and_then(|value| value.as_str()).unwrap_or("unknown")),
                "at": resets_at,
                "kind": "quota_reset_scheduled",
                "accountId": quota.get("accountId").cloned().unwrap_or(serde_json::Value::Null),
                "title": "计划额度重置",
                "detail": format!("{} · 官方当前计划时间", quota.get("label").and_then(|value| value.as_str()).unwrap_or("quota")),
                "confidence": "verified",
            }));
        }
    }
    events.sort_by(|left, right| {
        left.get("at")
            .and_then(|value| value.as_str())
            .cmp(&right.get("at").and_then(|value| value.as_str()))
    });
    Ok(events)
}
