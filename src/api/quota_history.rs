use super::*;

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
