use super::*;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
    /// Internal bundle clock; never accepted from or exposed to HTTP clients.
    #[serde(skip)]
    #[doc(hidden)]
    pub reference_time: Option<DateTime<Utc>>,
    pub period: Option<String>,
    pub account: Option<String>,
    pub project: Option<String>,
    pub model: Option<String>,
    pub timezone: Option<String>,
    pub dimension: Option<String>,
    pub session: Option<String>,
    pub grain: Option<String>,
    pub metric: Option<String>,
    pub ranking_period: Option<String>,
    pub ranking_sort: Option<String>,
    pub session_search: Option<String>,
    pub session_sort: Option<String>,
    pub session_offset: Option<usize>,
    pub session_limit: Option<usize>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub node_offset: Option<usize>,
    pub node_limit: Option<usize>,
    pub node_search: Option<String>,
}

impl UsageQuery {
    pub(super) fn validate(&self) -> Result<(), ApiError> {
        if self
            .node_limit
            .is_some_and(|limit| !(1..=1000).contains(&limit))
        {
            return Err(ApiError::InvalidQuery(
                "nodeLimit must be between 1 and 1000".to_owned(),
            ));
        }
        for (name, value, allowed) in [
            (
                "period",
                self.period.as_deref(),
                &[
                    "today",
                    "week",
                    "rolling7",
                    "month",
                    "rolling30",
                    "weeks12",
                    "months12",
                    "year",
                    "custom",
                    "lifetime",
                ][..],
            ),
            (
                "grain",
                self.grain.as_deref(),
                &["hour", "day", "week", "month"][..],
            ),
            (
                "metric",
                self.metric.as_deref(),
                &[
                    "total",
                    "input",
                    "cached",
                    "cacheWrite",
                    "uncached",
                    "output",
                    "reasoning",
                    "requests",
                ][..],
            ),
            (
                "sessionSort",
                self.session_sort.as_deref(),
                &["tokens", "output", "requests", "recent"][..],
            ),
        ] {
            if value.is_some_and(|value| !allowed.contains(&value)) {
                return Err(ApiError::InvalidQuery(format!("unsupported {name}")));
            }
        }
        if self.period.as_deref() == Some("custom") {
            let parse = |value: Option<&str>| {
                value.and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            };
            let dates = parse(self.start_date.as_deref()).zip(parse(self.end_date.as_deref()));
            if !dates.is_some_and(|(start, end)| start <= end && end.succ_opt().is_some()) {
                return Err(ApiError::InvalidQuery(
                    "custom requires ordered startDate and endDate (YYYY-MM-DD)".to_owned(),
                ));
            }
            let timezone = self
                .timezone
                .as_deref()
                .unwrap_or("Asia/Shanghai")
                .parse::<chrono_tz::Tz>()
                .map_err(|_| ApiError::InvalidQuery("unsupported timezone".to_owned()))?;
            let (start, end) = dates.expect("ordered dates validated above");
            if super::period::local_midnight_utc(start, timezone).is_none()
                || end
                    .succ_opt()
                    .and_then(|date| super::period::local_midnight_utc(date, timezone))
                    .is_none()
            {
                return Err(ApiError::InvalidQuery(
                    "custom date boundary does not exist in the selected timezone".to_owned(),
                ));
            }
        }
        if self
            .session_limit
            .is_some_and(|limit| !(1..=100).contains(&limit))
        {
            return Err(ApiError::InvalidQuery(
                "sessionLimit must be between 1 and 100".to_owned(),
            ));
        }
        if self
            .session_search
            .as_ref()
            .is_some_and(|search| search.chars().count() > 256)
        {
            return Err(ApiError::InvalidQuery(
                "sessionSearch exceeds 256 characters".to_owned(),
            ));
        }
        if self
            .timezone
            .as_deref()
            .is_some_and(|timezone| Tz::from_str(timezone).is_err())
        {
            return Err(ApiError::InvalidQuery(
                "timezone must be a valid IANA name".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("ledger database lock was poisoned")]
    StorePoisoned,
    #[error("ledger query worker stopped")]
    WorkerStopped,
    #[error("ledger database is not configured for this endpoint")]
    StoreUnavailable,
    #[error("no active Codex account can be identified safely")]
    ActiveAccountUnavailable,
    #[error("official Codex usage is unavailable: {0}")]
    OfficialUsage(String),
    #[error("a concrete session id is required")]
    SessionRequired,
    #[error("invalid user-confirmed account count: {0}")]
    InvalidAccountCount(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::Store(StoreError::InsufficientTimePrecision) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidAccountCount(_) | Self::InvalidQuery(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = match &self {
            Self::Store(StoreError::InsufficientTimePrecision) => "insufficient_time_precision",
            Self::InvalidAccountCount(_) | Self::InvalidQuery(_) => "invalid_query",
            _ => "request_failed",
        };
        (
            status,
            Json(serde_json::json!({"error": self.to_string(), "code": code})),
        )
            .into_response()
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/summary", get(summary))
        .route("/v1/timeseries", get(timeseries))
        .route("/v1/breakdowns", get(breakdowns))
        .route("/v1/quality", get(quality))
        .route("/v1/explorer", get(explorer))
        .route(
            "/v1/request-evidence",
            get(super::requests::request_evidence),
        )
        .route("/v1/bundle", get(bundle))
        .route("/v1/source-union", get(source_union))
        .route("/v1/turn-evidence", get(super::requests::turn_evidence))
        .route("/v1/quotas", get(quotas))
        .route("/v1/quota-history", get(super::quota_history::history))
        .route(
            "/v1/quota-interval-usage",
            get(super::quota_history::interval_usage),
        )
        .route("/v1/switches", get(switches))
        .route("/v1/changes", get(changes))
        .route(
            "/v1/account-registry",
            get(account_registry_status).post(update_account_registry),
        )
        .route("/v1/official/refresh", post(refresh_official))
        .route("/v1/official/thread/refresh", post(refresh_official_thread))
        .with_state(state)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountRegistryUpdate {
    user_confirmed_account_count: Option<u64>,
}

async fn account_registry_status(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .query_value(UsageQuery::default(), |store, _| {
                account_registry_value(store)
            })
            .await?,
    ))
}

async fn update_account_registry(
    State(state): State<ApiState>,
    Json(update): Json<AccountRegistryUpdate>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if update
        .user_confirmed_account_count
        .is_some_and(|count| !(1..=64).contains(&count))
    {
        return Err(ApiError::InvalidAccountCount(
            "must be between 1 and 64, or null to clear".to_owned(),
        ));
    }
    let store = state.store.clone().ok_or(ApiError::StoreUnavailable)?;
    let response = tokio::task::spawn_blocking(move || {
        let mut guard = store.lock().map_err(|_| ApiError::StorePoisoned)?;
        guard
            .set_user_confirmed_account_count(update.user_confirmed_account_count)
            .map_err(ApiError::from)?;
        account_registry_value(&guard)
            .map(Json)
            .map_err(ApiError::from)
    })
    .await
    .map_err(|_| ApiError::WorkerStopped)??;
    state.invalidate_query_cache();
    Ok(response)
}

async fn refresh_official(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = state.refresh_official_usage().await?;
    state.invalidate_query_cache();
    Ok(Json(response))
}

async fn refresh_official_thread(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let thread_id = selected(&query.session).ok_or(ApiError::SessionRequired)?;
    let response = state.refresh_official_thread_usage(thread_id).await?;
    state.invalidate_query_cache();
    Ok(Json(response))
}

async fn health() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"status": "ok", "service": "codex-usage-ledger", "processId": std::process::id()}),
    )
}

async fn summary(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .cached_query_value("summary", query, http_summary)
            .await?,
    ))
}

pub(super) async fn source_union(
    State(state): State<ApiState>,
    Query(query): Query<crate::store::SourceUnionQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if query.start >= query.end || query.timezone.parse::<Tz>().is_err() {
        return Err(ApiError::InvalidQuery(
            "require ordered timestamps and a valid timezone".into(),
        ));
    }
    Ok(Json(
        state
            .query_read_only(move |store| {
                Ok(serde_json::to_value(
                    store.read_source_union_projection(&query)?,
                )?)
            })
            .await?,
    ))
}

async fn timeseries(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .cached_query_value("timeseries", query, http_timeseries)
            .await?,
    ))
}

async fn breakdowns(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .cached_query_value("breakdowns", query, http_breakdowns)
            .await?,
    ))
}

async fn quality(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .cached_query_value("quality", query, http_quality)
            .await?,
    ))
}

async fn explorer(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .cached_query_value("explorer", query, http_explorer)
            .await?,
    ))
}

async fn bundle(
    State(state): State<ApiState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(state.bundle_json(query).await?))
}

async fn quotas(State(state): State<ApiState>) -> Json<serde_json::Value> {
    match state
        .query_value(UsageQuery::default(), |store, query| {
            Ok(serde_json::json!({"pools": quota_views(store, query)?}))
        })
        .await
    {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({"error": error.to_string(), "pools": []})),
    }
}

async fn switches(State(state): State<ApiState>) -> Json<serde_json::Value> {
    match state
        .query_value(UsageQuery::default(), |store, query| {
            Ok(serde_json::json!({"items": timeline_views(store, query)?}))
        })
        .await
    {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({"error": error.to_string(), "items": []})),
    }
}

async fn changes(State(state): State<ApiState>, headers: HeaderMap) -> impl IntoResponse {
    if !accepts_local_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let stream = async_stream::stream! {
        loop {
            let revision = state
                .query_value(UsageQuery::default(), |store, _| {
                    Ok(serde_json::json!({
                        "revision": store.dashboard_revision()?
                    }))
                })
                .await
                .ok()
                .and_then(|value| value.get("revision").cloned())
                .unwrap_or_else(|| serde_json::Value::String("unavailable".to_owned()));
            let payload = serde_json::json!({
                "revision": revision,
                "generatedAt": Utc::now(),
            });
            yield Ok::<Event, Infallible>(
                Event::default()
                    .event("ledger-change")
                    .data(payload.to_string()),
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    };
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}

pub(super) fn accepts_local_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin == "null"
}

#[cfg(test)]
mod health_tests {
    #[tokio::test]
    async fn health_identifies_the_serving_process_without_ledger_data() {
        let response = super::health().await.0;
        assert_eq!(response["processId"], std::process::id());
        assert_eq!(response["status"], "ok");
        assert_eq!(response["service"], "codex-usage-ledger");
        assert_eq!(response.as_object().unwrap().len(), 3);
    }
}
