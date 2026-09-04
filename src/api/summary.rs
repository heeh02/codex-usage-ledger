use super::*;

pub(super) fn http_summary(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    let (base, period) = filter_and_period(query, DataQuality::Confirmed);
    let confirmed = aggregate_selected_period(store, query, &base, &period)?;
    let quarantined = aggregate_for_quality(store, &base, DataQuality::Quarantined)?;
    let unknown = aggregate_for_quality(store, &base, DataQuality::Unknown)?;
    let previous =
        if let (Some(start), Some(end)) = (period.comparison_start, period.comparison_end) {
            let mut previous_filter = base.clone();
            previous_filter.start_inclusive = Some(start);
            previous_filter.end_exclusive = Some(end);
            Some(store.aggregate_rollup_usage(&previous_filter)?)
        } else {
            None
        };
    let cache_rate = if confirmed.usage.input_tokens == 0 {
        0.0
    } else {
        confirmed.usage.cached_input_tokens as f64 / confirmed.usage.input_tokens as f64
    };
    let previous_total = previous
        .as_ref()
        .map(|aggregate| aggregate.usage.total_tokens)
        .unwrap_or_default();
    let delta_tokens = i128::from(confirmed.usage.total_tokens) - i128::from(previous_total);
    let delta_percent = (previous_total > 0).then(|| delta_tokens as f64 / previous_total as f64);
    let elapsed_days = period
        .start
        .map(|start| {
            period
                .end
                .unwrap_or_else(Utc::now)
                .signed_duration_since(start)
                .num_seconds()
                .max(1) as f64
                / 86_400.0
        })
        .unwrap_or(1.0)
        .max(1.0);
    let evidence_events = confirmed
        .event_count
        .saturating_add(quarantined.event_count)
        .saturating_add(unknown.event_count);
    let match_rate = if evidence_events == 0 {
        1.0
    } else {
        confirmed.event_count as f64 / evidence_events as f64
    };
    // Account totals are a stable account/time metric. Project, model, session,
    // and selected local token dimensions must never change their definition.
    let mut account_query = query.clone();
    account_query.project = None;
    account_query.model = None;
    account_query.session = None;
    account_query.metric = Some("total".to_owned());
    let official = official_usage_view(store, &account_query, &period)?;
    let official_total = official
        .get("totalTokens")
        .and_then(serde_json::Value::as_u64);
    let official_coverage_complete = official
        .get("coverageComplete")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let local_coverage_complete = period
        .start
        .zip(earliest_event_at(store)?)
        .is_none_or(|(requested, earliest)| requested >= earliest);
    let reconciliation_comparable = official_coverage_complete
        && local_coverage_complete
        && official
            .get("authoritativeForAccountTotal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    let attribution_gap = reconciliation_comparable.then(|| {
        official_total
            .unwrap_or_default()
            .saturating_sub(confirmed.usage.total_tokens)
    });
    let account_total_metric = resolved_account_total_metric(query, &period, &official);
    let local_attributed_metric = ResolvedMetric {
        value: Some(confirmed.usage.total_tokens),
        source: MetricSource::Local,
        status: MetricStatus::LocalSample,
        window_start: period.start,
        window_end: period.end,
        timezone: period.timezone.clone(),
        account_scope: selected(&query.account).unwrap_or_else(|| "all".to_owned()),
        machine_scope: "this_machine".to_owned(),
        coverage: MetricCoverage {
            complete: local_coverage_complete,
            ratio: if local_coverage_complete { 1.0 } else { 0.0 },
            known_account_count: official
                .get("knownAccountCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            missing_official_account_count: 0,
        },
        definition_id: "local_attributed_total_v1".to_owned(),
    };
    let attribution_coverage =
        project_attribution_coverage(store, query, &period, &official, confirmed.usage)?;
    let missing_account_estimate = missing_account_estimate(store, query, &period)?;
    Ok(serde_json::json!({
        "generatedAt": Utc::now(),
        "mode": "http",
        "period": period_value(store, &period),
        "filters": filter_catalog(store)?,
        "usage": quality_usage_value(confirmed.usage, quarantined.usage, unknown.usage),
        "official": official,
        "attributionCoverage": attribution_coverage,
        "missingAccountEstimate": missing_account_estimate,
        "metrics": {
            "accountTotal": account_total_metric,
            "localAttributedTotal": local_attributed_metric,
        },
        "reconciliation": {
            "comparable": reconciliation_comparable,
            "officialTotalTokens": official_total,
            "localAttributedTokens": confirmed.usage.total_tokens,
            "attributionGapTokens": attribution_gap,
            "localCoverageComplete": local_coverage_complete,
            "reason": if reconciliation_comparable { "same covered period" } else { "official freshness, local coverage, or dimensional scope differs" },
        },
        "confirmedEvents": confirmed.event_count,
        "cacheRate": cache_rate,
        "comparison": {
            "usage": previous.as_ref().map(|aggregate| token_value(aggregate.usage)).unwrap_or_else(|| token_value(TokenUsage::default())),
            "previousEvents": previous.as_ref().map(|aggregate| aggregate.event_count).unwrap_or_default(),
            "deltaTokens": delta_tokens,
            "deltaPercent": delta_percent,
            "available": period.comparison_start.is_some(),
        },
        "averagePerDay": confirmed.usage.total_tokens as f64 / elapsed_days,
        "matchRate": match_rate,
        "unmatchedEvents": unknown.event_count,
        "latestConfirmedAt": latest_confirmed_at(store)?,
        "quotaPools": quota_views(store, query)?,
        "quotaCycles": quota_cycle_views(store, query)?,
    }))
}

pub(super) fn project_attribution_coverage(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &PeriodDescriptor,
    official: &serde_json::Value,
    selected_local_usage: TokenUsage,
) -> Result<serde_json::Value, StoreError> {
    let mut scope_query = query.clone();
    scope_query.project = None;
    scope_query.model = None;
    scope_query.session = None;
    scope_query.metric = Some("total".to_owned());
    let (confirmed_filter, _) = filter_and_period(&scope_query, DataQuality::Confirmed);
    let local = aggregate_selected_period(store, &scope_query, &confirmed_filter, period)?;
    let projects = aggregate_selected_period_by(
        store,
        &scope_query,
        AggregateDimension::Project,
        &confirmed_filter,
    )?;
    let unassigned_tokens = projects
        .iter()
        .filter(|bucket| bucket.key.as_deref() == Some(UNASSIGNED_PROJECT_ID))
        .map(|bucket| bucket.usage.total_tokens)
        .fold(0_u64, u64::saturating_add);
    let standalone_conversation_tokens = projects
        .iter()
        .filter(|bucket| bucket.key.as_deref() == Some(STANDALONE_PROJECT_ID))
        .map(|bucket| bucket.usage.total_tokens)
        .fold(0_u64, u64::saturating_add);
    let standalone = standalone_conversation_stats(store)?;
    let named_project_tokens = projects
        .iter()
        .filter(|bucket| {
            bucket
                .key
                .as_deref()
                .is_some_and(|key| key != STANDALONE_PROJECT_ID && key != UNASSIGNED_PROJECT_ID)
        })
        .map(|bucket| bucket.usage.total_tokens)
        .fold(0_u64, u64::saturating_add);
    let display_total = official
        .get("displayTotalTokens")
        .and_then(serde_json::Value::as_u64);
    let official_base = official
        .get("totalTokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let local_complement = official
        .get("localComplementTokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let classified_local_tokens = named_project_tokens
        .saturating_add(standalone_conversation_tokens)
        .saturating_add(unassigned_tokens);
    let unattributed_tokens = local
        .usage
        .total_tokens
        .saturating_sub(classified_local_tokens);
    let coverage_ratio = (local.usage.total_tokens > 0).then(|| {
        (classified_local_tokens as f64 / local.usage.total_tokens as f64).clamp(0.0, 1.0)
    });

    let timezone = Tz::from_str(&period.timezone).unwrap_or(chrono_tz::Asia::Shanghai);
    let (start_day, end_day) = official_day_bounds(period.start, period.end, timezone);
    let selected_account = selected(&scope_query.account);
    let official_accounts = if let Some(account) = selected_account {
        vec![account]
    } else {
        store.list_official_accounts()?
    };
    let mut official_by_day = BTreeMap::<String, u64>::new();
    for account in official_accounts {
        for bucket in
            store.official_daily_usage(&account, start_day.as_deref(), end_day.as_deref())?
        {
            let entry = official_by_day.entry(bucket.start_date).or_default();
            *entry = entry.saturating_add(bucket.tokens);
        }
    }

    let mut evidence_filter = confirmed_filter.clone();
    evidence_filter.quality = None;
    let evidence_days = store
        .aggregate_rollup_by(AggregateDimension::Day, &evidence_filter)?
        .into_iter()
        .filter_map(|bucket| bucket.key.filter(|_| bucket.event_count > 0))
        .collect::<BTreeSet<_>>();
    let local_days = store
        .aggregate_rollup_by(AggregateDimension::Day, &confirmed_filter)?
        .into_iter()
        .filter_map(|bucket| bucket.key.map(|day| (day, bucket.usage.total_tokens)))
        .collect::<HashMap<_, _>>();
    let first_evidence_day = evidence_days.iter().next().cloned();
    let mut official_before_local_evidence = 0_u64;
    let mut official_without_local_evidence = 0_u64;
    for (day, tokens) in &official_by_day {
        if first_evidence_day.as_ref().is_none_or(|first| day < first) {
            official_before_local_evidence = official_before_local_evidence.saturating_add(*tokens);
        } else if !evidence_days.contains(day) {
            official_without_local_evidence =
                official_without_local_evidence.saturating_add(*tokens);
        }
    }
    let gap = display_total
        .map(|total| i128::from(total) - i128::from(local.usage.total_tokens))
        .unwrap_or_default();
    let same_day_net_gap = gap
        - i128::from(official_before_local_evidence)
        - i128::from(official_without_local_evidence);
    let local_on_official_days = official_by_day
        .keys()
        .map(|day| local_days.get(day).copied().unwrap_or_default())
        .fold(0_u64, u64::saturating_add);
    let official_window_start = official_by_day.keys().next().cloned();
    let official_window_through = official_by_day.keys().next_back().cloned();

    Ok(serde_json::json!({
        "definitionId": "project_attribution_coverage_v1",
        "accountTotalTokens": display_total,
        "officialBaseTokens": official_base,
        "localComplementTokens": local_complement,
        "localAttributedTokens": local.usage.total_tokens,
        "selectedLocalTokens": selected_local_usage.total_tokens,
        "namedProjectTokens": named_project_tokens,
        "unassignedTokens": unassigned_tokens,
        "standaloneConversationTokens": standalone_conversation_tokens,
        "standaloneConversations": {
            "current": standalone.current,
            "historical": standalone.historical,
            "indexed": standalone.current.saturating_add(standalone.historical),
            "withLocalEvidence": standalone.with_local_evidence,
        },
        "unattributedTokens": unattributed_tokens,
        "coverageRatio": coverage_ratio,
        "officialWindowStart": official_window_start,
        "officialWindowThrough": official_window_through,
        "localWindowStart": first_evidence_day,
        "localWindowThrough": evidence_days.iter().next_back().cloned(),
        "officialDayCount": official_by_day.len(),
        "localEvidenceDayCount": evidence_days.len(),
        "localOnOfficialDays": local_on_official_days,
        "gapBuckets": [
            {
                "id": "official_before_local_evidence",
                "label": "官方早于本机证据",
                "tokens": official_before_local_evidence,
                "detail": "官方账号记录早于本机最早可归因事件",
            },
            {
                "id": "official_days_without_local_evidence",
                "label": "无本机采样证据的官方日期",
                "tokens": official_without_local_evidence,
                "detail": "这些日期官方有用量，但本机账本没有任何 sampling 事件",
            },
            {
                "id": "overlap_and_unbucketed_gap",
                "label": "其余净差",
                "tokens": same_day_net_gap,
                "detail": "含同日账号/本机差额、官方未分桶修正与本机尾部校正，不能强行分给项目",
            }
        ],
        "canAllocateGapToProjects": false,
        "scope": "official_account_total_vs_this_machine_attribution",
    }))
}
