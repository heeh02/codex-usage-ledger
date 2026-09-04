//! Read-only access to Codex's account token-activity ledger.
//!
//! The Codex app-server owns authentication and talks to the same backend that
//! powers the Codex profile usage chart. This module never reads, stores, or
//! refreshes OAuth credentials.

use std::{
    env,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsageSummary {
    pub lifetime_tokens: Option<u64>,
    pub peak_daily_tokens: Option<u64>,
    pub longest_running_turn_sec: Option<u64>,
    pub current_streak_days: Option<u64>,
    pub longest_streak_days: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialDailyUsageBucket {
    pub start_date: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialAccountUsage {
    pub summary: OfficialUsageSummary,
    #[serde(default, deserialize_with = "null_default")]
    pub daily_usage_buckets: Vec<OfficialDailyUsageBucket>,
    #[serde(default)]
    pub thread_usage: Option<OfficialThreadUsage>,
}

fn null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialThreadUsage {
    pub thread_id: String,
    pub estimated_usage_credits_micros: u64,
    pub estimated_usage_usd_micros: Option<u64>,
    #[serde(default)]
    pub groups: Vec<OfficialThreadUsageGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialThreadUsageGroup {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub speed: Option<String>,
    pub estimated_usage_credits_micros: u64,
    pub net_new_input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    #[serde(default, alias = "cacheWriteTokens")]
    pub cache_write_input_tokens: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<Value>,
}

/// Fetches the signed-in account's official token activity through Codex's
/// stable `account/usage/read` app-server method.
pub fn fetch_official_account_usage() -> Result<OfficialAccountUsage> {
    let binary = discover_codex_binary().context("locate a Codex app-server binary")?;
    fetch_with_binary(&binary, None)
}

pub fn fetch_official_thread_usage(thread_id: &str) -> Result<Option<OfficialThreadUsage>> {
    let binary = discover_codex_binary().context("locate a Codex app-server binary")?;
    Ok(fetch_with_binary(&binary, Some(thread_id))?.thread_usage)
}

pub fn discover_codex_binary() -> Option<PathBuf> {
    for variable in ["CODEX_USAGE_LEDGER_CODEX_BIN", "CODEX_CLI_PATH"] {
        if let Some(path) = env::var_os(variable).map(PathBuf::from)
            && is_executable_file(&path)
        {
            return Some(path);
        }
    }

    for candidate in [
        "/Applications/ChatGPT.app/Contents/Resources/codex",
        "/Applications/Codex.app/Contents/Resources/codex",
    ] {
        let path = PathBuf::from(candidate);
        if is_executable_file(&path) {
            return Some(path);
        }
    }

    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join("codex"))
            .find(|candidate| is_executable_file(candidate))
    })
}

fn fetch_with_binary(binary: &Path, thread_id: Option<&str>) -> Result<OfficialAccountUsage> {
    let mut child = Command::new(binary)
        .args(["app-server", "--listen", "stdio://"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("start {} app-server", binary.display()))?;

    let result = exchange_usage_request(&mut child, thread_id);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn exchange_usage_request(
    child: &mut Child,
    thread_id: Option<&str>,
) -> Result<OfficialAccountUsage> {
    let mut stdin = child
        .stdin
        .take()
        .context("Codex app-server stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("Codex app-server stdout unavailable")?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    send_rpc(
        &mut stdin,
        &json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "codex_usage_ledger",
                    "title": "Codex Usage Ledger",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
    )?;
    wait_for_response(&receiver, 1)?;
    send_rpc(&mut stdin, &json!({"method": "initialized", "params": {}}))?;
    let params = thread_id
        .map(|thread_id| json!({"threadId": thread_id}))
        .unwrap_or(Value::Null);
    send_rpc(
        &mut stdin,
        &json!({"method": "account/usage/read", "id": 2, "params": params}),
    )?;
    let response = wait_for_response(&receiver, 2)?;
    let result = response
        .result
        .context("account/usage/read returned no result")?;
    serde_json::from_value(result).context("decode account/usage/read response")
}

fn send_rpc(stdin: &mut ChildStdin, request: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, request)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn wait_for_response(receiver: &Receiver<String>, expected_id: u64) -> Result<RpcEnvelope> {
    loop {
        let line = receiver
            .recv_timeout(RESPONSE_TIMEOUT)
            .with_context(|| format!("wait for Codex app-server response {expected_id}"))?;
        let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(&line) else {
            continue;
        };
        if envelope.id != Some(expected_id) {
            continue;
        }
        if let Some(error) = &envelope.error {
            bail!("Codex app-server request {expected_id} failed: {error}");
        }
        if envelope.result.is_none() {
            return Err(anyhow!("Codex app-server response {expected_id} was empty"));
        }
        return Ok(envelope);
    }
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_official_profile_shape() {
        let usage: OfficialAccountUsage = serde_json::from_value(json!({
            "summary": {
                "lifetimeTokens": 61_052_184_141_u64,
                "peakDailyTokens": 4_124_570_551_u64,
                "longestRunningTurnSec": 53_003,
                "currentStreakDays": 11,
                "longestStreakDays": 28
            },
            "dailyUsageBuckets": [
                {"startDate": "2026-08-28", "tokens": 3_773_478_465_u64}
            ],
            "threadUsage": null
        }))
        .unwrap();
        assert_eq!(usage.summary.lifetime_tokens, Some(61_052_184_141));
        assert_eq!(usage.daily_usage_buckets[0].tokens, 3_773_478_465);
    }

    #[test]
    fn decodes_thread_mode_with_null_daily_buckets() {
        let usage: OfficialAccountUsage = serde_json::from_value(json!({
            "summary": {},
            "dailyUsageBuckets": null,
            "threadUsage": {
                "threadId": "019fc8ab-1fb2-7000-8000-000000000123",
                "estimatedUsageCreditsMicros": 46_000_000,
                "estimatedUsageUsdMicros": null,
                "groups": [{
                    "model": "gpt-5.4",
                    "reasoningEffort": "high",
                    "speed": "fast",
                    "estimatedUsageCreditsMicros": 46_000_000,
                    "netNewInputTokens": 80,
                    "cachedInputTokens": 20,
                    "inputTokens": 100,
                    "outputTokens": 40,
                    "totalTokens": 140
                }]
            }
        }))
        .unwrap();
        assert!(usage.daily_usage_buckets.is_empty());
        let thread = usage.thread_usage.unwrap();
        assert_eq!(thread.estimated_usage_credits_micros, 46_000_000);
        assert_eq!(thread.groups[0].total_tokens, Some(140));
    }
}
