use super::*;

pub(super) fn aggregate_selected_period_by(
    store: &LedgerStore,
    query: &UsageQuery,
    dimension: AggregateDimension,
    filter: &AggregateFilter,
) -> Result<Vec<crate::store::UsageBucket>, StoreError> {
    if query.period.as_deref() == Some("rolling7") {
        aggregate_exact_hour_window_by(store, dimension, filter)
    } else {
        store.aggregate_rollup_by(dimension, filter)
    }
}

fn floor_utc_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    value
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(value)
}

fn ceil_utc_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    let floor = floor_utc_hour(value);
    if floor == value {
        floor
    } else {
        floor + ChronoDuration::hours(1)
    }
}

/// Exact rolling windows use durable hour rollups for complete hours and scan
/// raw evidence only for the two partial boundary hours.
fn aggregate_exact_hour_window_series(
    store: &LedgerStore,
    dimension: Option<AggregateDimension>,
    filter: &AggregateFilter,
    timezone: &str,
) -> Result<Vec<crate::store::UsageSeriesBucket>, StoreError> {
    let (Some(start), Some(end)) = (filter.start_inclusive, filter.end_exclusive) else {
        return store.aggregate_exact_time_series(TimeGrain::Hour, dimension, filter, timezone);
    };
    if start >= end {
        return Ok(Vec::new());
    }
    let first_complete_hour = ceil_utc_hour(start).min(end);
    let last_complete_hour = floor_utc_hour(end).max(first_complete_hour);
    let mut merged = BTreeMap::<(String, Option<String>), (u64, TokenUsage)>::new();
    let mut merge = |buckets: Vec<crate::store::UsageSeriesBucket>| {
        for bucket in buckets {
            let entry = merged
                .entry((bucket.time_key, bucket.dimension_key))
                .or_default();
            entry.0 = entry.0.saturating_add(bucket.event_count);
            add_usage_saturating(&mut entry.1, bucket.usage);
        }
    };
    if start < first_complete_hour {
        let mut boundary = filter.clone();
        boundary.start_inclusive = Some(start);
        boundary.end_exclusive = Some(first_complete_hour);
        merge(store.aggregate_exact_time_series(
            TimeGrain::Hour,
            dimension,
            &boundary,
            timezone,
        )?);
    }
    if first_complete_hour < last_complete_hour {
        let mut middle = filter.clone();
        middle.start_inclusive = Some(first_complete_hour);
        middle.end_exclusive = Some(last_complete_hour);
        merge(store.aggregate_time_series(TimeGrain::Hour, dimension, &middle)?);
    }
    if last_complete_hour < end {
        let mut boundary = filter.clone();
        boundary.start_inclusive = Some(last_complete_hour);
        boundary.end_exclusive = Some(end);
        merge(store.aggregate_exact_time_series(
            TimeGrain::Hour,
            dimension,
            &boundary,
            timezone,
        )?);
    }
    Ok(merged
        .into_iter()
        .map(
            |((time_key, dimension_key), (event_count, usage))| crate::store::UsageSeriesBucket {
                time_key,
                dimension_key,
                event_count,
                usage,
            },
        )
        .collect())
}

fn aggregate_exact_hour_window(
    store: &LedgerStore,
    filter: &AggregateFilter,
) -> Result<UsageAggregate, StoreError> {
    let mut aggregate = UsageAggregate {
        event_count: 0,
        usage: TokenUsage::default(),
    };
    for bucket in aggregate_exact_hour_window_series(store, None, filter, "Asia/Shanghai")? {
        aggregate.event_count = aggregate.event_count.saturating_add(bucket.event_count);
        add_usage_saturating(&mut aggregate.usage, bucket.usage);
    }
    Ok(aggregate)
}

fn aggregate_exact_hour_window_by(
    store: &LedgerStore,
    dimension: AggregateDimension,
    filter: &AggregateFilter,
) -> Result<Vec<crate::store::UsageBucket>, StoreError> {
    let mut grouped = BTreeMap::<Option<String>, (u64, TokenUsage)>::new();
    for bucket in
        aggregate_exact_hour_window_series(store, Some(dimension), filter, "Asia/Shanghai")?
    {
        let entry = grouped.entry(bucket.dimension_key).or_default();
        entry.0 = entry.0.saturating_add(bucket.event_count);
        add_usage_saturating(&mut entry.1, bucket.usage);
    }
    Ok(grouped
        .into_iter()
        .map(|(key, (event_count, usage))| crate::store::UsageBucket {
            key,
            event_count,
            usage,
        })
        .collect())
}

pub(super) fn aggregate_selected_period(
    store: &LedgerStore,
    query: &UsageQuery,
    filter: &AggregateFilter,
    period: &PeriodDescriptor,
) -> Result<UsageAggregate, StoreError> {
    if query.period.as_deref() == Some("rolling7") {
        return aggregate_exact_hour_window(store, filter);
    }
    if period.default_grain == "hour" {
        return store.aggregate_hourly_usage(filter);
    }
    store.aggregate_rollup_usage(filter)
}

pub(super) fn http_bundle(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    let collector = store.collector_status()?;
    let rollup = store.rollup_progress()?;
    let bundle = serde_json::json!({
        "summary": http_summary(store, query)?,
        "timeseries": http_timeseries(store, query)?,
        "breakdowns": http_breakdowns(store, query)?,
        "quality": http_quality(store, query)?,
        "explorer": http_explorer(store, query)?,
        "collection": {
            "mode": collector.mode,
            "phase": collector.phase,
            "itemsTotal": collector.items_total,
            "itemsCompleted": collector.items_completed,
            "bytesRead": collector.bytes_read,
            "eventsInserted": collector.events_inserted,
            "message": collector.message,
            "updatedAt": collector.updated_at,
            "rollupItemsTotal": rollup.target_rowid,
            "rollupItemsCompleted": rollup.last_backfilled_rowid,
            "rollupComplete": rollup.complete,
            "rawRetentionDays": crate::store::RAW_EVENT_RETENTION_DAYS,
        }
    });
    serde_json::from_value::<crate::api::wire::DashboardBundle>(bundle.clone())?;
    Ok(bundle)
}

pub(super) fn http_timeseries(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    #[derive(Default)]
    struct DayUsage {
        confirmed: TokenUsage,
        quarantined: TokenUsage,
        unknown: TokenUsage,
        confirmed_events: u64,
        quarantined_events: u64,
        unknown_events: u64,
    }

    let (base, period) = filter_and_period(query, DataQuality::Confirmed);
    let grain = query
        .grain
        .as_deref()
        .unwrap_or(period.default_grain.as_str());
    let exact_rolling_window = query.period.as_deref() == Some("rolling7");
    let mut days: BTreeMap<String, DayUsage> = BTreeMap::new();
    for quality in [
        DataQuality::Confirmed,
        DataQuality::Quarantined,
        DataQuality::Unknown,
    ] {
        let mut filter = base.clone();
        filter.quality = Some(quality);
        let buckets = if exact_rolling_window {
            let source_grain = if grain == "hour" {
                TimeGrain::Hour
            } else {
                TimeGrain::Day
            };
            aggregate_exact_hour_window_series(store, None, &filter, &period.timezone)?
                .into_iter()
                .map(|bucket| {
                    let key = if source_grain == TimeGrain::Day {
                        aggregate_date_key(&bucket.time_key, grain).unwrap_or(bucket.time_key)
                    } else {
                        bucket.time_key
                    };
                    (key, bucket.event_count, bucket.usage)
                })
                .collect::<Vec<_>>()
        } else if grain == "hour" {
            store
                .aggregate_time_series(TimeGrain::Hour, None, &filter)?
                .into_iter()
                .map(|bucket| (bucket.time_key, bucket.event_count, bucket.usage))
                .collect::<Vec<_>>()
        } else {
            store
                .aggregate_rollup_by(AggregateDimension::Day, &filter)?
                .into_iter()
                .map(|bucket| {
                    let date = bucket.key.unwrap_or_else(|| "unknown".into());
                    (
                        aggregate_date_key(&date, grain).unwrap_or(date),
                        bucket.event_count,
                        bucket.usage,
                    )
                })
                .collect::<Vec<_>>()
        };
        for (bucket_key, bucket_events, bucket_usage) in buckets {
            let entry = days.entry(bucket_key).or_default();
            match quality {
                DataQuality::Confirmed => {
                    add_usage_saturating(&mut entry.confirmed, bucket_usage);
                    entry.confirmed_events = entry.confirmed_events.saturating_add(bucket_events);
                }
                DataQuality::Quarantined => {
                    add_usage_saturating(&mut entry.quarantined, bucket_usage);
                    entry.quarantined_events =
                        entry.quarantined_events.saturating_add(bucket_events);
                }
                DataQuality::Unknown => {
                    add_usage_saturating(&mut entry.unknown, bucket_usage);
                    entry.unknown_events = entry.unknown_events.saturating_add(bucket_events);
                }
            }
        }
    }
    let points = days
        .into_iter()
        .map(|(date, usage)| {
            serde_json::json!({
                "date": date,
                "confirmed": token_value(usage.confirmed),
                "quarantined": token_value(usage.quarantined),
                "unknown": token_value(usage.unknown),
                "confirmedEvents": usage.confirmed_events,
                "quarantinedEvents": usage.quarantined_events,
                "unknownEvents": usage.unknown_events,
            })
        })
        .collect::<Vec<_>>();
    let comparison_points =
        if let (Some(start), Some(end)) = (period.comparison_start, period.comparison_end) {
            let mut comparison_filter = base.clone();
            comparison_filter.start_inclusive = Some(start);
            comparison_filter.end_exclusive = Some(end);
            comparison_filter.quality = Some(DataQuality::Confirmed);
            confirmed_series_points(store, &comparison_filter, grain, false, &period.timezone)?
        } else {
            Vec::new()
        };
    let project_series =
        local_project_series(store, &base, grain, exact_rolling_window, &period.timezone)?;
    Ok(serde_json::json!({
        "generatedAt": Utc::now(),
        "period": period_value(store, &period),
        "grain": grain,
        "points": points,
        "comparisonPoints": comparison_points,
        "projectSeries": project_series,
        "official": official_usage_view(store, query, &period)?,
        "timeline": timeline_views(store, query)?,
    }))
}

fn local_project_series(
    store: &LedgerStore,
    filter: &AggregateFilter,
    grain: &str,
    exact_window: bool,
    timezone: &str,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let source_grain = if grain == "hour" {
        TimeGrain::Hour
    } else {
        TimeGrain::Day
    };
    let mut grouped = HashMap::<String, BTreeMap<String, (u64, TokenUsage)>>::new();
    let buckets = if exact_window {
        aggregate_exact_hour_window_series(
            store,
            Some(AggregateDimension::Project),
            filter,
            timezone,
        )?
    } else {
        store.aggregate_time_series(source_grain, Some(AggregateDimension::Project), filter)?
    };
    for bucket in buckets {
        let project = bucket
            .dimension_key
            .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned());
        let time = if source_grain == TimeGrain::Day {
            aggregate_date_key(&bucket.time_key, grain).unwrap_or(bucket.time_key)
        } else {
            bucket.time_key
        };
        let entry = grouped.entry(project).or_default().entry(time).or_default();
        entry.0 = entry.0.saturating_add(bucket.event_count);
        add_usage_saturating(&mut entry.1, bucket.usage);
    }
    let names = store
        .list_projects()?
        .into_iter()
        .map(|project| (project.project_id, project.project_name))
        .collect::<HashMap<_, _>>();
    let mut series = grouped
        .into_iter()
        .map(|(project, points)| {
            let total = points
                .values()
                .map(|(_, usage)| usage.total_tokens)
                .fold(0_u64, u64::saturating_add);
            serde_json::json!({
                "id": project,
                "label": if project == STANDALONE_PROJECT_ID { STANDALONE_PROJECT_LABEL.to_owned() } else if project == UNASSIGNED_PROJECT_ID { UNASSIGNED_PROJECT_LABEL.to_owned() } else { names.get(&project).cloned().unwrap_or_else(|| project.clone()) },
                "totalTokens": total,
                "points": points.into_iter().map(|(bucket, (events, usage))| serde_json::json!({
                    "date": bucket,
                    "confirmed": token_value(usage),
                    "confirmedEvents": events,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    series.sort_by_key(|item| std::cmp::Reverse(item["totalTokens"].as_u64().unwrap_or_default()));
    Ok(series)
}

fn confirmed_series_points(
    store: &LedgerStore,
    filter: &AggregateFilter,
    grain: &str,
    exact_window: bool,
    timezone: &str,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let buckets = if exact_window {
        let source_grain = if grain == "hour" {
            TimeGrain::Hour
        } else {
            TimeGrain::Day
        };
        store
            .aggregate_exact_time_series(source_grain, None, filter, timezone)?
            .into_iter()
            .map(|bucket| {
                let key = if source_grain == TimeGrain::Day {
                    aggregate_date_key(&bucket.time_key, grain).unwrap_or(bucket.time_key)
                } else {
                    bucket.time_key
                };
                (key, bucket.event_count, bucket.usage)
            })
            .collect::<Vec<_>>()
    } else if grain == "hour" {
        store
            .aggregate_time_series(TimeGrain::Hour, None, filter)?
            .into_iter()
            .map(|bucket| (bucket.time_key, bucket.event_count, bucket.usage))
            .collect::<Vec<_>>()
    } else {
        store
            .aggregate_rollup_by(AggregateDimension::Day, filter)?
            .into_iter()
            .map(|bucket| {
                let date = bucket.key.unwrap_or_else(|| "unknown".into());
                (
                    aggregate_date_key(&date, grain).unwrap_or(date),
                    bucket.event_count,
                    bucket.usage,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut grouped = BTreeMap::<String, (u64, TokenUsage)>::new();
    for (key, events, usage) in buckets {
        let entry = grouped.entry(key).or_default();
        entry.0 = entry.0.saturating_add(events);
        add_usage_saturating(&mut entry.1, usage);
    }
    Ok(grouped
        .into_iter()
        .map(|(date, (events, usage))| {
            serde_json::json!({
                "date": date,
                "confirmed": token_value(usage),
                "confirmedEvents": events,
            })
        })
        .collect())
}

pub(super) fn http_breakdowns(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    let (_, period) = filter_and_period(query, DataQuality::Confirmed);
    Ok(serde_json::json!({
        "generatedAt": Utc::now(),
        "period": period_value(store, &period),
        "account": breakdown_rows(store, query, AggregateDimension::Account)?,
        "project": breakdown_rows(store, query, AggregateDimension::Project)?,
        "model": breakdown_rows(store, query, AggregateDimension::Model)?,
        "officialAccounts": official_account_rows(store, query)?,
    }))
}

fn official_account_rows(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let active = store.active_account_fingerprint()?;
    let mut rows = Vec::new();
    let accounts = account_registry(store)?.observed();
    let mut accounts = accounts.into_iter().collect::<Vec<_>>();
    accounts.sort_by_key(|account| (active.as_deref() != Some(account.as_str()), account.clone()));
    for account in accounts {
        let official_available = store.latest_official_account_usage(&account)?.is_some();
        let plan_type = store
            .latest_quota_snapshot(&account)?
            .and_then(|snapshot| snapshot.snapshot.plan_type);
        let period_total = |period_key: &str| -> Result<serde_json::Value, StoreError> {
            let mut account_query = query.clone();
            account_query.account = Some(account.clone());
            account_query.project = Some("all".to_owned());
            account_query.model = Some("all".to_owned());
            account_query.session = Some("all".to_owned());
            account_query.metric = Some("total".to_owned());
            account_query.period = Some(period_key.to_owned());
            let (_, descriptor) = filter_and_period(&account_query, DataQuality::Confirmed);
            compact_official_usage_view(store, &account_query, &descriptor)
        };
        let today = period_total("today")?;
        let week = period_total("week")?;
        let month = period_total("month")?;
        let lifetime = period_total("lifetime")?;
        let epoch = store.auth_epoch_summary(&account)?;
        rows.push(serde_json::json!({
            "id": account,
            "label": account_label(&account),
            "active": active.as_deref() == Some(account.as_str()),
            "officialAvailable": official_available,
            "planType": plan_type,
            "todayTokens": today.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
            "todayIsLowerBound": today.get("displayIsLowerBound").cloned().unwrap_or(serde_json::Value::Bool(true)),
            "weekTokens": week.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
            "weekIsLowerBound": week.get("displayIsLowerBound").cloned().unwrap_or(serde_json::Value::Bool(true)),
            "monthTokens": month.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
            "monthIsLowerBound": month.get("displayIsLowerBound").cloned().unwrap_or(serde_json::Value::Bool(true)),
            "lifetimeTokens": lifetime.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
            "lifetimeIsLowerBound": lifetime.get("displayIsLowerBound").cloned().unwrap_or(serde_json::Value::Bool(true)),
            "coverageStart": lifetime.get("coverageStart").cloned().unwrap_or(serde_json::Value::Null),
            "coverageThrough": lifetime.get("coverageThrough").cloned().unwrap_or(serde_json::Value::Null),
            "observedAt": lifetime.get("observedAt").cloned().unwrap_or(serde_json::Value::Null),
            "authEpochCount": epoch.count,
            "firstSeenAt": epoch.first_seen,
            "lastSeenAt": epoch.last_seen,
        }));
    }
    Ok(rows)
}

pub(super) fn http_quality(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    let (base, period) = filter_and_period(query, DataQuality::Confirmed);
    let confirmed = store.aggregate_rollup_usage(&base)?;
    let quarantined = aggregate_for_quality(store, &base, DataQuality::Quarantined)?;
    let unknown = aggregate_for_quality(store, &base, DataQuality::Unknown)?;
    let issue_start = period
        .start
        .or_else(|| earliest_event_at(store).ok().flatten())
        .unwrap_or_else(Utc::now);
    let issue_end = period.end.unwrap_or_else(Utc::now);
    let mut issues = Vec::new();
    if quarantined.event_count > 0 {
        issues.push(serde_json::json!({
            "id": "quarantined-events",
            "state": "quarantined",
            "severity": "critical",
            "title": "检测到重复历史或计数来源冲突",
            "detail": "这些记录只保留用于审计，已隔离且不会计入可信总量。",
            "eventCount": quarantined.event_count,
            "tokenCount": quarantined.usage.total_tokens,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    if unknown.event_count > 0 {
        issues.push(serde_json::json!({
            "id": "unknown-events",
            "state": "unknown",
            "severity": "warning",
            "title": "部分请求缺少可核验的 Token 明细",
            "detail": "在账号、模型和增量来源能够安全确认前，这些请求保持未知并与主数字分离。",
            "eventCount": unknown.event_count,
            "tokenCount": serde_json::Value::Null,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    let official = compact_official_usage_view(store, query, &period)?;
    let missing_accounts = official
        .get("missingOfficialAccountCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if missing_accounts > 0 {
        issues.push(serde_json::json!({
            "id": "official-account-calibration",
            "state": "unknown",
            "severity": "warning",
            "title": "部分账号尚未同步官方档案",
            "detail": format!("已从本机登录日志恢复账号边界，但仍有 {missing_accounts} 个账号需要在下次切换登录时读取官方 Total。全部账号视图不会把现有官方值冒充完整合计。"),
            "eventCount": 0,
            "tokenCount": serde_json::Value::Null,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    let provisional_identities = official
        .get("provisionalIdentityCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let provisional_tokens = official
        .get("provisionalLocalTokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if provisional_identities > 0 {
        issues.push(serde_json::json!({
            "id": "provisional-account-identities",
            "state": "unknown",
            "severity": "warning",
            "title": "存在尚未校准的历史账号身份",
            "detail": format!("{provisional_identities} 个 workspace-only 历史账号尚未重新登录校准。它们作为独立登录 epoch 计入观察账号数，其本机 Token 只作为全部账号下限；切换到对应账号后会自动合并到官方身份。"),
            "eventCount": 0,
            "tokenCount": provisional_tokens,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    let reconstruction = store.reconstruction_summary()?;
    if reconstruction.pending + reconstruction.reconstructing > 0 {
        issues.push(serde_json::json!({
            "id": "rollout-reconstruction-pending",
            "state": "unknown",
            "severity": "info",
            "title": "历史 rollout 正在增量恢复",
            "detail": format!("尚有 {} 个来源等待或正在恢复。恢复游标会跨重启保留；未完成来源不会提前混入主数字。", reconstruction.pending + reconstruction.reconstructing),
            "eventCount": 0,
            "tokenCount": serde_json::Value::Null,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    if reconstruction.unrecoverable > 0 {
        issues.push(serde_json::json!({
            "id": "rollout-reconstruction-unrecoverable",
            "state": "unknown",
            "severity": "warning",
            "title": "部分本机历史源已不可恢复",
            "detail": format!("{} 个已索引来源的 rollout 已丢失、替换或截断；它们不会使用累计线程字段猜测 Token。", reconstruction.unrecoverable),
            "eventCount": 0,
            "tokenCount": serde_json::Value::Null,
            "firstSeen": issue_start,
            "lastSeen": issue_end,
        }));
    }
    Ok(serde_json::json!({
        "generatedAt": Utc::now(),
        "trustedPolicy": "Codex account/usage/read is authoritative for account totals. Local attribution chooses Sampling or replay-safe Reconstruction per thread/day and never adds both sources.",
        "states": [
            quality_state_value("confirmed", confirmed, "Effective local attribution after thread/day source selection; not a substitute for the official account total.", true),
            quality_state_value("quarantined", quarantined, "Excluded because replay or counter provenance is ambiguous.", true),
            quality_state_value("unknown", unknown, "Post-sampling call had no safe same-thread token detail match and remains excluded.", false),
        ],
        "issues": issues,
        "sources": source_health(store)?,
        "reconstruction": {
            "pendingSources": reconstruction.pending,
            "reconstructingSources": reconstruction.reconstructing,
            "reconstructedSources": reconstruction.reconstructed,
            "unrecoverableSources": reconstruction.unrecoverable,
            "bytesProcessed": reconstruction.bytes_processed,
            "bytesTotal": reconstruction.bytes_total,
            "selectedTokens": reconstruction.selected_tokens,
        },
    }))
}
