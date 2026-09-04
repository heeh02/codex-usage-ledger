use super::*;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
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
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
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
            Self::InvalidAccountCount(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({"error": self.to_string()}))).into_response()
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
        .route("/v1/bundle", get(bundle))
        .route("/v1/quotas", get(quotas))
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
    Json(serde_json::json!({"status": "ok", "service": "codex-usage-ledger"}))
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
    Ok(Json(
        state
            .cached_query_value("bundle", query, http_bundle)
            .await?,
    ))
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
