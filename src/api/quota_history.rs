use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct IntervalUsageQuery {
    selection: String,
}

pub(super) async fn interval_usage(
    State(state): State<ApiState>,
    Query(query): Query<IntervalUsageQuery>,
) -> Result<Json<wire::QuotaIntervalUsageResponse>, ApiError> {
    if query.selection.len() > 8192 {
        return Err(ApiError::InvalidQuery(
            "quota selection is too large".into(),
        ));
    }
    let selection: crate::store::QuotaHistoryCursor = serde_json::from_str(&query.selection)
        .map_err(|_| ApiError::InvalidQuery("invalid quota selection".into()))?;
    let value=state.query_read_only(move |store| store.with_source_audit_snapshot(|store| {
        let interval=store.selected_quota_history_interval(&selection)?;
        let (observations, observations_truncated)=store.quota_interval_observations(&selection)?;
        let mut response=serde_json::json!({
            "scope":"quota_interval_local_evidence_v1","status":"no_safe_interval",
            "intervalId":interval.id,"accountId":interval.account_id,"quotaView":selection.view,"observedAt":Utc::now(),
            "start":interval.token_sample_start,"end":interval.token_sample_end,"coverageComplete":false,"poolAttribution":false,
            "events":null,"usage":null,"models":[],"projects":[],
            "chart":{"observations":observations,"observationsTruncated":observations_truncated,"buckets":[]},
        });
        let (Some(start),Some(end))=(interval.token_sample_start,interval.token_sample_end) else {return Ok(response);};
        if store.quota_interval_needs_source_review(&interval.account_id,start,end)? {
            response["status"]=serde_json::json!("source_overlap_review");return Ok(response);
        }
        if !store.usage_projection_ready()? {response["status"]=serde_json::json!("pending");return Ok(response);}
        let filter=AggregateFilter {start_inclusive:Some(start),end_exclusive:Some(end),account_fingerprint:Some(interval.account_id),quality:Some(DataQuality::Confirmed),..Default::default()};
        let total=queries::aggregate_exact_hour_window(store,&filter)?;
        if total.event_count==0 {response["status"]=serde_json::json!("no_evidence");return Ok(response);}
        total.usage.validate().map_err(StoreError::InvalidConfirmedUsage)?;
        let series=store.aggregate_exact_time_series(crate::store::TimeGrain::Hour,None,&filter,"UTC")?;
        let mut bucket_total=TokenUsage::default();
        let mut bucket_events=0_u64;
        let mut buckets=Vec::new();
        for bucket in series {
            let at=chrono::NaiveDateTime::parse_from_str(&bucket.time_key,"%Y-%m-%dT%H:%M")
                .map_err(|_|StoreError::InvalidRequestQuery("invalid interval hour"))?.and_utc();
            let bucket_start=at.max(start);
            let bucket_end=(at+ChronoDuration::hours(1)).min(end);
            if bucket_start>=bucket_end {return Err(StoreError::QuotaIntervalMismatch);}
            add_usage_saturating(&mut bucket_total,bucket.usage);
            bucket_events=bucket_events.saturating_add(bucket.event_count);
            buckets.push(serde_json::json!({"start":bucket_start,"end":bucket_end,"events":bucket.event_count,"usage":token_value(bucket.usage)}));
        }
        if bucket_total!=total.usage || bucket_events!=total.event_count {return Err(StoreError::QuotaIntervalMismatch);}
        response["chart"]["buckets"]=serde_json::json!(buckets);
        let projects=store.list_projects()?.into_iter().map(|row|(row.project_id,row.project_name)).collect::<BTreeMap<_,_>>();
        let components=|usage:&TokenUsage,events:u64| [usage.input_tokens,usage.cached_input_tokens,usage.cache_write_input_tokens,usage.cache_write_observed_input_tokens,usage.output_tokens,usage.reasoning_output_tokens,usage.total_tokens,events].map(u128::from);
        for (name,dimension) in [("models",AggregateDimension::Model),("projects",AggregateDimension::Project)] {
            let buckets=queries::aggregate_exact_hour_window_by(store,dimension,&filter,"Asia/Shanghai")?;
            let mut sum=[0_u128;8];
            for row in &buckets { for (target,value) in sum.iter_mut().zip(components(&row.usage,row.event_count)) {*target+=value;} }
            if sum!=components(&total.usage,total.event_count) {return Err(StoreError::QuotaIntervalMismatch);}
            response[name]=serde_json::Value::Array(buckets.into_iter().map(|row| {
                let label=if name=="projects" {row.key.as_ref().and_then(|id|projects.get(id)).cloned()}else{row.key.clone()};
                serde_json::json!({"id":row.key,"label":label,"events":row.event_count,"usage":token_value(row.usage)})
            }).collect());
        }
        response["status"]=serde_json::json!("available");response["events"]=serde_json::json!(total.event_count);response["usage"]=token_value(total.usage);
        Ok(response)
    })).await.map_err(|error|match error {ApiError::Store(StoreError::InvalidRequestQuery(message))=>ApiError::InvalidQuery(message.into()),other=>other})?;
    Ok(Json(
        serde_json::from_value(value).map_err(StoreError::from)?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HistoryQuery {
    account: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

pub(super) async fn history(
    State(state): State<ApiState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<wire::QuotaHistoryResponse>, ApiError> {
    let account = query.account.as_deref().unwrap_or("all").trim().to_owned();
    let limit = query.limit.unwrap_or(20);
    if account.is_empty()
        || account.len() > 256
        || !(1..=100).contains(&limit)
        || query
            .cursor
            .as_ref()
            .is_some_and(|value| value.len() > 8192)
    {
        return Err(ApiError::InvalidQuery("invalid quota history query".into()));
    }
    let cursor: Option<crate::store::QuotaHistoryCursor> = query
        .cursor
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| ApiError::InvalidQuery("invalid quota history cursor".into()))?;
    let value = state
        .query_read_only(move |store| {
            let mut value = serde_json::to_value(store.quota_history_page(
                &account,
                cursor.as_ref(),
                limit,
            )?)?;
            value["scope"] = serde_json::json!("quota_observation_history_v1");
            Ok(value)
        })
        .await
        .map_err(|error| match error {
            ApiError::Store(StoreError::InvalidRequestQuery(message)) => {
                ApiError::InvalidQuery(message.into())
            }
            other => other,
        })?;
    Ok(Json(
        serde_json::from_value(value).map_err(StoreError::from)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::normalize_rate_limit_event;

    #[tokio::test]
    async fn interval_without_safe_range_or_evidence_never_claims_measured_zero() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let now = Utc::now();
        for (minutes, used) in [(60, 20), (30, 10)] {
            let snapshot=normalize_rate_limit_event(&serde_json::json!({"limit_id":"pool","primary":{"used_percent":used,"window_minutes":10080,"resets_at":(now+ChronoDuration::days(1)).timestamp()}})).unwrap();
            store
                .append_quota_snapshot(
                    "account-a",
                    "epoch",
                    now - ChronoDuration::minutes(minutes),
                    &snapshot,
                )
                .unwrap();
        }
        let page = store.quota_history_page("account-a", None, 20).unwrap();
        assert_eq!(page.intervals.len(), 2);
        store.with_usage_snapshot(|_| Ok(())).unwrap();
        let state = ApiState::with_store(store);
        for (index, expected) in [(0, "no_evidence"), (1, "no_safe_interval")] {
            let result = interval_usage(
                State(state.clone()),
                Query(IntervalUsageQuery {
                    selection: serde_json::to_string(&page.selections[index]).unwrap(),
                }),
            )
            .await
            .unwrap()
            .0;
            let value = serde_json::to_value(result).unwrap();
            assert_eq!(value["status"], expected);
            assert!(value["usage"].is_null());
            assert!(value["events"].is_null());
            assert_eq!(value["models"].as_array().unwrap().len(), 0);
        }
    }

    #[tokio::test]
    async fn interval_usage_uses_signed_bounds_account_and_conserved_dimensions() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let now = Utc::now();
        let start = now - ChronoDuration::hours(2);
        let snapshot=normalize_rate_limit_event(&serde_json::json!({"limit_id":"pool-a","primary":{"used_percent":20,"window_minutes":10080,"resets_at":(now+ChronoDuration::days(1)).timestamp()}})).unwrap();
        store
            .append_quota_snapshot("account-a", "epoch", start, &snapshot)
            .unwrap();
        for (id, account, at, model, project) in [
            (
                "inside-a",
                "account-a",
                start + ChronoDuration::minutes(10),
                "model-a",
                "project-a",
            ),
            (
                "inside-b",
                "account-a",
                start + ChronoDuration::minutes(20),
                "model-b",
                "project-b",
            ),
            (
                "other-account",
                "account-b",
                start + ChronoDuration::minutes(20),
                "other",
                "other",
            ),
            (
                "before",
                "account-a",
                start - ChronoDuration::minutes(1),
                "other",
                "other",
            ),
            (
                "future",
                "account-a",
                now + ChronoDuration::hours(1),
                "other",
                "other",
            ),
        ] {
            let mut event = super::super::tests::explorer_event(id, "thread", None);
            event.observed_at = at;
            event.source_timestamp = Some(at);
            event.account_fingerprint = Some(account.into());
            event.model = Some(model.into());
            event.project.project_id = Some(project.into());
            store.upsert_event(&event).unwrap();
        }
        let page = store.quota_history_page("all", None, 20).unwrap();
        let selection = serde_json::to_string(&page.selections[0]).unwrap();
        let state = ApiState::with_store(store);
        let pending = interval_usage(
            State(state.clone()),
            Query(IntervalUsageQuery {
                selection: selection.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(pending.status, "pending");
        assert!(serde_json::to_value(pending).unwrap()["usage"].is_null());
        state
            .store
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .with_usage_snapshot(|_| Ok(()))
            .unwrap();
        let result = interval_usage(
            State(state.clone()),
            Query(IntervalUsageQuery {
                selection: selection.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["status"], "available");
        assert_eq!(value["accountId"], "account-a");
        assert_eq!(value["events"], 2);
        assert_eq!(value["usage"]["total"].as_f64(), Some(240.0));
        let chart = &value["chart"];
        assert_eq!(chart["observationsTruncated"], false);
        let buckets = chart["buckets"].as_array().unwrap();
        assert_eq!(
            buckets
                .iter()
                .map(|b| b["usage"]["total"].as_f64().unwrap())
                .sum::<f64>(),
            240.0
        );
        assert_eq!(
            buckets
                .iter()
                .map(|b| b["events"].as_u64().unwrap())
                .sum::<u64>(),
            2
        );
        assert_eq!(value["poolAttribution"], false);
        for dimension in ["models", "projects"] {
            let rows = value[dimension].as_array().unwrap();
            assert_eq!(rows.len(), 2);
            assert_eq!(
                rows.iter()
                    .map(|row| row["usage"]["total"].as_f64().unwrap())
                    .sum::<f64>(),
                240.0
            );
        }
        let mut altered = page.selections[0].clone();
        altered.before.at = now.to_rfc3339();
        assert!(matches!(
            interval_usage(
                State(state),
                Query(IntervalUsageQuery {
                    selection: serde_json::to_string(&altered).unwrap()
                })
            )
            .await,
            Err(ApiError::InvalidQuery(_))
        ));
    }

    #[tokio::test]
    async fn quota_history_endpoint_pages_and_rejects_cross_account_cursor() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let at = Utc::now() - ChronoDuration::days(2);
        for index in 0..45 {
            let snapshot=normalize_rate_limit_event(&serde_json::json!({"limit_id":"pool-a","primary":{
                "used_percent":20,"window_minutes":10080,"resets_at":(at+ChronoDuration::hours(index+1)).timestamp()
            }})).unwrap();
            store
                .append_quota_snapshot(
                    "account-a",
                    "epoch",
                    at + ChronoDuration::minutes(index),
                    &snapshot,
                )
                .unwrap();
        }
        let state = ApiState::with_store(store);
        let first = history(
            State(state.clone()),
            Query(HistoryQuery {
                account: Some("all".into()),
                cursor: None,
                limit: Some(20),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(first.intervals.len(), 20);
        assert!(first.index_ready);
        assert!(!first.source_history_complete);
        let json = serde_json::to_value(&first).unwrap();
        let cursor = serde_json::to_string(&json["next"]).unwrap();
        let second = history(
            State(state.clone()),
            Query(HistoryQuery {
                account: Some("all".into()),
                cursor: Some(cursor.clone()),
                limit: Some(20),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(second.intervals.len(), 20);
        assert_eq!(second.view.revision, first.view.revision);
        assert_ne!(first.intervals[0].id, second.intervals[0].id);
        assert!(matches!(
            history(
                State(state),
                Query(HistoryQuery {
                    account: Some("account-a".into()),
                    cursor: Some(cursor),
                    limit: Some(20)
                })
            )
            .await,
            Err(ApiError::InvalidQuery(_))
        ));
    }

    #[tokio::test]
    async fn quota_history_endpoint_does_not_recreate_missing_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quota.sqlite3");
        let state = ApiState::with_store(LedgerStore::open(&path).unwrap());
        std::fs::remove_file(&path).unwrap();
        assert!(
            history(
                State(state),
                Query(HistoryQuery {
                    account: None,
                    cursor: None,
                    limit: None
                })
            )
            .await
            .is_err()
        );
        assert!(!path.exists());
    }
}
