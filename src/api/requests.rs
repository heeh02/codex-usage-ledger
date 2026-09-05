use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TurnEvidenceQuery {
    thread_id: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    account: Option<String>,
    model: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

pub(super) async fn turn_evidence(
    State(state): State<ApiState>,
    Query(query): Query<TurnEvidenceQuery>,
) -> Result<Json<wire::TurnEvidenceResponse>, ApiError> {
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    if query.thread_id.is_empty()
        || query.start >= query.end
        || !(1..=500).contains(&limit)
        || offset > i64::MAX as usize
    {
        return Err(ApiError::InvalidQuery(
            "invalid turn-evidence scope or pagination".into(),
        ));
    }
    let value=state.query_value(UsageQuery::default(),move |store,_| {
        let page=store.retained_turn_page(crate::store::RetainedRequestScope {
            thread_id:&query.thread_id,start:query.start,end:query.end,
            account:query.account.as_deref().filter(|value|*value!="all"),
            model:query.model.as_deref().filter(|value|*value!="all"),
        },offset,limit)?;
        Ok(serde_json::json!({
            "scope":"thread_own_retained_turns","historyComplete":false,
            "threadId":query.thread_id,"start":query.start,"end":query.end,
            "selectedAccount":selected(&query.account),"selectedModel":selected(&query.model),
            "nextOffset":page.next_offset,
            "rows":page.observations.into_iter().map(|row|serde_json::json!({
                "groupId":row.group_id,"turnId":row.turn_id,"firstAt":row.first_at,"lastAt":row.last_at,
                "requestCount":row.request_count,"confirmedRequestCount":row.confirmed_request_count,
                "confirmedUsage":if row.confirmed_request_count>0 {token_value(row.usage)} else {serde_json::Value::Null},
            })).collect::<Vec<_>>()
        }))
    }).await?;
    Ok(Json(
        serde_json::from_value(value).map_err(StoreError::from)?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RequestEvidenceQuery {
    thread_id: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    after_time: Option<String>,
    after_id: Option<String>,
    limit: Option<usize>,
    account: Option<String>,
    model: Option<String>,
}

pub(super) async fn request_evidence(
    State(state): State<ApiState>,
    Query(query): Query<RequestEvidenceQuery>,
) -> Result<Json<wire::RequestEvidenceResponse>, ApiError> {
    let limit = query.limit.unwrap_or(100);
    if query.thread_id.is_empty()
        || query.start >= query.end
        || !(1..=500).contains(&limit)
        || query.after_time.is_some() != query.after_id.is_some()
    {
        return Err(ApiError::InvalidQuery(
            "invalid request-evidence scope or cursor".into(),
        ));
    }
    let cursor = query
        .after_time
        .zip(query.after_id)
        .map(
            |(effective_at, event_id)| crate::store::RetainedRequestCursor {
                effective_at,
                event_id,
            },
        );
    let value = state
        .query_value(UsageQuery::default(), move |store, _| {
            let page = store.retained_request_page_for_scope(
                crate::store::RetainedRequestScope {
                    thread_id: &query.thread_id,
                    start: query.start,
                    end: query.end,
                    account: query.account.as_deref().filter(|value| *value != "all"),
                    model: query.model.as_deref().filter(|value| *value != "all"),
                },
                cursor.as_ref(),
                limit,
            )?;
            Ok(serde_json::json!({
                "scope": "thread_own_retained_observations",
                "attribution": "ingest_observed",
                "selectionAttribution": "current_ledger",
                "selectedAccount": selected(&query.account),
                "selectedModel": selected(&query.model),
                "historyComplete": false,
                "threadId": query.thread_id,
                "start": query.start, "end": query.end,
                "next": page.next.map(|cursor| serde_json::json!({
                    "afterTime": cursor.effective_at, "afterId": cursor.event_id
                })),
                "rows": page.observations.into_iter().map(|row| serde_json::json!({
                    "id": row.cursor.event_id, "at": row.cursor.effective_at,
                    "turnId": row.turn_id, "model": row.model,
                    "observedAccount": row.observed_account,
                    "observedProject": row.observed_project,
                    "accountConfidence": row.observed_account_confidence,
                    "projectConfidence": row.observed_project_confidence,
                    "quality": row.quality, "usage": token_value(row.usage)
                })).collect::<Vec<_>>()
            }))
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

    #[tokio::test]
    async fn turn_endpoint_keeps_unknown_usage_null() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let at = Utc::now();
        let mut event = super::super::tests::explorer_event("unknown-turn-request", "thread", None);
        event.source_timestamp = Some(at);
        event.quality = DataQuality::Unknown;
        store.upsert_event(&event).unwrap();
        let result = turn_evidence(
            State(ApiState::with_store(store)),
            Query(TurnEvidenceQuery {
                thread_id: "thread".into(),
                start: at - ChronoDuration::hours(1),
                end: at + ChronoDuration::hours(1),
                account: None,
                model: None,
                offset: None,
                limit: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["rows"][0]["confirmedUsage"], serde_json::Value::Null);
        assert_eq!(value["rows"][0]["requestCount"], 1);
        assert_eq!(value["rows"][0]["confirmedRequestCount"], 0);
        assert_eq!(value["rows"][0]["groupId"], "request:unknown-turn-request");
    }

    #[tokio::test]
    async fn request_endpoint_preserves_observation_scope_and_rejects_invalid_cursor() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let at = Utc::now();
        let mut event = super::super::tests::explorer_event("request", "thread", None);
        event.source_timestamp = Some(at);
        store.upsert_event(&event).unwrap();
        let state = ApiState::with_store(store);
        let make_query = || RequestEvidenceQuery {
            thread_id: "thread".into(),
            start: at - ChronoDuration::hours(1),
            end: at + ChronoDuration::hours(1),
            after_time: None,
            after_id: None,
            limit: Some(10),
            account: None,
            model: None,
        };
        let value = request_evidence(State(state.clone()), Query(make_query()))
            .await
            .unwrap()
            .0;
        let value = serde_json::to_value(value).unwrap();
        assert_eq!(value["scope"], "thread_own_retained_observations");
        assert_eq!(value["historyComplete"], false);
        assert_eq!(
            value["rows"][0]["usage"]["total"].as_f64(),
            Some(event.usage.total_tokens as f64)
        );
        assert_eq!(value["rows"][0]["turnId"], serde_json::Value::Null);
        let mut filtered = make_query();
        filtered.model = Some("absent-model".into());
        let filtered = request_evidence(State(state.clone()), Query(filtered))
            .await
            .unwrap()
            .0;
        assert!(filtered.rows.is_empty());
        let mut invalid = make_query();
        invalid.after_time = Some("bad-time".into());
        invalid.after_id = Some("request".into());
        let error = request_evidence(State(state), Query(invalid))
            .await
            .unwrap_err();
        assert_eq!(error.into_response().status(), StatusCode::BAD_REQUEST);
    }
}
