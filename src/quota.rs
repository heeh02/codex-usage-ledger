use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuotaError {
    #[error("quota payload must be a JSON object")]
    InvalidRoot,
    #[error("quota payload did not contain a supported rate-limit shape")]
    UnsupportedShape,
}

pub type QuotaResult<T> = Result<T, QuotaError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaSource {
    WhamUsage,
    TokenCountEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaPoolKind {
    Main,
    Additional,
    CodeReview,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowRole {
    Primary,
    Secondary,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaWindow {
    pub role: WindowRole,
    pub server_name: String,
    pub used_percent: Option<f64>,
    pub window_seconds: Option<u64>,
    pub resets_at_unix: Option<i64>,
    pub server_fields: BTreeMap<String, Value>,
}

impl QuotaWindow {
    pub fn remaining_percent(&self) -> Option<f64> {
        self.used_percent
            .map(|used| (100.0 - used).clamp(0.0, 100.0))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaPool {
    /// Stable only within the source schema. Account ownership must be supplied
    /// separately by the immutable auth snapshot used for the request.
    pub pool_key: String,
    pub kind: QuotaPoolKind,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub windows: Vec<QuotaWindow>,
    pub rate_limit_reached_type: Option<String>,
    pub server_fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditPool {
    pub has_credits: Option<bool>,
    pub unlimited: Option<bool>,
    /// Kept as a decimal string so balances are never rounded through `f64`.
    pub balance: Option<String>,
    pub server_fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub source: QuotaSource,
    pub plan_type: Option<String>,
    pub pools: Vec<QuotaPool>,
    pub credits: Option<CreditPool>,
    pub individual_limit: Option<Value>,
    pub spend_control_reached: Option<bool>,
    pub warnings: Vec<String>,
    pub server_fields: BTreeMap<String, Value>,
}

/// Auto-detects either the `wham/usage` response or Codex token-count
/// `payload.rate_limits` shape. Unknown pools and fields are preserved rather
/// than being silently collapsed into a hard-coded main/Spark model.
pub fn normalize_quota_payload(value: &Value) -> QuotaResult<QuotaSnapshot> {
    let root = value.as_object().ok_or(QuotaError::InvalidRoot)?;

    if root.contains_key("rate_limit")
        || root.contains_key("additional_rate_limits")
        || root.contains_key("code_review_rate_limit")
    {
        return normalize_usage_response(value);
    }

    if let Some(rate_limits) = root.get("rate_limits") {
        return normalize_rate_limit_event(rate_limits);
    }
    if let Some(rate_limits) = root
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("rate_limits"))
    {
        return normalize_rate_limit_event(rate_limits);
    }
    if root.contains_key("limit_id")
        || root.contains_key("primary")
        || root.contains_key("secondary")
    {
        return normalize_rate_limit_event(value);
    }

    Err(QuotaError::UnsupportedShape)
}

pub fn normalize_usage_response(value: &Value) -> QuotaResult<QuotaSnapshot> {
    let root = value.as_object().ok_or(QuotaError::InvalidRoot)?;
    let mut warnings = Vec::new();
    let mut pools = Vec::new();

    if let Some(rate_limit) = root.get("rate_limit") {
        if let Some(rate_limit) = rate_limit.as_object() {
            pools.push(pool_from_wham(
                "main".to_owned(),
                QuotaPoolKind::Main,
                string_field(root, "limit_id"),
                string_field(root, "limit_name"),
                rate_limit,
                &mut warnings,
            ));
            if let Some(main) = pools.last_mut()
                && main.rate_limit_reached_type.is_none()
            {
                main.rate_limit_reached_type = string_field(root, "rate_limit_reached_type");
            }
        } else if !rate_limit.is_null() {
            warnings.push("rate_limit was not an object".to_owned());
        }
    }

    if let Some(additional) = root.get("additional_rate_limits") {
        match additional.as_array() {
            Some(entries) => {
                for (index, entry) in entries.iter().enumerate() {
                    let Some(entry) = entry.as_object() else {
                        warnings.push(format!("additional_rate_limits[{index}] was not an object"));
                        continue;
                    };
                    let limit_id = string_field(entry, "limit_id");
                    let limit_name = string_field(entry, "limit_name");
                    let pool_key = dynamic_pool_key(
                        limit_id.as_deref(),
                        limit_name.as_deref(),
                        "additional",
                        index,
                    );
                    let Some(rate_limit) = entry.get("rate_limit").and_then(Value::as_object)
                    else {
                        warnings.push(format!(
                            "additional_rate_limits[{index}].rate_limit was not an object"
                        ));
                        pools.push(QuotaPool {
                            pool_key,
                            kind: QuotaPoolKind::Additional,
                            limit_id,
                            limit_name,
                            windows: Vec::new(),
                            rate_limit_reached_type: string_field(entry, "rate_limit_reached_type"),
                            server_fields: extras(
                                entry,
                                &[
                                    "limit_id",
                                    "limit_name",
                                    "rate_limit",
                                    "rate_limit_reached_type",
                                ],
                            ),
                        });
                        continue;
                    };
                    let mut pool = pool_from_wham(
                        pool_key,
                        QuotaPoolKind::Additional,
                        limit_id,
                        limit_name,
                        rate_limit,
                        &mut warnings,
                    );
                    pool.rate_limit_reached_type = string_field(entry, "rate_limit_reached_type");
                    pool.server_fields = extras(
                        entry,
                        &[
                            "limit_id",
                            "limit_name",
                            "rate_limit",
                            "rate_limit_reached_type",
                        ],
                    );
                    pools.push(pool);
                }
            }
            None if !additional.is_null() => {
                warnings.push("additional_rate_limits was not an array".to_owned())
            }
            None => {}
        }
    }

    if let Some(code_review) = root.get("code_review_rate_limit") {
        if let Some(code_review) = code_review.as_object() {
            pools.push(pool_from_wham(
                "code_review".to_owned(),
                QuotaPoolKind::CodeReview,
                string_field(code_review, "limit_id"),
                string_field(code_review, "limit_name"),
                code_review,
                &mut warnings,
            ));
        } else if !code_review.is_null() {
            warnings.push("code_review_rate_limit was not an object".to_owned());
        }
    }

    let credits = root.get("credits").and_then(normalize_credits);
    Ok(QuotaSnapshot {
        source: QuotaSource::WhamUsage,
        plan_type: string_field(root, "plan_type"),
        pools,
        credits,
        individual_limit: root
            .get("individual_limit")
            .filter(|value| !value.is_null())
            .map(sanitize_value),
        spend_control_reached: bool_field(root, "spend_control_reached"),
        warnings,
        server_fields: extras(
            root,
            &[
                "plan_type",
                "limit_id",
                "limit_name",
                "rate_limit",
                "additional_rate_limits",
                "code_review_rate_limit",
                "credits",
                "individual_limit",
                "spend_control_reached",
                "rate_limit_reached_type",
            ],
        ),
    })
}

pub fn normalize_rate_limit_event(value: &Value) -> QuotaResult<QuotaSnapshot> {
    let root = value.as_object().ok_or(QuotaError::InvalidRoot)?;
    let mut warnings = Vec::new();
    let limit_id = string_field(root, "limit_id");
    let limit_name = string_field(root, "limit_name");
    let pool_key = dynamic_pool_key(limit_id.as_deref(), limit_name.as_deref(), "dynamic", 0);
    let mut windows = Vec::new();
    if let Some(primary) = root.get("primary") {
        if let Some(primary) = primary.as_object() {
            windows.push(window_from_event(
                primary,
                WindowRole::Primary,
                &mut warnings,
            ));
        } else if !primary.is_null() {
            warnings.push("primary was not an object".to_owned());
        }
    }
    if let Some(secondary) = root.get("secondary") {
        if let Some(secondary) = secondary.as_object() {
            windows.push(window_from_event(
                secondary,
                WindowRole::Secondary,
                &mut warnings,
            ));
        } else if !secondary.is_null() {
            warnings.push("secondary was not an object".to_owned());
        }
    }
    for (server_name, dynamic) in root {
        if matches!(server_name.as_str(), "primary" | "secondary") {
            continue;
        }
        let Some(dynamic) = dynamic.as_object() else {
            continue;
        };
        if looks_like_window(dynamic) {
            windows.push(window_from_event_named(
                dynamic,
                WindowRole::Dynamic,
                server_name,
                &mut warnings,
            ));
        }
    }

    let credits = root.get("credits").and_then(normalize_credits);
    Ok(QuotaSnapshot {
        source: QuotaSource::TokenCountEvent,
        plan_type: string_field(root, "plan_type"),
        pools: vec![QuotaPool {
            pool_key,
            kind: QuotaPoolKind::Dynamic,
            limit_id,
            limit_name,
            windows,
            rate_limit_reached_type: string_field(root, "rate_limit_reached_type"),
            server_fields: extras(
                root,
                &[
                    "limit_id",
                    "limit_name",
                    "primary",
                    "secondary",
                    "credits",
                    "individual_limit",
                    "spend_control_reached",
                    "plan_type",
                    "rate_limit_reached_type",
                ],
            ),
        }],
        credits,
        individual_limit: root
            .get("individual_limit")
            .filter(|value| !value.is_null())
            .map(sanitize_value),
        spend_control_reached: bool_field(root, "spend_control_reached"),
        warnings,
        server_fields: BTreeMap::new(),
    })
}

fn pool_from_wham(
    pool_key: String,
    kind: QuotaPoolKind,
    limit_id: Option<String>,
    limit_name: Option<String>,
    value: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> QuotaPool {
    let mut windows = Vec::new();
    if let Some(primary) = value.get("primary_window") {
        if let Some(primary) = primary.as_object() {
            windows.push(window_from_wham(primary, WindowRole::Primary, warnings));
        } else if !primary.is_null() {
            warnings.push(format!("{pool_key}.primary_window was not an object"));
        }
    }
    if let Some(secondary) = value.get("secondary_window") {
        if let Some(secondary) = secondary.as_object() {
            windows.push(window_from_wham(secondary, WindowRole::Secondary, warnings));
        } else if !secondary.is_null() {
            warnings.push(format!("{pool_key}.secondary_window was not an object"));
        }
    }
    for (server_name, dynamic) in value {
        if matches!(server_name.as_str(), "primary_window" | "secondary_window")
            || !server_name.ends_with("_window")
        {
            continue;
        }
        if let Some(dynamic) = dynamic.as_object() {
            windows.push(window_from_wham_named(
                dynamic,
                WindowRole::Dynamic,
                server_name,
                warnings,
            ));
        } else if !dynamic.is_null() {
            warnings.push(format!("{pool_key}.{server_name} was not an object"));
        }
    }
    QuotaPool {
        pool_key,
        kind,
        limit_id,
        limit_name,
        windows,
        rate_limit_reached_type: string_field(value, "rate_limit_reached_type"),
        server_fields: extras(
            value,
            &[
                "primary_window",
                "secondary_window",
                "rate_limit_reached_type",
                "limit_id",
                "limit_name",
            ],
        ),
    }
}

fn window_from_wham(
    value: &Map<String, Value>,
    role: WindowRole,
    warnings: &mut Vec<String>,
) -> QuotaWindow {
    let server_name = match role {
        WindowRole::Primary => "primary_window",
        WindowRole::Secondary => "secondary_window",
        WindowRole::Dynamic => "dynamic_window",
    };
    window_from_wham_named(value, role, server_name, warnings)
}

fn window_from_wham_named(
    value: &Map<String, Value>,
    role: WindowRole,
    server_name: &str,
    warnings: &mut Vec<String>,
) -> QuotaWindow {
    let used_percent = percent_field(value, "used_percent", warnings);
    let window_seconds = unsigned_field(value, "limit_window_seconds", warnings);
    let resets_at_unix = signed_field(value, "reset_at", warnings);
    QuotaWindow {
        role,
        server_name: server_name.to_owned(),
        used_percent,
        window_seconds,
        resets_at_unix,
        server_fields: extras(
            value,
            &[
                "used_percent",
                "limit_window_seconds",
                "reset_at",
                "resets_at",
            ],
        ),
    }
}

fn window_from_event(
    value: &Map<String, Value>,
    role: WindowRole,
    warnings: &mut Vec<String>,
) -> QuotaWindow {
    let server_name = match role {
        WindowRole::Primary => "primary",
        WindowRole::Secondary => "secondary",
        WindowRole::Dynamic => "dynamic",
    };
    window_from_event_named(value, role, server_name, warnings)
}

fn window_from_event_named(
    value: &Map<String, Value>,
    role: WindowRole,
    server_name: &str,
    warnings: &mut Vec<String>,
) -> QuotaWindow {
    let used_percent = percent_field(value, "used_percent", warnings);
    let window_seconds = match unsigned_field(value, "window_minutes", warnings) {
        Some(minutes) => match minutes.checked_mul(60) {
            Some(seconds) => Some(seconds),
            None => {
                warnings.push("window_minutes overflowed seconds".to_owned());
                None
            }
        },
        None => unsigned_field(value, "limit_window_seconds", warnings),
    };
    let resets_at_unix = signed_field(value, "resets_at", warnings)
        .or_else(|| signed_field(value, "reset_at", warnings));
    QuotaWindow {
        role,
        server_name: server_name.to_owned(),
        used_percent,
        window_seconds,
        resets_at_unix,
        server_fields: extras(
            value,
            &[
                "used_percent",
                "window_minutes",
                "limit_window_seconds",
                "resets_at",
                "reset_at",
            ],
        ),
    }
}

fn normalize_credits(value: &Value) -> Option<CreditPool> {
    let value = value.as_object()?;
    Some(CreditPool {
        has_credits: bool_field(value, "has_credits"),
        unlimited: bool_field(value, "unlimited"),
        balance: value.get("balance").and_then(decimal_string),
        server_fields: extras(value, &["has_credits", "unlimited", "balance"]),
    })
}

fn decimal_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn looks_like_window(value: &Map<String, Value>) -> bool {
    value.contains_key("used_percent")
        && (value.contains_key("window_minutes")
            || value.contains_key("limit_window_seconds")
            || value.contains_key("resets_at")
            || value.contains_key("reset_at"))
}

fn dynamic_pool_key(
    limit_id: Option<&str>,
    limit_name: Option<&str>,
    prefix: &str,
    index: usize,
) -> String {
    limit_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("{prefix}:id:{value}"))
        .or_else(|| {
            limit_name
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("{prefix}:name:{value}"))
        })
        .unwrap_or_else(|| format!("{prefix}:{index}"))
}

fn string_field(value: &Map<String, Value>, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn bool_field(value: &Map<String, Value>, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn percent_field(value: &Map<String, Value>, key: &str, warnings: &mut Vec<String>) -> Option<f64> {
    let raw = value.get(key)?;
    let Some(number) = raw.as_f64() else {
        if !raw.is_null() {
            warnings.push(format!("{key} was not numeric"));
        }
        return None;
    };
    if !number.is_finite() || !(0.0..=100.0).contains(&number) {
        warnings.push(format!("{key} was outside 0..=100"));
        return None;
    }
    Some(number)
}

fn unsigned_field(
    value: &Map<String, Value>,
    key: &str,
    warnings: &mut Vec<String>,
) -> Option<u64> {
    let raw = value.get(key)?;
    if raw.is_null() {
        return None;
    }
    let parsed = raw.as_u64();
    if parsed.is_none() {
        warnings.push(format!("{key} was not an unsigned integer"));
    }
    parsed
}

fn signed_field(value: &Map<String, Value>, key: &str, warnings: &mut Vec<String>) -> Option<i64> {
    let raw = value.get(key)?;
    if raw.is_null() {
        return None;
    }
    let parsed = raw.as_i64();
    if parsed.is_none() {
        warnings.push(format!("{key} was not a signed integer"));
    }
    parsed
}

fn extras(value: &Map<String, Value>, known: &[&str]) -> BTreeMap<String, Value> {
    value
        .iter()
        .filter(|(key, _)| !known.contains(&key.as_str()) && !is_sensitive_key(key))
        .map(|(key, value)| (key.clone(), sanitize_value(value)))
        .collect()
}

fn sanitize_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !is_sensitive_key(key))
                .map(|(key, value)| (key.clone(), sanitize_value(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sanitize_value).collect()),
        other => other.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "authorization"
        || key == "cookie"
        || key == "set-cookie"
        || key == "password"
        || key == "secret"
        || key.ends_with("_secret")
        || key == "api_key"
        || key.ends_with("_api_key")
        || key == "credential"
        || key.ends_with("_credential")
        || key == "email"
        || key.ends_with("_email")
        || key == "account_id"
        || key.ends_with("_account_id")
        || key == "user_id"
        || key.ends_with("_user_id")
        || key == "workspace_id"
        || key.ends_with("_workspace_id")
        || key == "organization_id"
        || key.ends_with("_organization_id")
        || key == "org_id"
        || key.ends_with("_org_id")
        || key == "token"
        || key.ends_with("_token")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn normalizes_wham_main_additional_code_review_and_credits() {
        let snapshot = normalize_usage_response(&json!({
            "plan_type": "pro",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 25.5,
                    "reset_at": 2_000_000_000_i64,
                    "limit_window_seconds": 18_000
                }
            },
            "additional_rate_limits": [{
                "limit_id": "codex_bengalfox",
                "limit_name": "GPT-5.3-Codex-Spark",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 10,
                        "reset_at": 2_000_000_100_i64,
                        "limit_window_seconds": 18_000
                    },
                    "secondary_window": {
                        "used_percent": 20,
                        "reset_at": 2_000_604_800_i64,
                        "limit_window_seconds": 604_800
                    }
                }
            }],
            "code_review_rate_limit": {
                "primary_window": { "used_percent": 5, "limit_window_seconds": 604_800 }
            },
            "credits": { "has_credits": false, "unlimited": false, "balance": "12.50" }
        }))
        .unwrap();

        assert_eq!(snapshot.source, QuotaSource::WhamUsage);
        assert_eq!(snapshot.pools.len(), 3);
        assert_eq!(
            snapshot.pools[1].limit_id.as_deref(),
            Some("codex_bengalfox")
        );
        assert_eq!(snapshot.pools[1].windows[1].window_seconds, Some(604_800));
        assert_eq!(snapshot.credits.unwrap().balance.as_deref(), Some("12.50"));
        assert!(snapshot.warnings.is_empty());
    }

    #[test]
    fn normalizes_token_count_rate_limit_without_model_inference() {
        let snapshot = normalize_quota_payload(&json!({
            "payload": {
                "type": "token_count",
                "rate_limits": {
                    "limit_id": "codex_bengalfox",
                    "limit_name": "GPT-5.3-Codex-Spark",
                    "primary": {
                        "used_percent": 45,
                        "window_minutes": 300,
                        "resets_at": 2_000_000_000_i64
                    },
                    "secondary": {
                        "used_percent": 50,
                        "window_minutes": 10_080,
                        "resets_at": 2_000_604_800_i64
                    },
                    "credits": { "has_credits": false, "balance": "0" },
                    "plan_type": "pro"
                }
            }
        }))
        .unwrap();
        assert_eq!(snapshot.source, QuotaSource::TokenCountEvent);
        assert_eq!(snapshot.pools[0].pool_key, "dynamic:id:codex_bengalfox");
        assert_eq!(snapshot.pools[0].windows[0].window_seconds, Some(18_000));
        assert_eq!(snapshot.pools[0].windows[1].window_seconds, Some(604_800));
        assert_eq!(snapshot.plan_type.as_deref(), Some("pro"));
    }

    #[test]
    fn invalid_percent_is_unknown_not_clamped_or_trusted() {
        let snapshot = normalize_rate_limit_event(&json!({
            "limit_id": "future-pool",
            "primary": { "used_percent": 125, "window_minutes": 5 }
        }))
        .unwrap();
        assert_eq!(snapshot.pools[0].windows[0].used_percent, None);
        assert_eq!(snapshot.pools[0].windows[0].window_seconds, Some(300));
        assert_eq!(snapshot.warnings.len(), 1);
    }

    #[test]
    fn false_credit_state_remains_distinct_from_missing_credit_data() {
        let explicit = normalize_rate_limit_event(&json!({
            "limit_id": "codex",
            "credits": { "has_credits": false }
        }))
        .unwrap();
        let missing = normalize_rate_limit_event(&json!({ "limit_id": "codex" })).unwrap();
        assert_eq!(explicit.credits.unwrap().has_credits, Some(false));
        assert_eq!(missing.credits, None);
    }

    #[test]
    fn normalized_unknown_fields_drop_credentials_and_raw_identity() {
        let snapshot = normalize_rate_limit_event(&json!({
            "limit_id": "codex",
            "access_token": "secret",
            "future": {
                "account_id": "raw-account",
                "safe_counter": 7
            },
            "individual_limit": {
                "user_id": "raw-user",
                "cap": 10
            }
        }))
        .unwrap();
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("raw-account"));
        assert!(!encoded.contains("raw-user"));
        assert!(encoded.contains("safe_counter"));
        assert!(encoded.contains("\"cap\":10"));
    }

    #[test]
    fn future_window_names_are_normalized_dynamically() {
        let event = normalize_rate_limit_event(&json!({
            "limit_id": "future",
            "primary": { "used_percent": 1, "window_minutes": 5 },
            "burst": { "used_percent": 2, "window_minutes": 1 }
        }))
        .unwrap();
        assert_eq!(event.pools[0].windows.len(), 2);
        assert_eq!(event.pools[0].windows[1].role, WindowRole::Dynamic);
        assert_eq!(event.pools[0].windows[1].server_name, "burst");

        let wham = normalize_usage_response(&json!({
            "rate_limit": {
                "primary_window": { "used_percent": 1, "limit_window_seconds": 300 },
                "burst_window": { "used_percent": 2, "limit_window_seconds": 60 }
            }
        }))
        .unwrap();
        assert_eq!(wham.pools[0].windows.len(), 2);
        assert_eq!(wham.pools[0].windows[1].server_name, "burst_window");
    }
}
