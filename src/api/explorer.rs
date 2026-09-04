use super::*;

type CatalogCounts = DashboardCatalogCounts;

pub(super) fn http_explorer(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<serde_json::Value, StoreError> {
    let lifetime = aggregate_for_period(store, query, "lifetime")?;
    let week = aggregate_for_period(store, query, "week")?;
    let today = aggregate_for_period(store, query, "today")?;
    let selected_period =
        aggregate_for_period(store, query, query.period.as_deref().unwrap_or("week"))?;
    let mut recent_filter = period_filter(query, "lifetime");
    recent_filter.start_inclusive = Some(Utc::now() - ChronoDuration::minutes(15));
    recent_filter.end_exclusive = Some(Utc::now() + ChronoDuration::seconds(1));
    let recent = store.aggregate_usage(&recent_filter)?;
    let active_sessions = store
        .aggregate_by(AggregateDimension::Thread, &recent_filter)?
        .into_iter()
        .filter(|bucket| bucket.key.is_some())
        .count();
    let catalog_counts = catalog_counts(store, query.project.as_deref())?;
    let standalone = standalone_conversation_stats(store)?;
    let mut standalone_query = query.clone();
    standalone_query.project = Some(STANDALONE_PROJECT_ID.to_owned());
    standalone_query.session = None;
    let standalone_lifetime = aggregate_for_period(store, &standalone_query, "lifetime")?;
    let standalone_selected = aggregate_for_period(
        store,
        &standalone_query,
        query.period.as_deref().unwrap_or("week"),
    )?;
    let project_count = store.project_count()?;

    let projects = explorer_projects(store, query)?;
    let sessions = explorer_sessions(store, query)?;
    let selected_session = query
        .session
        .as_deref()
        .map(|thread_id| explorer_session_detail(store, query, thread_id))
        .transpose()?;
    let official_period = |period_key: &str| -> Result<serde_json::Value, StoreError> {
        let mut period_query = query.clone();
        period_query.period = Some(period_key.to_owned());
        let (_, descriptor) = filter_and_period(&period_query, DataQuality::Confirmed);
        compact_official_usage_view(store, &period_query, &descriptor)
    };
    let official_today = official_period("today")?;
    let official_week = official_period("week")?;
    let official_month = official_period("month")?;
    let official_lifetime = official_period("lifetime")?;
    let official_selected = official_period(query.period.as_deref().unwrap_or("week"))?;
    let ranking_window = |period_key: &str| {
        let mut period_query = query.clone();
        period_query.period = Some(period_key.to_owned());
        let (_, descriptor) = filter_and_period(&period_query, DataQuality::Confirmed);
        period_value(store, &descriptor)
    };

    Ok(serde_json::json!({
        "generatedAt": Utc::now(),
        "period": query.period.as_deref().unwrap_or("week"),
        "rankingWindows": {
            "week": ranking_window("week"),
            "month": ranking_window("month"),
            "lifetime": ranking_window("lifetime"),
        },
        "stats": {
            "projectCount": project_count,
            "sessionCount": catalog_counts.current_sessions,
            "subagentCount": catalog_counts.current_subagents,
            "orphanSubagentCount": catalog_counts.current_orphan_subagents,
            "historicalSessionCount": catalog_counts.historical_sessions,
            "historicalSubagentCount": catalog_counts.historical_subagents,
            "standaloneConversations": {
                "current": standalone.current,
                "historical": standalone.historical,
                "indexed": standalone.current.saturating_add(standalone.historical),
                "withLocalEvidence": standalone.with_local_evidence,
                "lifetimeUsage": token_value(standalone_lifetime.usage),
                "selectedPeriodUsage": token_value(standalone_selected.usage),
            },
            "lifetime": token_value(lifetime.usage),
            "week": token_value(week.usage),
            "today": token_value(today.usage),
            "selectedPeriod": token_value(selected_period.usage),
            "localRecent15Minutes": token_value(recent.usage),
            "localRecent15Events": recent.event_count,
            "official": {
                "todayTokens": official_today.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
                "weekTokens": official_week.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
                "monthTokens": official_month.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
                "selectedPeriodTokens": official_selected.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
                "lifetimeTokens": official_lifetime.get("displayTotalTokens").cloned().unwrap_or(serde_json::Value::Null),
                "peakDailyTokens": official_lifetime.get("peakDailyTokens").cloned().unwrap_or(serde_json::Value::Null),
                "coverageThrough": official_lifetime.get("coverageThrough").cloned().unwrap_or(serde_json::Value::Null),
                "observedAt": official_lifetime.get("observedAt").cloned().unwrap_or(serde_json::Value::Null),
                "backendIncludesToday": official_lifetime.get("backendIncludesToday").cloned().unwrap_or(serde_json::Value::Bool(false)),
                "accountCoverageComplete": official_lifetime.get("accountCoverageComplete").cloned().unwrap_or(serde_json::Value::Bool(false)),
                "knownAccountCount": official_lifetime.get("knownAccountCount").cloned().unwrap_or(serde_json::Value::from(0)),
                "missingOfficialAccountCount": official_lifetime.get("missingOfficialAccountCount").cloned().unwrap_or(serde_json::Value::from(0)),
                "totalIsLowerBound": official_lifetime.get("totalIsLowerBound").cloned().unwrap_or(serde_json::Value::Bool(false)),
            },
            "activeSessions": active_sessions,
            "latestConfirmedAt": latest_confirmed_at(store)?,
        },
        "projects": projects,
        "sessions": sessions,
        "selectedSession": selected_session,
    }))
}

fn aggregate_for_period(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &str,
) -> Result<UsageAggregate, StoreError> {
    let mut period_query = query.clone();
    period_query.period = Some(period.to_owned());
    let (filter, descriptor) = filter_and_period(&period_query, DataQuality::Confirmed);
    aggregate_selected_period(store, &period_query, &filter, &descriptor)
}

fn period_filter(query: &UsageQuery, period: &str) -> AggregateFilter {
    let mut query = query.clone();
    query.period = Some(period.to_owned());
    filter_and_period(&query, DataQuality::Confirmed).0
}

fn explorer_projects(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let mut projects = store
        .list_projects()?
        .into_iter()
        .map(|project| (project.project_id, project.project_name))
        .collect::<Vec<_>>();
    let mut all_projects_query = query.clone();
    all_projects_query.project = Some("all".to_owned());
    let usage_map = |period: &str| -> Result<HashMap<String, TokenUsage>, StoreError> {
        let mut period_query = all_projects_query.clone();
        period_query.period = Some(period.to_owned());
        let (filter, _) = filter_and_period(&period_query, DataQuality::Confirmed);
        Ok(aggregate_selected_period_by(
            store,
            &period_query,
            AggregateDimension::Project,
            &filter,
        )?
        .into_iter()
        .map(|bucket| {
            (
                bucket
                    .key
                    .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
                bucket.usage,
            )
        })
        .collect())
    };
    let previous_usage_map = |period: &str| -> Result<HashMap<String, TokenUsage>, StoreError> {
        let mut period_query = all_projects_query.clone();
        period_query.period = Some(period.to_owned());
        let (_, descriptor) = filter_and_period(&period_query, DataQuality::Confirmed);
        let (Some(start), Some(end)) = (descriptor.comparison_start, descriptor.comparison_end)
        else {
            return Ok(HashMap::new());
        };
        let mut filter = period_filter(&period_query, "lifetime");
        filter.start_inclusive = Some(start);
        filter.end_exclusive = Some(end);
        Ok(store
            .aggregate_rollup_by(AggregateDimension::Project, &filter)?
            .into_iter()
            .map(|bucket| {
                (
                    bucket
                        .key
                        .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
                    bucket.usage,
                )
            })
            .collect())
    };
    let period_usage = usage_map(query.period.as_deref().unwrap_or("week"))?;
    let selected_period_key = query.period.as_deref().unwrap_or("week");
    let mut selected_period_query = all_projects_query.clone();
    selected_period_query.period = Some(selected_period_key.to_owned());
    let (selected_period_filter, _) =
        filter_and_period(&selected_period_query, DataQuality::Confirmed);
    let period_events = aggregate_selected_period_by(
        store,
        &selected_period_query,
        AggregateDimension::Project,
        &selected_period_filter,
    )?
    .into_iter()
    .map(|bucket| {
        (
            bucket
                .key
                .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
            bucket.event_count,
        )
    })
    .collect::<HashMap<_, _>>();
    let (_, selected_period_descriptor) =
        filter_and_period(&all_projects_query, DataQuality::Confirmed);
    let previous_project_usage = if let (Some(start), Some(end)) = (
        selected_period_descriptor.comparison_start,
        selected_period_descriptor.comparison_end,
    ) {
        let mut previous_filter = period_filter(&all_projects_query, "lifetime");
        previous_filter.start_inclusive = Some(start);
        previous_filter.end_exclusive = Some(end);
        store
            .aggregate_rollup_by(AggregateDimension::Project, &previous_filter)?
            .into_iter()
            .map(|bucket| {
                (
                    bucket
                        .key
                        .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
                    (bucket.usage, bucket.event_count),
                )
            })
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let mut recent_filter = period_filter(&all_projects_query, "lifetime");
    recent_filter.start_inclusive = Some(Utc::now() - ChronoDuration::minutes(15));
    recent_filter.end_exclusive = Some(Utc::now() + ChronoDuration::seconds(1));
    let recent_project_usage = store
        .aggregate_by(AggregateDimension::Project, &recent_filter)?
        .into_iter()
        .map(|bucket| {
            (
                bucket
                    .key
                    .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
                (bucket.usage, bucket.event_count),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut project_sparklines = HashMap::<String, Vec<u64>>::new();
    let sparkline_buckets = if selected_period_key == "rolling7" {
        store.aggregate_exact_time_series(
            TimeGrain::Day,
            Some(AggregateDimension::Project),
            &selected_period_filter,
            query.timezone.as_deref().unwrap_or("Asia/Shanghai"),
        )?
    } else {
        store.aggregate_time_series(
            TimeGrain::Day,
            Some(AggregateDimension::Project),
            &selected_period_filter,
        )?
    };
    for bucket in sparkline_buckets {
        project_sparklines
            .entry(
                bucket
                    .dimension_key
                    .unwrap_or_else(|| UNASSIGNED_PROJECT_ID.to_owned()),
            )
            .or_default()
            .push(bucket.usage.total_tokens);
    }
    let active_cutoff = (Utc::now() - ChronoDuration::minutes(5)).to_rfc3339();
    let active_project_sessions = store.active_project_session_counts(&active_cutoff)?;
    let lifetime_usage = usage_map("lifetime")?;
    let week_usage = usage_map("week")?;
    let month_usage = usage_map("month")?;
    let week_previous_usage = previous_usage_map("week")?;
    let month_previous_usage = previous_usage_map("month")?;
    let today_usage = usage_map("today")?;
    let mut known_projects = projects
        .iter()
        .map(|(project_id, _)| project_id.clone())
        .collect::<BTreeSet<_>>();
    for project_id in lifetime_usage.keys() {
        if known_projects.insert(project_id.clone()) {
            projects.push((
                project_id.clone(),
                if project_id == STANDALONE_PROJECT_ID {
                    STANDALONE_PROJECT_LABEL.to_owned()
                } else if project_id == UNASSIGNED_PROJECT_ID {
                    UNASSIGNED_PROJECT_LABEL.to_owned()
                } else {
                    project_id.clone()
                },
            ));
        }
    }
    let standalone = standalone_conversation_stats(store)?;
    if standalone.current.saturating_add(standalone.historical) > 0
        && known_projects.insert(STANDALONE_PROJECT_ID.to_owned())
    {
        projects.push((
            STANDALONE_PROJECT_ID.to_owned(),
            STANDALONE_PROJECT_LABEL.to_owned(),
        ));
    }
    let catalog_summaries = catalog_project_summaries(store)?;

    let mut rows = Vec::with_capacity(projects.len());
    for (project_id, label) in projects {
        let (catalog_counts, last_active_at) = catalog_summaries
            .get(&project_id)
            .cloned()
            .unwrap_or_default();
        let (previous_usage, previous_events) = previous_project_usage
            .get(&project_id)
            .copied()
            .unwrap_or_default();
        let (recent_usage, recent_events) = recent_project_usage
            .get(&project_id)
            .copied()
            .unwrap_or_default();
        let sparkline = project_sparklines
            .get(&project_id)
            .map(|values| {
                values
                    .iter()
                    .rev()
                    .take(12)
                    .copied()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        rows.push(serde_json::json!({
            "id": project_id,
            "label": label,
            "kind": if project_id == STANDALONE_PROJECT_ID { "standalone_conversations" } else if project_id == UNASSIGNED_PROJECT_ID { "unmatched_records" } else { "project" },
            "sessionCount": catalog_counts.current_sessions,
            "subagentCount": catalog_counts.current_subagents,
            "orphanSubagentCount": catalog_counts.current_orphan_subagents,
            "historicalSessionCount": catalog_counts.historical_sessions,
            "historicalSubagentCount": catalog_counts.historical_subagents,
            "periodUsage": token_value(period_usage.get(&project_id).copied().unwrap_or_default()),
            "periodEvents": period_events.get(&project_id).copied().unwrap_or_default(),
            "previousPeriodUsage": token_value(previous_usage),
            "previousPeriodEvents": previous_events,
            "recent15Usage": token_value(recent_usage),
            "recent15Events": recent_events,
            "activeSessionCount": active_project_sessions.get(&project_id).copied().unwrap_or_default(),
            "sparkline": sparkline,
            "lifetimeUsage": token_value(lifetime_usage.get(&project_id).copied().unwrap_or_default()),
            "weekUsage": token_value(week_usage.get(&project_id).copied().unwrap_or_default()),
            "weekPreviousUsage": token_value(week_previous_usage.get(&project_id).copied().unwrap_or_default()),
            "monthUsage": token_value(month_usage.get(&project_id).copied().unwrap_or_default()),
            "monthPreviousUsage": token_value(month_previous_usage.get(&project_id).copied().unwrap_or_default()),
            "todayUsage": token_value(today_usage.get(&project_id).copied().unwrap_or_default()),
            "lastActiveAt": last_active_at,
        }));
    }
    rows.sort_by(|left, right| {
        let left_usage = left
            .pointer("/periodUsage/total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let right_usage = right
            .pointer("/periodUsage/total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        right_usage.cmp(&left_usage).then_with(|| {
            left.get("label")
                .and_then(serde_json::Value::as_str)
                .cmp(&right.get("label").and_then(serde_json::Value::as_str))
        })
    });
    Ok(rows)
}

fn explorer_sessions(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<Vec<serde_json::Value>, StoreError> {
    let roots = catalog_roots(
        store,
        query.project.as_deref(),
        if selected(&query.project).is_some() {
            500
        } else {
            30
        },
    )?;
    let mut filter = period_filter(query, query.period.as_deref().unwrap_or("week"));
    filter.project_id = None;
    let now = Utc::now();
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let root_ids = roots
        .iter()
        .map(|root| root.thread_id.clone())
        .collect::<Vec<_>>();
    let usage_by_root = store
        .aggregate_rollup_by_root_threads(&root_ids, &filter)?
        .into_iter()
        .map(|bucket| (bucket.root_thread_id.clone(), bucket))
        .collect::<HashMap<_, _>>();
    let node_counts = store.root_thread_member_counts(&root_ids)?;

    roots
        .into_iter()
        .map(|root| {
            let usage = usage_by_root
                .get(&root.thread_id)
                .cloned();
            let own = usage.as_ref().map(|value| value.own.usage).unwrap_or_default();
            let tree_usage = usage.as_ref().map(|value| value.tree.usage).unwrap_or_default();
            let tree_events = usage.as_ref().map(|value| value.tree.event_count).unwrap_or_default();
            let node_count = node_counts.get(&root.thread_id).copied().unwrap_or(1);
            let updated = parse_timestamp(&root.updated_at);
            Ok(serde_json::json!({
                "id": root.thread_id,
                "title": thread_label(&root),
                "model": root.model,
                "createdAt": root.created_at,
                "updatedAt": root.updated_at,
                "archived": root.archived,
                "presentInCodex": root.present_in_codex,
                "hasUserEvent": root.has_user_event,
                "subagentCount": node_count.saturating_sub(1),
                "kind": if root.parent_thread_id.is_none() && root.depth > 0 { "orphan_subagent" } else { "session" },
                "ownUsage": token_value(own),
                "treeUsage": token_value(tree_usage),
                "eventCount": tree_events,
                "active": root.present_in_codex && updated.is_some_and(|value| now - value < ChronoDuration::minutes(5)),
            }))
        })
        .collect()
}

fn explorer_session_detail(
    store: &LedgerStore,
    query: &UsageQuery,
    thread_id: &str,
) -> Result<serde_json::Value, StoreError> {
    let tree = catalog_descendants(store, thread_id)?;
    if tree.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    let ids = tree
        .iter()
        .map(|(thread, _)| thread.thread_id.clone())
        .collect::<Vec<_>>();
    let mut filter = period_filter(query, query.period.as_deref().unwrap_or("week"));
    filter.project_id = None;
    let (_, detail_period) = filter_and_period(query, DataQuality::Confirmed);
    let detail_grain = query
        .grain
        .as_deref()
        .unwrap_or(detail_period.default_grain.as_str());
    let source_grain = if detail_grain == "hour" {
        TimeGrain::Hour
    } else {
        TimeGrain::Day
    };
    let root_thread_id = tree[0].0.thread_id.clone();
    let timeline_for =
        |thread_ids: &[String]| -> Result<BTreeMap<String, (u64, TokenUsage)>, StoreError> {
            let mut timeline = BTreeMap::<String, (u64, TokenUsage)>::new();
            for bucket in
                store.aggregate_time_series_for_threads(source_grain, thread_ids, &filter)?
            {
                let key = if source_grain == TimeGrain::Day {
                    aggregate_date_key(&bucket.time_key, detail_grain).unwrap_or(bucket.time_key)
                } else {
                    bucket.time_key
                };
                let entry = timeline.entry(key).or_default();
                entry.0 = entry.0.saturating_add(bucket.event_count);
                add_usage_saturating(&mut entry.1, bucket.usage);
            }
            Ok(timeline)
        };
    let timeline = timeline_for(&ids)?;
    let own_timeline = timeline_for(std::slice::from_ref(&root_thread_id))?;
    let mut own_usage = store
        .aggregate_rollup_by_thread_ids(&ids, &filter)?
        .into_iter()
        .filter_map(|bucket| {
            bucket
                .key
                .map(|key| (key, (bucket.usage, bucket.event_count)))
        })
        .collect::<HashMap<_, _>>();
    let mut subtree_usage = ids
        .iter()
        .map(|id| {
            let usage = own_usage.get(id).map(|value| value.0).unwrap_or_default();
            (id.clone(), usage)
        })
        .collect::<HashMap<_, _>>();
    let mut subtree_events = ids
        .iter()
        .map(|id| {
            let events = own_usage.get(id).map(|value| value.1).unwrap_or_default();
            (id.clone(), events)
        })
        .collect::<HashMap<_, _>>();
    let mut ordered = tree.clone();
    ordered.sort_by_key(|(_, relative_depth)| std::cmp::Reverse(*relative_depth));
    for (thread, _) in &ordered {
        if let Some(parent) = thread.parent_thread_id.as_ref()
            && let Some(usage) = subtree_usage.get(&thread.thread_id).copied()
            && let Some(parent_usage) = subtree_usage.get_mut(parent)
        {
            add_usage_saturating(parent_usage, usage);
        }
        if let Some(parent) = thread.parent_thread_id.as_ref()
            && let Some(events) = subtree_events.get(&thread.thread_id).copied()
            && let Some(parent_events) = subtree_events.get_mut(parent)
        {
            *parent_events = parent_events.saturating_add(events);
        }
    }
    let root = tree[0].0.clone();
    let total_nodes = tree.len();
    let nodes = tree
        .into_iter()
        .take(800)
        .map(|(thread, relative_depth)| {
            let (usage, event_count) = own_usage.remove(&thread.thread_id).unwrap_or_default();
            let subtree = subtree_usage.remove(&thread.thread_id).unwrap_or_default();
            let subtree_event_count = subtree_events.remove(&thread.thread_id).unwrap_or_default();
            serde_json::json!({
                "id": thread.thread_id,
                "parentId": thread.parent_thread_id,
                "projectId": thread.project_id,
                "projectName": thread.project_name,
                "title": thread_label(&thread),
                "model": thread.model,
                "agentNickname": thread.agent_nickname,
                "agentRole": thread.agent_role,
                "agentPath": thread.agent_path,
                "depth": thread.depth,
                "relativeDepth": relative_depth,
                "createdAt": thread.created_at,
                "updatedAt": thread.updated_at,
                "archived": thread.archived,
                "sourceKind": thread.source_kind,
                "presentInCodex": thread.present_in_codex,
                "ownUsage": token_value(usage),
                "subtreeUsage": token_value(subtree),
                "eventCount": event_count,
                "subtreeEventCount": subtree_event_count,
            })
        })
        .collect::<Vec<_>>();
    let official_thread = if let Some(account) = store.active_account_fingerprint()? {
        store.latest_official_thread_usage(&account, &root.thread_id)?
    } else {
        None
    }
    .map(|stored| {
        serde_json::json!({
            "observedAt": stored.observed_at,
            "estimatedUsageCreditsMicros": stored.usage.estimated_usage_credits_micros,
            "estimatedUsageUsdMicros": stored.usage.estimated_usage_usd_micros,
            "groups": stored.usage.groups.into_iter().map(|group| serde_json::json!({
                "model": group.model,
                "reasoningEffort": group.reasoning_effort,
                "speed": group.speed,
                "estimatedUsageCreditsMicros": group.estimated_usage_credits_micros,
                "netNewInputTokens": group.net_new_input_tokens,
                "cachedInputTokens": group.cached_input_tokens,
                "cacheWriteInputTokens": group.cache_write_input_tokens,
                "inputTokens": group.input_tokens,
                "outputTokens": group.output_tokens,
                "totalTokens": group.total_tokens,
            })).collect::<Vec<_>>(),
        })
    });
    Ok(serde_json::json!({
        "id": root.thread_id,
        "title": thread_label(&root),
        "projectId": root.project_id,
        "projectName": root.project_name,
        "model": root.model,
        "createdAt": root.created_at,
        "updatedAt": root.updated_at,
        "presentInCodex": root.present_in_codex,
        "ownUsage": nodes.first().and_then(|node| node.get("ownUsage")).cloned().unwrap_or_else(|| token_value(TokenUsage::default())),
        "treeUsage": nodes.first().and_then(|node| node.get("subtreeUsage")).cloned().unwrap_or_else(|| token_value(TokenUsage::default())),
        "subagentCount": total_nodes.saturating_sub(1),
        "samplingTimeline": timeline.into_iter().map(|(bucket, (events, usage))| serde_json::json!({
            "bucket": bucket,
            "events": events,
            "usage": token_value(usage),
        })).collect::<Vec<_>>(),
        "ownSamplingTimeline": own_timeline.into_iter().map(|(bucket, (events, usage))| serde_json::json!({
            "bucket": bucket,
            "events": events,
            "usage": token_value(usage),
        })).collect::<Vec<_>>(),
        "samplingGrain": detail_grain,
        "officialThreadUsage": official_thread,
        "nodes": nodes,
        "truncated": total_nodes > 800,
    }))
}

fn catalog_roots(
    store: &LedgerStore,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<CatalogThread>, StoreError> {
    store.dashboard_catalog_roots(project_id, limit)
}

fn catalog_descendants(
    store: &LedgerStore,
    thread_id: &str,
) -> Result<Vec<(CatalogThread, u32)>, StoreError> {
    store.dashboard_catalog_descendants(thread_id)
}

fn catalog_project_summaries(
    store: &LedgerStore,
) -> Result<HashMap<String, (CatalogCounts, Option<String>)>, StoreError> {
    Ok(store
        .dashboard_catalog_project_summaries()?
        .into_iter()
        .collect())
}

fn catalog_counts(
    store: &LedgerStore,
    project_id: Option<&str>,
) -> Result<CatalogCounts, StoreError> {
    store.dashboard_catalog_counts(project_id)
}

pub(super) fn standalone_conversation_stats(
    store: &LedgerStore,
) -> Result<crate::store::StandaloneConversationStats, StoreError> {
    store.standalone_conversation_stats()
}

pub(super) fn thread_label(thread: &CatalogThread) -> String {
    if thread.depth > 0 {
        if let Some(path) = thread.agent_path.as_deref()
            && let Some(label) = path.rsplit('/').find(|part| !part.is_empty())
        {
            return label.replace('_', " ");
        }
        if let Some(nickname) = thread.agent_nickname.as_deref() {
            return nickname.to_owned();
        }
        return format!("Subagent {}", short_thread_id(&thread.thread_id));
    }
    if let Some(title) = thread.title.as_deref().map(str::trim)
        && !title.is_empty()
        && !title.contains('\n')
        && !title.contains('<')
        && !title.contains("/Users/")
        && !title.contains("/home/")
        && !contains_sensitive_title(title)
        && title.chars().count() <= 96
    {
        return title.to_owned();
    }
    format!("Session {}", short_thread_id(&thread.thread_id))
}

fn contains_sensitive_title(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if [
        "/private/",
        "/volumes/",
        "api key",
        "apikey",
        "password",
        "passwd",
        "secret=",
        "token=",
        "bearer ",
        "密钥",
        "密码",
    ]
    .into_iter()
    .any(|pattern| lower.contains(pattern))
        || lower.contains("\\users\\")
        || (lower.contains(" --") && lower.chars().count() > 72)
        || contains_private_address(&lower)
        || contains_email_like(&lower)
        || contains_long_identifier(&lower)
    {
        return true;
    }
    ["sk-", "ghp_", "github_pat_", "xoxb-", "xoxp-"]
        .into_iter()
        .any(|prefix| {
            let mut search_from = 0;
            while let Some(relative) = lower[search_from..].find(prefix) {
                let start = search_from + relative + prefix.len();
                let token_len = lower[start..]
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                    })
                    .count();
                if token_len >= 20 {
                    return true;
                }
                search_from = start;
            }
            false
        })
}

fn contains_private_address(value: &str) -> bool {
    if value.contains("fe80:")
        || value
            .split(|character: char| !character.is_ascii_hexdigit() && character != ':')
            .any(|part| {
                let part = part.to_ascii_lowercase();
                part.starts_with("fc") || part.starts_with("fd")
            })
    {
        return true;
    }
    value
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .filter(|part| part.contains('.'))
        .filter_map(|part| {
            let octets = part
                .split('.')
                .map(str::parse::<u8>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            (octets.len() == 4).then_some(octets)
        })
        .any(|octets| {
            octets[0] == 10
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || octets[0] == 127
        })
}

fn contains_email_like(value: &str) -> bool {
    value.split_whitespace().any(|part| {
        let part = part.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '@' | '.' | '_' | '-')
        });
        part.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
        })
    })
}

fn contains_long_identifier(value: &str) -> bool {
    value
        .split(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.')
        })
        .any(|part| {
            part.len() >= 32
                && part.chars().any(|character| character.is_ascii_digit())
                && part
                    .chars()
                    .any(|character| character.is_ascii_alphabetic())
        })
}

fn short_thread_id(thread_id: &str) -> String {
    thread_id.chars().take(13).collect()
}
