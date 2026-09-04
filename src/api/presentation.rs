use super::*;

pub(super) fn add_usage_saturating(total: &mut TokenUsage, next: TokenUsage) {
    total.input_tokens = total.input_tokens.saturating_add(next.input_tokens);
    total.cached_input_tokens = total
        .cached_input_tokens
        .saturating_add(next.cached_input_tokens);
    total.cache_write_input_tokens = total
        .cache_write_input_tokens
        .saturating_add(next.cache_write_input_tokens);
    total.cache_write_observed_input_tokens = total
        .cache_write_observed_input_tokens
        .saturating_add(next.cache_write_observed_input_tokens);
    total.output_tokens = total.output_tokens.saturating_add(next.output_tokens);
    total.reasoning_output_tokens = total
        .reasoning_output_tokens
        .saturating_add(next.reasoning_output_tokens);
    total.total_tokens = total.total_tokens.saturating_add(next.total_tokens);
}

pub(super) fn filter_and_period(
    query: &UsageQuery,
    quality: DataQuality,
) -> (AggregateFilter, PeriodDescriptor) {
    let (start, end, period) = resolve_period(query);
    (
        AggregateFilter {
            start_inclusive: start,
            end_exclusive: end,
            account_fingerprint: selected(&query.account),
            project_id: selected(&query.project),
            model: selected(&query.model),
            quality: Some(quality),
        },
        period,
    )
}

pub(super) fn selected(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_owned)
}

pub(super) fn token_value(usage: TokenUsage) -> serde_json::Value {
    serde_json::json!({
        "input": usage.input_tokens,
        "cached": usage.cached_input_tokens,
        "cacheWrite": usage.cache_write_input_tokens,
        "cacheWriteObservedInput": usage.cache_write_observed_input_tokens,
        "cacheWriteCoverage": usage.cache_write_coverage(),
        "uncached": usage.uncached_input_tokens(),
        "output": usage.output_tokens,
        "reasoning": usage.reasoning_output_tokens,
        "total": usage.total_tokens,
    })
}

pub(super) fn quality_usage_value(
    confirmed: TokenUsage,
    quarantined: TokenUsage,
    unknown: TokenUsage,
) -> serde_json::Value {
    serde_json::json!({
        "confirmed": token_value(confirmed),
        "quarantined": token_value(quarantined),
        "unknown": token_value(unknown),
    })
}

pub(super) fn quality_state_value(
    state: &str,
    aggregate: UsageAggregate,
    description: &str,
    token_count_known: bool,
) -> serde_json::Value {
    serde_json::json!({
        "state": state,
        "eventCount": aggregate.event_count,
        "tokenCount": token_count_known.then_some(aggregate.usage.total_tokens),
        "usage": token_value(aggregate.usage),
        "description": description,
    })
}

pub(super) fn period_value(store: &LedgerStore, period: &PeriodDescriptor) -> serde_json::Value {
    let coverage_start = earliest_event_at(store).ok().flatten();
    let start = period.start.or(coverage_start);
    let end = period.end.unwrap_or_else(Utc::now);
    let timezone = Tz::from_str(&period.timezone).unwrap_or(chrono_tz::Asia::Shanghai);
    let window_kind = match period.label.as_str() {
        "today" | "week" | "month" | "weeks12" | "months12" => "calendar",
        "rolling7" | "rolling30" => "rolling",
        _ => "lifetime",
    };
    let label = match period.label.as_str() {
        "today" => "今日",
        "week" => "本周",
        "rolling7" => "近7天",
        "month" => "本月",
        "rolling30" => "近30天",
        "weeks12" => "12周",
        "months12" => "12月",
        _ => "至今",
    };
    let definition = match period.label.as_str() {
        "today" => "本地时间 00:00 至今",
        "week" => "本周一 00:00 至今",
        "rolling7" => "当前时间向前 7×24 小时",
        "month" => "本月 1 日 00:00 至今",
        "rolling30" => "当前时间向前 30×24 小时",
        "weeks12" => "含当前周的最近 12 个自然周",
        "months12" => "含当前月的最近 12 个自然月",
        _ => "可信数据覆盖起点至今",
    };
    let coverage_complete = period
        .start
        .zip(coverage_start)
        .is_none_or(|(requested, coverage)| requested >= coverage);
    let comparison_available = period
        .comparison_start
        .zip(coverage_start)
        .is_some_and(|(comparison, coverage)| comparison >= coverage);
    let coverage_offset = period
        .start
        .zip(coverage_start)
        .map(|(requested, coverage)| {
            let total = end
                .signed_duration_since(requested)
                .num_milliseconds()
                .max(1) as f64;
            let missing = coverage
                .signed_duration_since(requested)
                .num_milliseconds()
                .clamp(0, total as i64) as f64;
            (missing / total).clamp(0.0, 1.0)
        })
        .unwrap_or(0.0);
    serde_json::json!({
        "key": period.label,
        "label": label,
        "definition": definition,
        "start": start.map(|value| value.to_rfc3339()).unwrap_or_default(),
        "end": end.to_rfc3339(),
        "timezone": period.timezone,
        "comparisonStart": period.comparison_start.map(|value| value.to_rfc3339()),
        "comparisonEnd": period.comparison_end.map(|value| value.to_rfc3339()),
        "coverageStart": coverage_start.map(|value| value.to_rfc3339()),
        "coverageComplete": coverage_complete,
        "coverageOffset": coverage_offset,
        "coverageRatio": 1.0 - coverage_offset,
        "comparisonAvailable": comparison_available,
        "partial": period.partial,
        "defaultGrain": period.default_grain,
        "windowKind": window_kind,
        "crossesMonth": window_crosses_month(start, end, timezone),
        "crossesYear": window_crosses_year(start, end, timezone),
    })
}

pub(super) fn window_crosses_month(
    start: Option<DateTime<Utc>>,
    end: DateTime<Utc>,
    timezone: Tz,
) -> bool {
    start.is_some_and(|start| {
        let start = start.with_timezone(&timezone);
        let end = end.with_timezone(&timezone);
        start.year() != end.year() || start.month() != end.month()
    })
}

pub(super) fn window_crosses_year(
    start: Option<DateTime<Utc>>,
    end: DateTime<Utc>,
    timezone: Tz,
) -> bool {
    start.is_some_and(|start| {
        start.with_timezone(&timezone).year() != end.with_timezone(&timezone).year()
    })
}

pub(super) fn earliest_event_at(store: &LedgerStore) -> Result<Option<DateTime<Utc>>, StoreError> {
    let value = store.earliest_rollup_day()?;
    Ok(value.and_then(|value| parse_timestamp(&format!("{value}T00:00:00+08:00"))))
}

pub(super) fn latest_confirmed_at(store: &LedgerStore) -> Result<Option<String>, StoreError> {
    store.latest_confirmed_evidence_at()
}

pub(super) fn filter_catalog(store: &LedgerStore) -> Result<serde_json::Value, StoreError> {
    let all = AggregateFilter {
        quality: None,
        ..Default::default()
    };
    let registry = account_registry(store)?;
    let all_accounts_label = format!(
        "全部账号 · 已捕获 {}/{}",
        registry.observed_count(),
        registry.expected_count()
    );
    let account_ids = registry.observed();
    let active_account = store.active_account_fingerprint()?;
    let official_accounts = store
        .list_official_accounts()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut account_ids = account_ids.into_iter().collect::<Vec<_>>();
    account_ids.sort_by_key(|id| (active_account.as_deref() != Some(id.as_str()), id.clone()));
    let accounts = account_ids
        .into_iter()
        .map(|id| {
            let prefix = if active_account.as_deref() == Some(id.as_str()) {
                "当前账号"
            } else if official_accounts.contains(&id) {
                "已校准账号"
            } else if registry.provisional.contains(&id) {
                "历史账号待校准"
            } else {
                "待校准账号"
            };
            serde_json::json!({"id": id, "label": format!("{prefix} · {}", account_label(&id))})
        })
        .collect::<Vec<_>>();
    let models = store
        .aggregate_rollup_by(AggregateDimension::Model, &all)?
        .into_iter()
        .filter_map(|bucket| bucket.key)
        .map(|id| serde_json::json!({"id": id, "label": id}))
        .collect::<Vec<_>>();
    let mut projects = store
        .list_projects()?
        .into_iter()
        .map(|project| serde_json::json!({"id": project.project_id, "label": project.project_name}))
        .collect::<Vec<_>>();
    let standalone = standalone_conversation_stats(store)?;
    if standalone.current.saturating_add(standalone.historical) > 0 {
        projects.insert(
            0,
            serde_json::json!({"id": STANDALONE_PROJECT_ID, "label": STANDALONE_PROJECT_LABEL}),
        );
    }
    let unmatched = store.aggregate_rollup_usage(&AggregateFilter {
        project_id: Some(UNASSIGNED_PROJECT_ID.to_owned()),
        quality: None,
        ..AggregateFilter::default()
    })?;
    if unmatched.event_count > 0 {
        projects.push(
            serde_json::json!({"id": UNASSIGNED_PROJECT_ID, "label": UNASSIGNED_PROJECT_LABEL}),
        );
    }

    Ok(serde_json::json!({
        "accounts": prepend_all(&all_accounts_label, accounts),
        "projects": prepend_all("全部项目与对话", projects),
        "models": prepend_all("全部模型", models),
        "periods": [
            {"id": "today", "label": "今日"},
            {"id": "week", "label": "本周"},
            {"id": "rolling7", "label": "近7天"},
            {"id": "month", "label": "本月"},
            {"id": "rolling30", "label": "近30天"},
            {"id": "weeks12", "label": "12周"},
            {"id": "months12", "label": "12月"},
            {"id": "lifetime", "label": "至今"},
        ],
    }))
}

fn prepend_all(label: &str, mut values: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    values.insert(0, serde_json::json!({"id": "all", "label": label}));
    values
}

pub(super) fn breakdown_rows(
    store: &LedgerStore,
    query: &UsageQuery,
    dimension: AggregateDimension,
) -> Result<Vec<serde_json::Value>, StoreError> {
    #[derive(Default)]
    struct BucketQuality {
        confirmed: TokenUsage,
        quarantined: TokenUsage,
        unknown: TokenUsage,
        confirmed_events: u64,
    }

    let mut values: BTreeMap<String, BucketQuality> = BTreeMap::new();
    for quality in [
        DataQuality::Confirmed,
        DataQuality::Quarantined,
        DataQuality::Unknown,
    ] {
        let (filter, _) = filter_and_period(query, quality);
        for bucket in aggregate_selected_period_by(store, query, dimension, &filter)? {
            let key = bucket.key.unwrap_or_else(|| "unknown".into());
            let value = values.entry(key).or_default();
            match quality {
                DataQuality::Confirmed => {
                    value.confirmed = bucket.usage;
                    value.confirmed_events = bucket.event_count;
                }
                DataQuality::Quarantined => value.quarantined = bucket.usage,
                DataQuality::Unknown => value.unknown = bucket.usage,
            }
        }
    }
    let confirmed_total = values
        .values()
        .map(|value| value.confirmed.total_tokens)
        .sum::<u64>();
    let project_names = store
        .list_projects()?
        .into_iter()
        .map(|project| (project.project_id, project.project_name))
        .collect::<BTreeMap<_, _>>();
    let mut rows = values
        .into_iter()
        .map(|(id, value)| {
            let label = match dimension {
                AggregateDimension::Account => account_label(&id),
                AggregateDimension::Project => {
                    project_names.get(&id).cloned().unwrap_or_else(|| {
                        if id == STANDALONE_PROJECT_ID {
                            STANDALONE_PROJECT_LABEL.into()
                        } else if id == UNASSIGNED_PROJECT_ID || id == "unknown" {
                            UNASSIGNED_PROJECT_LABEL.into()
                        } else {
                            id.clone()
                        }
                    })
                }
                _ => {
                    if id == "unknown" {
                        "未知模型".into()
                    } else {
                        id.clone()
                    }
                }
            };
            let share = if confirmed_total == 0 {
                0.0
            } else {
                value.confirmed.total_tokens as f64 / confirmed_total as f64
            };
            serde_json::json!({
                "id": id,
                "label": label,
                "usage": quality_usage_value(value.confirmed, value.quarantined, value.unknown),
                "confirmedEvents": value.confirmed_events,
                "shareOfConfirmed": share,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        let left = left
            .pointer("/usage/confirmed/total")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let right = right
            .pointer("/usage/confirmed/total")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        right.cmp(&left)
    });
    Ok(rows)
}

pub(super) fn source_health(store: &LedgerStore) -> Result<Vec<serde_json::Value>, StoreError> {
    let mut values = Vec::new();
    for row in store.source_cursor_health()? {
        let lag = parse_timestamp(&row.updated_at)
            .map(|value| Utc::now().signed_duration_since(value).num_seconds().max(0))
            .unwrap_or(i64::MAX);
        let status = if lag <= 120 {
            "fresh"
        } else if lag <= 600 {
            "delayed"
        } else {
            "offline"
        };
        values.push(serde_json::json!({
            "sourceId": format!("machine-{}", short_hash(&row.machine_id)),
            "label": format!("{} rollout files", row.file_count),
            "machineLabel": format!("Local {}", short_hash(&row.machine_id)),
            "status": status,
            "lastObservedAt": row.updated_at,
            "lagSeconds": lag,
        }));
    }
    for account in store.list_official_accounts()? {
        let Some(snapshot) = store.latest_official_account_usage(&account)? else {
            continue;
        };
        let lag = Utc::now()
            .signed_duration_since(snapshot.observed_at)
            .num_seconds()
            .max(0);
        let sync = store.official_usage_sync_state(&account)?;
        let failed = sync
            .as_ref()
            .and_then(|value| value.last_error.as_ref())
            .is_some();
        let status = if failed {
            "offline"
        } else if lag <= 900 {
            "fresh"
        } else {
            "delayed"
        };
        values.push(serde_json::json!({
            "sourceId": format!("official-{}", short_hash(&account)),
            "label": "Codex official account usage",
            "machineLabel": account_label(&account),
            "status": status,
            "lastObservedAt": snapshot.observed_at,
            "lagSeconds": lag,
        }));
    }
    Ok(values)
}

pub(super) fn account_label(value: &str) -> String {
    if value == "unknown" {
        return "未知账号".into();
    }
    let prefix = value.chars().take(8).collect::<String>();
    format!("Account {prefix}…")
}

fn short_hash(value: &str) -> String {
    let digest = sha2::Sha256::digest(value.as_bytes());
    hex::encode(digest)[..8].to_owned()
}

pub(super) fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}
