use super::*;

pub fn snapshot_from_store(
    store: &LedgerStore,
    query: &UsageQuery,
) -> Result<DashboardSnapshot, StoreError> {
    let (start, end, period) = resolve_period(query);
    let base = AggregateFilter {
        start_inclusive: start,
        end_exclusive: end,
        account_fingerprint: selected(&query.account),
        project_id: selected(&query.project),
        model: selected(&query.model),
        quality: Some(DataQuality::Confirmed),
    };

    let confirmed = store.aggregate_rollup_usage(&base)?;
    let quarantined = aggregate_for_quality(store, &base, DataQuality::Quarantined)?;
    let unknown = aggregate_for_quality(store, &base, DataQuality::Unknown)?;

    let by_model = breakdown(store, AggregateDimension::Model, &base, "unknown model")?;
    let by_account = breakdown(store, AggregateDimension::Account, &base, "unknown account")?;
    let by_project = breakdown(store, AggregateDimension::Project, &base, "unassigned")?;
    let day_buckets = store.aggregate_rollup_by(AggregateDimension::Day, &base)?;
    let timeseries = day_buckets
        .into_iter()
        .map(|bucket| SeriesPoint {
            bucket: bucket.key.unwrap_or_else(|| "unknown".into()),
            usage: bucket.usage.into(),
            quarantined_tokens: 0,
            unknown_tokens: 0,
        })
        .collect();

    Ok(DashboardSnapshot {
        schema_version: 1,
        revision: Utc::now().timestamp_millis().max(0) as u64,
        generated_at: Utc::now(),
        period,
        trusted_usage: confirmed.usage.into(),
        quality: QualitySummary {
            confirmed: confirmed.usage.into(),
            quarantined: quarantined.usage.into(),
            unknown: unknown.usage.into(),
            confirmed_events: confirmed.event_count,
            quarantined_events: quarantined.event_count,
            unknown_events: unknown.event_count,
            missing_model_events: 0,
            missing_account_events: 0,
            replay_events_removed: 0,
        },
        timeseries,
        by_model,
        by_account,
        by_project,
        quota_pools: Vec::new(),
        account_switches: Vec::new(),
        source_freshness: None,
    })
}

pub(super) fn aggregate_for_quality(
    store: &LedgerStore,
    base: &AggregateFilter,
    quality: DataQuality,
) -> Result<UsageAggregate, StoreError> {
    let mut filter = base.clone();
    filter.quality = Some(quality);
    store.aggregate_rollup_usage(&filter)
}

fn breakdown(
    store: &LedgerStore,
    dimension: AggregateDimension,
    filter: &AggregateFilter,
    unknown_label: &str,
) -> Result<Vec<BreakdownItem>, StoreError> {
    Ok(store
        .aggregate_rollup_by(dimension, filter)?
        .into_iter()
        .map(|bucket| {
            let known = bucket.key.is_some();
            let key = bucket.key.unwrap_or_else(|| "unknown".into());
            BreakdownItem {
                label: match (dimension, key.as_str()) {
                    (AggregateDimension::Project, STANDALONE_PROJECT_ID) => {
                        STANDALONE_PROJECT_LABEL.to_owned()
                    }
                    (AggregateDimension::Project, UNASSIGNED_PROJECT_ID) => {
                        UNASSIGNED_PROJECT_LABEL.to_owned()
                    }
                    (_, "unknown") => unknown_label.into(),
                    _ => key.clone(),
                },
                key,
                usage: bucket.usage.into(),
                event_count: bucket.event_count,
                confidence: if known {
                    "observed".into()
                } else {
                    "unknown".into()
                },
            }
        })
        .collect())
}
