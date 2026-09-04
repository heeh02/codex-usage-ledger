use super::*;

#[derive(Debug, Clone, Default)]
struct ResidualAggregate {
    source_events: u64,
    usage: TokenUsage,
}

#[derive(Debug, Clone, Default)]
struct ResidualAccountAudit {
    aligned_days: u64,
    excess_days: u64,
    local_tokens: u64,
    official_tokens: u64,
    estimated_tokens: u64,
}

/// Estimates the combined usage of accounts that have not yet been captured.
///
/// This is deliberately a third ledger. It never mutates official account
/// totals or confirmed local attribution. For every captured account/day with
/// an official bucket, only `max(local - official, 0)` is considered evidence
/// of historical over-attribution. That residual is distributed over the
/// account/day's local project/model/token composition with exact largest-
/// remainder arithmetic.
pub(super) fn missing_account_estimate(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &PeriodDescriptor,
) -> Result<serde_json::Value, StoreError> {
    let registry = account_registry(store)?;
    let captured_accounts = store.list_official_accounts()?;
    let missing_accounts = registry.unobserved_count();
    let selected_account = selected(&query.account);
    let applicable =
        selected_account.is_none() && missing_accounts > 0 && !captured_accounts.is_empty();
    let timezone = Tz::from_str(&period.timezone).unwrap_or(chrono_tz::Asia::Shanghai);
    let (start_day, end_day) = official_day_bounds(period.start, period.end, timezone);

    let empty = || {
        serde_json::json!({
            "definitionId": "missing_accounts_residual_v1",
            "status": if selected_account.is_some() { "not_applicable_to_single_account" } else { "insufficient_coverage" },
            "applicable": applicable,
            "isEstimate": true,
            "isConservativeFloor": true,
            "canSplitByMissingAccount": false,
            "combinedUnobservedAccountCount": missing_accounts,
            "capturedAccountCount": captured_accounts.len(),
            "coverageStart": serde_json::Value::Null,
            "coverageThrough": serde_json::Value::Null,
            "alignedAccountDays": 0,
            "excessAccountDays": 0,
            "excludedAccountDays": 0,
            "rawResidualTokens": 0,
            "allocationRoundingDelta": 0,
            "componentInvariantMismatchTokens": 0,
            "localAssignedOnAlignedDays": 0,
            "officialOnAlignedDays": 0,
            "knownLocalCappedTokens": 0,
            "selectedUsage": token_value(TokenUsage::default()),
            "totalUsage": token_value(TokenUsage::default()),
            "byDay": [],
            "byProject": [],
            "byModel": [],
            "sourceAccountExcess": [],
            "method": "逐账号逐日比较；仅分配 max(本地归因 - 同账号官方日桶, 0)",
        })
    };
    if !applicable {
        return Ok(empty());
    }

    let mut official_by_account_day = HashMap::<(String, String), u64>::new();
    for account in &captured_accounts {
        for bucket in
            store.official_daily_usage(account, start_day.as_deref(), end_day.as_deref())?
        {
            official_by_account_day.insert((account.clone(), bucket.start_date), bucket.tokens);
        }
    }

    let source_rows =
        store.residual_usage_rows(start_day.as_deref(), end_day.as_deref(), &captured_accounts)?;

    let mut grouped = BTreeMap::<(String, String), Vec<ResidualUsageRow>>::new();
    for row in source_rows {
        grouped
            .entry((row.account.clone(), row.day.clone()))
            .or_default()
            .push(row);
    }

    let mut allocated_rows = Vec::<ResidualUsageRow>::new();
    let mut source_audit = BTreeMap::<String, ResidualAccountAudit>::new();
    let mut aligned_account_days = 0_u64;
    let mut excess_account_days = 0_u64;
    let mut excluded_account_days = 0_u64;
    let mut raw_residual_tokens = 0_u64;
    let mut component_invariant_mismatch_tokens = 0_u64;
    let mut local_assigned_on_aligned_days = 0_u64;
    let mut official_on_aligned_days = 0_u64;
    let mut known_local_capped_tokens = 0_u64;
    let mut coverage_start: Option<String> = None;
    let mut coverage_through: Option<String> = None;
    let mut aligned_dates = BTreeSet::<String>::new();

    for ((account, day), rows) in grouped {
        let Some(official_tokens) = official_by_account_day
            .get(&(account.clone(), day.clone()))
            .copied()
        else {
            excluded_account_days = excluded_account_days.saturating_add(1);
            continue;
        };
        let local_tokens = rows
            .iter()
            .map(|row| row.usage.total_tokens)
            .fold(0_u64, u64::saturating_add);
        aligned_account_days = aligned_account_days.saturating_add(1);
        aligned_dates.insert(day.clone());
        local_assigned_on_aligned_days =
            local_assigned_on_aligned_days.saturating_add(local_tokens);
        official_on_aligned_days = official_on_aligned_days.saturating_add(official_tokens);
        known_local_capped_tokens =
            known_local_capped_tokens.saturating_add(local_tokens.min(official_tokens));
        coverage_start =
            Some(coverage_start.map_or_else(|| day.clone(), |start| start.min(day.clone())));
        coverage_through =
            Some(coverage_through.map_or_else(|| day.clone(), |end| end.max(day.clone())));

        let audit = source_audit.entry(account.clone()).or_default();
        audit.aligned_days = audit.aligned_days.saturating_add(1);
        audit.local_tokens = audit.local_tokens.saturating_add(local_tokens);
        audit.official_tokens = audit.official_tokens.saturating_add(official_tokens);

        let residual = local_tokens.saturating_sub(official_tokens);
        if residual == 0 {
            continue;
        }
        excess_account_days = excess_account_days.saturating_add(1);
        raw_residual_tokens = raw_residual_tokens.saturating_add(residual);
        audit.excess_days = audit.excess_days.saturating_add(1);

        let (allocations, mismatch) = allocate_residual_components(&rows, residual);
        audit.estimated_tokens = audit.estimated_tokens.saturating_add(
            allocations
                .iter()
                .map(|usage| usage.total_tokens)
                .fold(0_u64, u64::saturating_add),
        );
        component_invariant_mismatch_tokens =
            component_invariant_mismatch_tokens.saturating_add(mismatch);
        for (row, usage) in rows.into_iter().zip(allocations) {
            if usage.total_tokens == 0 {
                continue;
            }
            allocated_rows.push(ResidualUsageRow { usage, ..row });
        }
    }

    let mut total_usage = TokenUsage::default();
    let mut selected_usage = TokenUsage::default();
    let mut by_day = BTreeMap::<String, ResidualAggregate>::new();
    let mut by_project = BTreeMap::<String, ResidualAggregate>::new();
    let mut by_model = BTreeMap::<String, ResidualAggregate>::new();
    for day in aligned_dates {
        by_day.entry(day).or_default();
    }
    let selected_project = selected(&query.project);
    let selected_model = selected(&query.model);
    for row in allocated_rows {
        add_usage_saturating(&mut total_usage, row.usage);
        let project = if row.project.is_empty() {
            UNASSIGNED_PROJECT_ID.to_owned()
        } else {
            row.project.clone()
        };
        let model = if row.model.is_empty() {
            "unknown".to_owned()
        } else {
            row.model.clone()
        };
        for aggregate in [
            by_day.entry(row.day.clone()).or_default(),
            by_project.entry(project.clone()).or_default(),
            by_model.entry(model.clone()).or_default(),
        ] {
            aggregate.source_events = aggregate.source_events.saturating_add(row.source_events);
            add_usage_saturating(&mut aggregate.usage, row.usage);
        }
        let project_matches = selected_project
            .as_deref()
            .is_none_or(|selected| selected == project);
        let model_matches = selected_model
            .as_deref()
            .is_none_or(|selected| selected == model);
        if project_matches && model_matches {
            add_usage_saturating(&mut selected_usage, row.usage);
        }
    }

    let project_names = store
        .list_projects()?
        .into_iter()
        .map(|project| (project.project_id, project.project_name))
        .collect::<HashMap<_, _>>();
    let mut project_values = by_project
        .into_iter()
        .map(|(id, aggregate)| {
            let label = if id == STANDALONE_PROJECT_ID {
                STANDALONE_PROJECT_LABEL.to_owned()
            } else if id == UNASSIGNED_PROJECT_ID {
                UNASSIGNED_PROJECT_LABEL.to_owned()
            } else {
                project_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| id.clone())
            };
            serde_json::json!({
                "id": id,
                "label": label,
                "usage": token_value(aggregate.usage),
                "sourceEvents": aggregate.source_events,
            })
        })
        .collect::<Vec<_>>();
    project_values.sort_by_key(|value| {
        std::cmp::Reverse(value["usage"]["total"].as_u64().unwrap_or_default())
    });

    let mut model_values = by_model
        .into_iter()
        .map(|(id, aggregate)| {
            serde_json::json!({
                "id": id,
                "label": if id == "unknown" { "未知模型" } else { &id },
                "usage": token_value(aggregate.usage),
                "sourceEvents": aggregate.source_events,
            })
        })
        .collect::<Vec<_>>();
    model_values.sort_by_key(|value| {
        std::cmp::Reverse(value["usage"]["total"].as_u64().unwrap_or_default())
    });

    let allocated_total = total_usage.total_tokens;
    let allocation_rounding_delta = raw_residual_tokens.saturating_sub(allocated_total);
    let status = if aligned_account_days == 0 {
        "insufficient_coverage"
    } else {
        "conservative_floor"
    };
    Ok(serde_json::json!({
        "definitionId": "missing_accounts_residual_v1",
        "status": status,
        "applicable": true,
        "isEstimate": true,
        "isConservativeFloor": true,
        "canSplitByMissingAccount": false,
        "combinedUnobservedAccountCount": missing_accounts,
        "capturedAccountCount": captured_accounts.len(),
        "coverageStart": coverage_start,
        "coverageThrough": coverage_through,
        "alignedAccountDays": aligned_account_days,
        "excessAccountDays": excess_account_days,
        "excludedAccountDays": excluded_account_days,
        "rawResidualTokens": raw_residual_tokens,
        "allocationRoundingDelta": allocation_rounding_delta,
        "componentInvariantMismatchTokens": component_invariant_mismatch_tokens,
        "localAssignedOnAlignedDays": local_assigned_on_aligned_days,
        "officialOnAlignedDays": official_on_aligned_days,
        "knownLocalCappedTokens": known_local_capped_tokens,
        "selectedUsage": token_value(selected_usage),
        "totalUsage": token_value(total_usage),
        "byDay": by_day.into_iter().map(|(date, aggregate)| serde_json::json!({
            "date": date,
            "usage": token_value(aggregate.usage),
            "sourceEvents": aggregate.source_events,
        })).collect::<Vec<_>>(),
        "byProject": project_values,
        "byModel": model_values,
        "sourceAccountExcess": source_audit.into_iter().map(|(account, audit)| serde_json::json!({
            "accountId": account,
            "accountLabel": account_label(&account),
            "alignedDays": audit.aligned_days,
            "excessDays": audit.excess_days,
            "localTokens": audit.local_tokens,
            "officialTokens": audit.official_tokens,
            "estimatedTokens": audit.estimated_tokens,
        })).collect::<Vec<_>>(),
        "method": "逐账号逐日比较；仅分配 max(本地归因 - 同账号官方日桶, 0)",
    }))
}

fn allocate_residual_components(
    rows: &[ResidualUsageRow],
    residual: u64,
) -> (Vec<TokenUsage>, u64) {
    let mut weights = Vec::<u64>::with_capacity(rows.len().saturating_mul(5));
    let mut expected_total = 0_u64;
    let mut component_total = 0_u64;
    for row in rows {
        expected_total = expected_total.saturating_add(row.usage.total_tokens);
        let components = [
            row.usage.cached_input_tokens.min(row.usage.input_tokens),
            row.usage.cache_write_input_tokens.min(
                row.usage
                    .input_tokens
                    .saturating_sub(row.usage.cached_input_tokens),
            ),
            row.usage.uncached_input_tokens(),
            row.usage
                .reasoning_output_tokens
                .min(row.usage.output_tokens),
            row.usage
                .output_tokens
                .saturating_sub(row.usage.reasoning_output_tokens),
        ];
        component_total = component_total.saturating_add(components.iter().copied().sum::<u64>());
        weights.extend(components);
    }
    let target = residual.min(component_total);
    let allocated = largest_remainder_allocate(&weights, target);
    let mut result = Vec::with_capacity(rows.len());
    for (row, chunk) in rows.iter().zip(allocated.chunks_exact(5)) {
        let cached = chunk[0];
        let cache_write = chunk[1];
        let uncached = chunk[2];
        let reasoning = chunk[3];
        let other_output = chunk[4];
        let input = cached.saturating_add(cache_write).saturating_add(uncached);
        let output = reasoning.saturating_add(other_output);
        result.push(TokenUsage {
            input_tokens: input,
            cached_input_tokens: cached,
            cache_write_input_tokens: cache_write,
            cache_write_observed_input_tokens: if row.usage.cache_write_observed_input_tokens > 0 {
                input
            } else {
                0
            },
            output_tokens: output,
            reasoning_output_tokens: reasoning,
            total_tokens: input.saturating_add(output),
        });
    }
    (result, expected_total.abs_diff(component_total))
}

fn largest_remainder_allocate(weights: &[u64], target: u64) -> Vec<u64> {
    let denominator = weights.iter().map(|value| u128::from(*value)).sum::<u128>();
    if denominator == 0 || target == 0 {
        return vec![0; weights.len()];
    }
    let target = u128::from(target);
    let mut allocations = vec![0_u64; weights.len()];
    let mut remainders = Vec::<(u128, usize)>::with_capacity(weights.len());
    let mut assigned = 0_u64;
    for (index, weight) in weights.iter().copied().enumerate() {
        let numerator = u128::from(weight).saturating_mul(target);
        let base = (numerator / denominator) as u64;
        allocations[index] = base;
        assigned = assigned.saturating_add(base);
        remainders.push((numerator % denominator, index));
    }
    remainders.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    for (_, index) in remainders
        .into_iter()
        .take((target as u64).saturating_sub(assigned) as usize)
    {
        allocations[index] = allocations[index].saturating_add(1);
    }
    allocations
}

pub(super) fn resolved_account_total_metric(
    query: &UsageQuery,
    period: &PeriodDescriptor,
    official: &serde_json::Value,
) -> ResolvedMetric {
    let value = official
        .get("displayTotalTokens")
        .and_then(serde_json::Value::as_u64);
    let display_kind = official
        .get("displayTotalKind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("not_applicable");
    let coverage_complete = official
        .get("coverageComplete")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let account_coverage_complete = official
        .get("accountCoverageComplete")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let display_is_lower_bound = official
        .get("displayIsLowerBound")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let source = match display_kind {
        "official" => MetricSource::Official,
        "official_plus_local_tail_lower_bound" => MetricSource::Reconciled,
        _ => MetricSource::Local,
    };
    let status = if value.is_none() {
        MetricStatus::Unknown
    } else if display_is_lower_bound || !coverage_complete || !account_coverage_complete {
        MetricStatus::LowerBound
    } else {
        MetricStatus::Exact
    };
    ResolvedMetric {
        value,
        source,
        status,
        window_start: period.start,
        window_end: period.end,
        timezone: period.timezone.clone(),
        account_scope: selected(&query.account).unwrap_or_else(|| "all".to_owned()),
        machine_scope: "all_devices".to_owned(),
        coverage: MetricCoverage {
            complete: coverage_complete && account_coverage_complete,
            ratio: official
                .get("coverageRatio")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0),
            known_account_count: official
                .get("knownAccountCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
            missing_official_account_count: official
                .get("missingOfficialAccountCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default(),
        },
        definition_id: "account_total_v1".to_owned(),
    }
}

pub(super) fn official_usage_view(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &PeriodDescriptor,
) -> Result<serde_json::Value, StoreError> {
    official_usage_view_with_reconciliation(store, query, period, true)
}

pub(super) fn compact_official_usage_view(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &PeriodDescriptor,
) -> Result<serde_json::Value, StoreError> {
    official_usage_view_with_reconciliation(store, query, period, false)
}

#[derive(Debug, Clone, Default)]
pub(super) struct AccountRegistry {
    pub(super) canonical: BTreeSet<String>,
    pub(super) provisional: BTreeSet<String>,
    pub(super) user_confirmed_total: Option<u64>,
}

impl AccountRegistry {
    pub(super) fn observed(&self) -> BTreeSet<String> {
        let mut observed = self.canonical.clone();
        observed.extend(self.provisional.iter().cloned());
        observed
    }

    pub(super) fn observed_count(&self) -> u64 {
        self.observed().len() as u64
    }

    pub(super) fn expected_count(&self) -> u64 {
        self.user_confirmed_total
            .unwrap_or_default()
            .max(self.observed_count())
    }

    pub(super) fn unobserved_count(&self) -> u64 {
        self.expected_count().saturating_sub(self.observed_count())
    }
}

pub(super) fn account_registry_value(store: &LedgerStore) -> Result<serde_json::Value, StoreError> {
    let registry = account_registry(store)?;
    Ok(serde_json::json!({
        "observedAccountCount": registry.observed_count(),
        "verifiedAccountCount": registry.canonical.len(),
        "provisionalIdentityCount": registry.provisional.len(),
        "userConfirmedAccountCount": registry.user_confirmed_total,
        "knownAccountCount": registry.expected_count(),
        "unobservedAccountCount": registry.unobserved_count(),
    }))
}

pub(super) fn account_registry(store: &LedgerStore) -> Result<AccountRegistry, StoreError> {
    let mut canonical = store
        .list_official_accounts()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut provisional = BTreeSet::new();
    for (account, is_canonical) in store.account_workspace_aliases()? {
        if is_canonical {
            canonical.insert(account);
        } else {
            provisional.insert(account);
        }
    }
    for account in store.verified_auth_accounts()? {
        canonical.insert(account);
    }
    for account in store
        .aggregate_rollup_by(
            AggregateDimension::Account,
            &AggregateFilter {
                quality: None,
                ..Default::default()
            },
        )?
        .into_iter()
        .filter_map(|bucket| bucket.key)
    {
        if !canonical.contains(&account) {
            provisional.insert(account);
        }
    }
    provisional.retain(|account| !canonical.contains(account));
    Ok(AccountRegistry {
        canonical,
        provisional,
        user_confirmed_total: store.user_confirmed_account_count()?,
    })
}

fn official_usage_view_with_reconciliation(
    store: &LedgerStore,
    query: &UsageQuery,
    period: &PeriodDescriptor,
    include_reconciled_points: bool,
) -> Result<serde_json::Value, StoreError> {
    let timezone_name = query.timezone.as_deref().unwrap_or("Asia/Shanghai");
    let timezone = Tz::from_str(timezone_name).unwrap_or(chrono_tz::Asia::Shanghai);
    let selected_account = selected(&query.account);
    let registry = account_registry(store)?;
    let known_accounts = registry.observed();
    let accounts = if let Some(account) = selected_account.clone() {
        vec![account]
    } else {
        store.list_official_accounts()?
    };
    let expected_account_count = if selected_account.is_some() {
        1_u64
    } else {
        registry.expected_count()
    };
    let observed_account_count = if selected_account.is_some() {
        1_u64
    } else {
        registry.observed_count()
    };
    let unobserved_account_count = if selected_account.is_some() {
        0_u64
    } else {
        registry.unobserved_count()
    };
    let (start_day, end_day) = official_day_bounds(period.start, period.end, timezone);
    let (previous_start_day, previous_end_day) =
        official_day_bounds(period.comparison_start, period.comparison_end, timezone);
    let mut points = BTreeMap::<String, u64>::new();
    let mut comparison_points = BTreeMap::<String, u64>::new();
    let mut previous_total = 0_u64;
    let mut lifetime_total = 0_u64;
    let mut lifetime_available = !accounts.is_empty();
    let mut peak_daily = 0_u64;
    let mut observed_at: Option<DateTime<Utc>> = None;
    let mut coverage_start: Option<String> = None;
    let mut coverage_through: Option<String> = None;
    let mut account_coverage_start = BTreeMap::<String, String>::new();
    let mut account_coverage_through = BTreeMap::<String, String>::new();
    let mut successful_accounts = 0_u64;
    let mut last_error: Option<String> = None;

    for account in &accounts {
        let Some(snapshot) = store.latest_official_account_usage(account)? else {
            lifetime_available = false;
            continue;
        };
        successful_accounts = successful_accounts.saturating_add(1);
        observed_at =
            Some(observed_at.map_or(snapshot.observed_at, |at| at.max(snapshot.observed_at)));
        if let Some(value) = snapshot.usage.summary.lifetime_tokens {
            lifetime_total = lifetime_total.saturating_add(value);
        } else {
            lifetime_available = false;
        }
        peak_daily = peak_daily.max(snapshot.usage.summary.peak_daily_tokens.unwrap_or_default());
        for bucket in
            store.official_daily_usage(account, start_day.as_deref(), end_day.as_deref())?
        {
            coverage_start = Some(coverage_start.map_or_else(
                || bucket.start_date.clone(),
                |value| value.min(bucket.start_date.clone()),
            ));
            coverage_through = Some(coverage_through.map_or_else(
                || bucket.start_date.clone(),
                |value| value.max(bucket.start_date.clone()),
            ));
            let entry = points.entry(bucket.start_date).or_default();
            *entry = entry.saturating_add(bucket.tokens);
        }
        for bucket in store.official_daily_usage(
            account,
            previous_start_day.as_deref(),
            previous_end_day.as_deref(),
        )? {
            previous_total = previous_total.saturating_add(bucket.tokens);
            let entry = comparison_points.entry(bucket.start_date).or_default();
            *entry = entry.saturating_add(bucket.tokens);
        }
        if let Some(sync) = store.official_usage_sync_state(account)?
            && sync.last_error.is_some()
        {
            last_error = sync.last_error;
        }
    }

    // Coverage must be derived from the complete account history, not merely
    // the selected range (which may contain no activity).
    for account in &accounts {
        for bucket in store.official_daily_usage(account, None, None)? {
            account_coverage_start
                .entry(account.clone())
                .and_modify(|value| {
                    if bucket.start_date < *value {
                        *value = bucket.start_date.clone();
                    }
                })
                .or_insert_with(|| bucket.start_date.clone());
            account_coverage_through
                .entry(account.clone())
                .and_modify(|value| {
                    if bucket.start_date > *value {
                        *value = bucket.start_date.clone();
                    }
                })
                .or_insert_with(|| bucket.start_date.clone());
            coverage_start = Some(coverage_start.map_or_else(
                || bucket.start_date.clone(),
                |value| value.min(bucket.start_date.clone()),
            ));
            coverage_through = Some(coverage_through.map_or_else(
                || bucket.start_date.clone(),
                |value| value.max(bucket.start_date.clone()),
            ));
        }
    }

    let all_accounts_have_daily_coverage = expected_account_count > 0
        && account_coverage_start.len() as u64 == expected_account_count
        && account_coverage_through.len() as u64 == expected_account_count;
    let common_coverage_start = all_accounts_have_daily_coverage
        .then(|| account_coverage_start.values().max().cloned())
        .flatten();
    let common_coverage_through = all_accounts_have_daily_coverage
        .then(|| account_coverage_through.values().min().cloned())
        .flatten();
    let latest_coverage_through = account_coverage_through.values().max().cloned();
    let coverage_accounts = if let Some(account) = selected_account.clone() {
        vec![account]
    } else {
        known_accounts.iter().cloned().collect::<Vec<_>>()
    };
    let account_coverage = coverage_accounts
        .iter()
        .map(|account| {
            serde_json::json!({
                "accountId": account,
                "coverageStart": account_coverage_start.get(account),
                "coverageThrough": account_coverage_through.get(account),
                "officialAvailable": account_coverage_start.contains_key(account),
            })
        })
        .collect::<Vec<_>>();
    let mut reconciled_points = if include_reconciled_points {
        reconcile_account_days(
            store,
            AccountReconciliationScope {
                accounts: &coverage_accounts,
                coverage_start: &account_coverage_start,
                coverage_through: &account_coverage_through,
                known_account_count: expected_account_count,
            },
            start_day.as_ref(),
            end_day.as_ref(),
            timezone,
        )?
    } else {
        Vec::new()
    };
    let mut reconciled_comparison_points = if include_reconciled_points
        && previous_start_day.is_some()
        && previous_end_day.is_some()
    {
        reconcile_account_days(
            store,
            AccountReconciliationScope {
                accounts: &coverage_accounts,
                coverage_start: &account_coverage_start,
                coverage_through: &account_coverage_through,
                known_account_count: expected_account_count,
            },
            previous_start_day.as_ref(),
            previous_end_day.as_ref(),
            timezone,
        )?
    } else {
        Vec::new()
    };

    let fill_covered_zeros = |values: &mut BTreeMap<String, u64>,
                              requested_start: Option<&String>,
                              requested_end: Option<&String>| {
        let start = requested_start
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .or_else(|| {
                common_coverage_start
                    .as_deref()
                    .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            });
        let end = requested_end.and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let covered_start = common_coverage_start
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let covered_end = common_coverage_through
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .and_then(|date| date.succ_opt());
        if let (Some(start), Some(end), Some(covered_start), Some(covered_end)) =
            (start, end, covered_start, covered_end)
        {
            let mut day = start.max(covered_start);
            let end = end.min(covered_end);
            while day < end {
                values.entry(day.to_string()).or_default();
                let Some(next) = day.succ_opt() else { break };
                day = next;
            }
        }
    };
    fill_covered_zeros(&mut points, start_day.as_ref(), end_day.as_ref());
    fill_covered_zeros(
        &mut comparison_points,
        previous_start_day.as_ref(),
        previous_end_day.as_ref(),
    );

    let bucket_total = points.values().copied().fold(0_u64, u64::saturating_add);
    let requested_grain = query
        .grain
        .as_deref()
        .unwrap_or(period.default_grain.as_str());
    let official_grain = match requested_grain {
        "week" => "week",
        "month" => "month",
        _ => "day",
    };
    reconciled_points = aggregate_reconciled_account_days(reconciled_points, official_grain);
    reconciled_comparison_points =
        aggregate_reconciled_account_days(reconciled_comparison_points, official_grain);
    let display_points = aggregate_official_points(points, official_grain);
    let display_comparison_points = aggregate_official_points(comparison_points, official_grain);
    let lifetime_period = query.period.as_deref() == Some("lifetime");
    let coverage_exclusive = coverage_through
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .and_then(|date| date.succ_opt())
        .map(|date| date.to_string());
    let selected_has_coverage = lifetime_period
        || start_day
            .as_ref()
            .zip(coverage_exclusive.as_ref())
            .is_some_and(|(start, coverage_end)| start < coverage_end);
    let previous_has_coverage = previous_start_day
        .as_ref()
        .zip(coverage_exclusive.as_ref())
        .is_some_and(|(start, coverage_end)| start < coverage_end);
    let selected_total = if successful_accounts == 0 || !selected_has_coverage {
        None
    } else if lifetime_period && lifetime_available {
        Some(lifetime_total)
    } else {
        Some(bucket_total)
    };
    let previous =
        (successful_accounts > 0 && period.comparison_start.is_some() && previous_has_coverage)
            .then_some(previous_total);
    let delta_tokens = selected_total
        .zip(previous)
        .map(|(current, previous)| i128::from(current) - i128::from(previous));
    let delta_percent = delta_tokens
        .zip(previous)
        .and_then(|(delta, previous)| (previous > 0).then(|| delta as f64 / previous as f64));
    let today = Utc::now().with_timezone(&timezone).date_naive().to_string();
    let common_start_covers_request = match (&common_coverage_start, &start_day) {
        (Some(covered), Some(requested)) => covered <= requested,
        (Some(_), None) => true,
        _ => false,
    };
    let common_end_covers_request = match (&common_coverage_through, &end_day) {
        (Some(through), Some(end)) => NaiveDate::parse_from_str(through, "%Y-%m-%d")
            .ok()
            .and_then(|date| date.succ_opt())
            .is_some_and(|next| next.to_string() >= *end),
        (Some(_), None) => true,
        _ => false,
    };
    let coverage_complete = if lifetime_period {
        lifetime_available && successful_accounts == expected_account_count
    } else {
        common_start_covers_request && common_end_covers_request
    };
    let coverage_ratio = if lifetime_period && end_day.is_none() {
        1.0
    } else {
        let requested_start = start_day
            .as_deref()
            .or(common_coverage_start.as_deref())
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let requested_end = end_day
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let covered_start = common_coverage_start
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        let covered_end = common_coverage_through
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .and_then(|date| date.succ_opt());
        match (requested_start, requested_end, covered_start, covered_end) {
            (Some(start), Some(end), Some(covered_start), Some(covered_end)) if end > start => {
                let overlap_start = start.max(covered_start);
                let overlap_end = end.min(covered_end);
                let covered_days = overlap_end
                    .signed_duration_since(overlap_start)
                    .num_days()
                    .max(0);
                let requested_days = end.signed_duration_since(start).num_days().max(1);
                (covered_days as f64 / requested_days as f64).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    };
    let exact_period = period
        .start
        .is_none_or(|start| start.with_timezone(&timezone).time() == NaiveTime::MIN)
        && period
            .end
            .is_none_or(|end| end.with_timezone(&timezone).time() == NaiveTime::MIN);
    let primary_scope = selected(&query.project).is_none()
        && selected(&query.model).is_none()
        && selected(&query.session).is_none()
        && query
            .metric
            .as_deref()
            .is_none_or(|metric| metric == "total");
    let account_coverage_complete =
        expected_account_count > 0 && successful_accounts == expected_account_count;
    let identity_scope_complete = selected_account.is_some()
        || (registry.provisional.is_empty() && unobserved_account_count == 0);
    let local_tail_start = coverage_through
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .and_then(|date| date.succ_opt())
        .and_then(|date| local_midnight_utc(date, timezone));
    let effective_tail_start = match (period.start, local_tail_start) {
        (Some(period_start), Some(coverage_end)) => Some(period_start.max(coverage_end)),
        (Some(period_start), None) => Some(period_start),
        (None, value) => value,
    };
    let complement_accounts = if let Some(account) = selected_account.clone() {
        vec![account]
    } else {
        known_accounts.iter().cloned().collect::<Vec<_>>()
    };
    let mut local_tail_tokens = 0_u64;
    let mut missing_account_local_tokens = 0_u64;
    if primary_scope {
        for account in complement_accounts {
            let account_coverage_end = account_coverage_through
                .get(&account)
                .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
                .and_then(|date| date.succ_opt())
                .and_then(|date| local_midnight_utc(date, timezone));
            let complement_start = match (period.start, account_coverage_end) {
                (Some(period_start), Some(coverage_end)) => Some(period_start.max(coverage_end)),
                (Some(period_start), None) => Some(period_start),
                (None, value) => value,
            };
            let usage = store.aggregate_rollup_usage(&AggregateFilter {
                start_inclusive: complement_start,
                end_exclusive: period.end,
                account_fingerprint: Some(account.clone()),
                project_id: None,
                model: None,
                quality: Some(DataQuality::Confirmed),
            })?;
            if account_coverage_through.contains_key(&account) {
                local_tail_tokens = local_tail_tokens.saturating_add(usage.usage.total_tokens);
            } else {
                missing_account_local_tokens =
                    missing_account_local_tokens.saturating_add(usage.usage.total_tokens);
            }
        }
    }
    let local_complement_tokens = local_tail_tokens.saturating_add(missing_account_local_tokens);
    let mut provisional_local_tokens = 0_u64;
    if primary_scope && selected_account.is_none() {
        for provisional in &registry.provisional {
            provisional_local_tokens = provisional_local_tokens.saturating_add(
                store
                    .aggregate_rollup_usage(&AggregateFilter {
                        start_inclusive: period.start,
                        end_exclusive: period.end,
                        account_fingerprint: Some(provisional.clone()),
                        project_id: None,
                        model: None,
                        quality: Some(DataQuality::Confirmed),
                    })?
                    .usage
                    .total_tokens,
            );
        }
    }
    let display_total_tokens = primary_scope.then(|| {
        selected_total
            .unwrap_or_default()
            .saturating_add(local_complement_tokens)
    });
    let display_total_kind = if !primary_scope {
        "not_applicable"
    } else if selected_total.is_none() {
        "local_lower_bound"
    } else if local_complement_tokens > 0 || !account_coverage_complete || !identity_scope_complete
    {
        "official_plus_local_tail_lower_bound"
    } else {
        "official"
    };
    let display_is_lower_bound = primary_scope
        && (selected_total.is_none()
            || local_complement_tokens > 0
            || !account_coverage_complete
            || !identity_scope_complete
            || !coverage_complete);
    let display_previous_total_tokens = if reconciled_comparison_points.is_empty() {
        previous
    } else {
        Some(
            reconciled_comparison_points
                .iter()
                .fold(0_u64, |sum, point| sum.saturating_add(point.value)),
        )
    };
    let previous_display_is_lower_bound = reconciled_comparison_points
        .iter()
        .any(|point| !matches!(point.status, AccountDayStatus::ExactOfficial));
    let display_delta_tokens = (!display_is_lower_bound && !previous_display_is_lower_bound)
        .then(|| display_total_tokens.zip(display_previous_total_tokens))
        .flatten()
        .map(|(current, previous)| i128::from(current) - i128::from(previous));
    let display_delta_percent = display_delta_tokens
        .zip(display_previous_total_tokens)
        .and_then(|(delta, previous)| (previous > 0).then(|| delta as f64 / previous as f64));
    let mut result = serde_json::json!({
        "source": "codex_account_usage_read",
        "primaryScope": primary_scope,
        "authoritativeForAccountTotal": primary_scope && account_coverage_complete && identity_scope_complete && coverage_complete,
        "accountCoverageComplete": account_coverage_complete,
        "identityScopeComplete": identity_scope_complete,
        "knownAccountCount": expected_account_count,
        "observedAccountCount": observed_account_count,
        "userConfirmedAccountCount": registry.user_confirmed_total,
        "unobservedAccountCount": unobserved_account_count,
        "verifiedAccountCount": registry.canonical.len(),
        "missingOfficialAccountCount": expected_account_count.saturating_sub(successful_accounts),
        "provisionalIdentityCount": registry.provisional.len(),
        "provisionalLocalTokens": provisional_local_tokens,
        "totalIsLowerBound": successful_accounts > 0 && (!account_coverage_complete || !identity_scope_complete || !coverage_complete),
        "displayTotalTokens": display_total_tokens,
        "displayTotalKind": display_total_kind,
        "displayIsLowerBound": display_is_lower_bound,
        "localTailTokens": local_tail_tokens,
        "missingAccountLocalTokens": missing_account_local_tokens,
        "localComplementTokens": local_complement_tokens,
        "localTailStart": effective_tail_start,
        "totalTokens": selected_total,
        "bucketTotalTokens": bucket_total,
        "lifetimeTokens": lifetime_available.then_some(lifetime_total),
        "peakDailyTokens": (successful_accounts > 0).then_some(peak_daily),
        "previousTotalTokens": previous,
        "deltaTokens": delta_tokens,
        "deltaPercent": delta_percent,
        "displayPreviousTotalTokens": display_previous_total_tokens,
        "previousDisplayIsLowerBound": previous_display_is_lower_bound,
        "displayDeltaTokens": display_delta_tokens,
        "displayDeltaPercent": display_delta_percent,
    });
    let details = serde_json::json!({
        "points": display_points.into_iter().map(|(date, tokens)| serde_json::json!({"date": date, "tokens": tokens})).collect::<Vec<_>>(),
        "comparisonPoints": display_comparison_points.into_iter().map(|(date, tokens)| serde_json::json!({"date": date, "tokens": tokens})).collect::<Vec<_>>(),
        "accountCount": successful_accounts,
        "observedAt": observed_at,
        "coverageStart": coverage_start,
        "coverageThrough": coverage_through,
        "commonCoverageStart": common_coverage_start,
        "commonCoverageThrough": common_coverage_through,
        "latestCoverageThrough": latest_coverage_through,
        "accountCoverage": account_coverage,
        "reconciledPoints": reconciled_points,
        "reconciledComparisonPoints": reconciled_comparison_points,
        "coverageComplete": coverage_complete,
        "coverageRatio": coverage_ratio,
        "periodExact": exact_period,
        "backendIncludesToday": coverage_through.as_deref() == Some(today.as_str()),
        "granularity": official_grain,
        "lastError": last_error,
    });
    if let (Some(target), Some(details)) = (result.as_object_mut(), details.as_object()) {
        target.extend(details.clone());
    }
    Ok(result)
}

struct AccountReconciliationScope<'a> {
    accounts: &'a [String],
    coverage_start: &'a BTreeMap<String, String>,
    coverage_through: &'a BTreeMap<String, String>,
    known_account_count: u64,
}

fn reconcile_account_days(
    store: &LedgerStore,
    scope: AccountReconciliationScope<'_>,
    requested_start: Option<&String>,
    requested_end: Option<&String>,
    timezone: Tz,
) -> Result<Vec<ReconciledAccountDay>, StoreError> {
    let accounts = scope.accounts;
    let coverage_start = scope.coverage_start;
    let coverage_through = scope.coverage_through;
    let known_account_count = scope.known_account_count;
    let local_start =
        earliest_event_at(store)?.map(|value| value.with_timezone(&timezone).date_naive());
    let official_start = coverage_start
        .values()
        .filter_map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .min();
    let start = requested_start
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .or_else(|| match (local_start, official_start) {
            (Some(local), Some(official)) => Some(local.min(official)),
            (Some(local), None) => Some(local),
            (None, Some(official)) => Some(official),
            (None, None) => None,
        });
    let end = requested_end
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .or_else(|| Utc::now().with_timezone(&timezone).date_naive().succ_opt());
    let (Some(start), Some(end)) = (start, end) else {
        return Ok(Vec::new());
    };
    if end <= start || accounts.is_empty() {
        return Ok(Vec::new());
    }

    let start_utc = local_midnight_utc(start, timezone);
    let end_utc = local_midnight_utc(end, timezone);
    let mut official_by_account = HashMap::<String, BTreeMap<String, u64>>::new();
    let mut local_by_account = HashMap::<String, BTreeMap<String, u64>>::new();
    for account in accounts {
        let official = store
            .official_daily_usage(account, Some(&start.to_string()), Some(&end.to_string()))?
            .into_iter()
            .map(|bucket| (bucket.start_date, bucket.tokens))
            .collect::<BTreeMap<_, _>>();
        official_by_account.insert(account.clone(), official);
        let local = store
            .aggregate_rollup_by(
                AggregateDimension::Day,
                &AggregateFilter {
                    start_inclusive: start_utc,
                    end_exclusive: end_utc,
                    account_fingerprint: Some(account.clone()),
                    project_id: None,
                    model: None,
                    quality: Some(DataQuality::Confirmed),
                },
            )?
            .into_iter()
            .filter_map(|bucket| bucket.key.map(|date| (date, bucket.usage.total_tokens)))
            .collect::<BTreeMap<_, _>>();
        local_by_account.insert(account.clone(), local);
    }

    let mut days = Vec::new();
    let mut day = start;
    while day < end {
        let date = day.to_string();
        let mut official_tokens = 0_u64;
        let mut local_tail_tokens = 0_u64;
        let mut local_only_tokens = 0_u64;
        let mut covered_accounts = 0_u64;
        let mut has_local_tail = false;
        let mut has_local_only_account = false;
        let mut has_unknown_gap = known_account_count > accounts.len() as u64;

        for account in accounts {
            let covered = coverage_start
                .get(account)
                .zip(coverage_through.get(account))
                .is_some_and(|(start, through)| start <= &date && &date <= through);
            if covered {
                covered_accounts = covered_accounts.saturating_add(1);
                official_tokens = official_tokens.saturating_add(
                    official_by_account
                        .get(account)
                        .and_then(|values| values.get(&date))
                        .copied()
                        .unwrap_or_default(),
                );
                continue;
            }

            let local = local_by_account
                .get(account)
                .and_then(|values| values.get(&date))
                .copied()
                .unwrap_or_default();
            match (coverage_start.get(account), coverage_through.get(account)) {
                (None, None) => {
                    has_local_only_account = true;
                    local_only_tokens = local_only_tokens.saturating_add(local);
                }
                (_, Some(through)) if &date > through => {
                    has_local_tail = true;
                    local_tail_tokens = local_tail_tokens.saturating_add(local);
                }
                _ => {
                    has_unknown_gap = true;
                    local_only_tokens = local_only_tokens.saturating_add(local);
                }
            }
        }

        let status = if covered_accounts == known_account_count {
            AccountDayStatus::ExactOfficial
        } else if has_local_only_account {
            AccountDayStatus::LocalOnlyAccount
        } else if has_unknown_gap {
            AccountDayStatus::Unknown
        } else if has_local_tail {
            AccountDayStatus::LocalTail
        } else {
            AccountDayStatus::Unknown
        };
        days.push(ReconciledAccountDay {
            date,
            value: official_tokens
                .saturating_add(local_tail_tokens)
                .saturating_add(local_only_tokens),
            official_tokens,
            local_tail_tokens,
            local_only_tokens,
            status,
            covered_accounts,
            known_accounts: known_account_count,
        });
        let Some(next) = day.succ_opt() else {
            break;
        };
        day = next;
    }
    Ok(days)
}

fn aggregate_reconciled_account_days(
    points: Vec<ReconciledAccountDay>,
    grain: &str,
) -> Vec<ReconciledAccountDay> {
    if grain == "day" {
        return points;
    }
    let status_rank = |status: AccountDayStatus| match status {
        AccountDayStatus::ExactOfficial => 0_u8,
        AccountDayStatus::LocalTail => 1,
        AccountDayStatus::Unknown => 2,
        AccountDayStatus::LocalOnlyAccount => 3,
    };
    let mut aggregated = BTreeMap::<String, ReconciledAccountDay>::new();
    for point in points {
        let key = aggregate_date_key(&point.date, grain).unwrap_or_else(|| point.date.clone());
        let entry = aggregated
            .entry(key.clone())
            .or_insert(ReconciledAccountDay {
                date: key,
                value: 0,
                official_tokens: 0,
                local_tail_tokens: 0,
                local_only_tokens: 0,
                status: AccountDayStatus::ExactOfficial,
                covered_accounts: point.covered_accounts,
                known_accounts: point.known_accounts,
            });
        entry.value = entry.value.saturating_add(point.value);
        entry.official_tokens = entry.official_tokens.saturating_add(point.official_tokens);
        entry.local_tail_tokens = entry
            .local_tail_tokens
            .saturating_add(point.local_tail_tokens);
        entry.local_only_tokens = entry
            .local_only_tokens
            .saturating_add(point.local_only_tokens);
        if status_rank(point.status) > status_rank(entry.status) {
            entry.status = point.status;
        }
        entry.covered_accounts = entry.covered_accounts.min(point.covered_accounts);
        entry.known_accounts = entry.known_accounts.max(point.known_accounts);
    }
    aggregated.into_values().collect()
}

pub(super) fn official_day_bounds(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    timezone: Tz,
) -> (Option<String>, Option<String>) {
    let start_day = start.map(|value| value.with_timezone(&timezone).date_naive().to_string());
    let end_day = end.map(|value| {
        let local = value.with_timezone(&timezone);
        if local.time() == NaiveTime::MIN {
            local.date_naive()
        } else {
            local.date_naive().succ_opt().unwrap_or(local.date_naive())
        }
        .to_string()
    });
    (start_day, end_day)
}

pub(super) fn aggregate_official_points(
    points: BTreeMap<String, u64>,
    grain: &str,
) -> BTreeMap<String, u64> {
    let mut aggregated = BTreeMap::<String, u64>::new();
    for (date, tokens) in points {
        let key = aggregate_date_key(&date, grain).unwrap_or(date);
        let entry = aggregated.entry(key).or_default();
        *entry = entry.saturating_add(tokens);
    }
    aggregated
}

pub(super) fn aggregate_date_key(date: &str, grain: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?;
    match grain {
        "week" => Some(
            (date - ChronoDuration::days(date.weekday().num_days_from_monday() as i64)).to_string(),
        ),
        "month" => Some(format!("{:04}-{:02}-01", date.year(), date.month())),
        _ => Some(date.to_string()),
    }
}
