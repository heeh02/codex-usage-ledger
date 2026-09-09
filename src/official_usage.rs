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
    time::{Duration, Instant},
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
pub(crate) fn fetch_official_usage_at(
    home: &Path,
    thread_id: Option<&str>,
) -> Result<OfficialAccountUsage> {
    let binary = discover_codex_binary().context("locate a Codex app-server binary")?;
    fetch_with_binary(&binary, home, thread_id)
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

fn fetch_with_binary(
    binary: &Path,
    home: &Path,
    thread_id: Option<&str>,
) -> Result<OfficialAccountUsage> {
    let mut child = app_server_command(binary, home)
        .spawn()
        .with_context(|| format!("start {} app-server", binary.display()))?;

    let result = exchange_usage_request(&mut child, thread_id);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn app_server_command(binary: &Path, home: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .args(["app-server", "--listen", "stdio://"])
        .args(["-c", "cli_auth_credentials_store=\"file\""])
        .env("CODEX_HOME", home)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

fn exchange_usage_request(
    child: &mut Child,
    thread_id: Option<&str>,
) -> Result<OfficialAccountUsage> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
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
    wait_for_response(&receiver, 1, deadline)?;
    send_rpc(&mut stdin, &json!({"method": "initialized", "params": {}}))?;
    send_rpc(
        &mut stdin,
        &json!({"method":"account/read","id":2,"params":{"refreshToken":false}}),
    )?;
    let before = account_metadata(wait_for_response(&receiver, 2, deadline)?)?;
    let params = thread_id
        .map(|thread_id| json!({"threadId": thread_id}))
        .unwrap_or(Value::Null);
    send_rpc(
        &mut stdin,
        &json!({"method": "account/usage/read", "id": 3, "params": params}),
    )?;
    let response = wait_for_response(&receiver, 3, deadline)?;
    let result = response
        .result
        .context("account/usage/read returned no result")?;
    let usage: OfficialAccountUsage =
        serde_json::from_value(result).context("decode account/usage/read response")?;
    send_rpc(
        &mut stdin,
        &json!({"method":"account/read","id":4,"params":{"refreshToken":false}}),
    )?;
    let after = account_metadata(wait_for_response(&receiver, 4, deadline)?)?;
    if before != after {
        bail!("app-server account changed during usage read; result discarded");
    }
    if let Some(requested) = thread_id
        && usage
            .thread_usage
            .as_ref()
            .is_some_and(|value| value.thread_id != requested)
    {
        bail!("official thread response did not match requested thread");
    }
    if thread_id.is_none() && usage.thread_usage.is_some() {
        bail!("account usage request returned a thread-only response");
    }
    Ok(usage)
}

fn account_metadata(response: RpcEnvelope) -> Result<Value> {
    let account = response
        .result
        .and_then(|value| value.get("account").cloned())
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("chatgpt"))
        .ok_or_else(|| anyhow!("app-server has no ChatGPT account for usage read"))?;
    Ok(account)
}

fn send_rpc(stdin: &mut ChildStdin, request: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, request)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn wait_for_response(
    receiver: &Receiver<String>,
    expected_id: u64,
    deadline: Instant,
) -> Result<RpcEnvelope> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| anyhow!("Codex app-server exchange timed out"))?;
        let line = receiver
            .recv_timeout(remaining)
            .with_context(|| format!("wait for Codex app-server response {expected_id}"))?;
        let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(&line) else {
            continue;
        };
        if envelope.id != Some(expected_id) {
            continue;
        }
        if envelope.error.is_some() {
            bail!("Codex app-server request {expected_id} failed");
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
    fn child_command_uses_explicit_home_and_does_not_inherit_alternate_auth_modes() {
        let command =
            app_server_command(Path::new("synthetic-codex"), Path::new("/synthetic/home"));
        let args = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|p| p == ["-c", "cli_auth_credentials_store=\"file\""])
        );
        let env = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            env[std::ffi::OsStr::new("CODEX_HOME")],
            Some(std::ffi::OsStr::new("/synthetic/home"))
        );
        for key in ["CODEX_ACCESS_TOKEN", "CODEX_API_KEY", "OPENAI_API_KEY"] {
            assert_eq!(env[std::ffi::OsStr::new(key)], None);
        }
    }

    #[test]
    fn fixed_deadline_is_not_extended_by_notifications_and_errors_are_sanitized() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(r#"{"method":"account/updated"}"#.into())
            .unwrap();
        sender.send(r#"{"id":7,"result":{}}"#.into()).unwrap();
        assert!(wait_for_response(&receiver, 7, Instant::now()).is_err());
        assert!(wait_for_response(&receiver, 7, Instant::now() + Duration::from_secs(1)).is_ok());
        sender
            .send(r#"{"id":8,"error":{"message":"SYNTHETIC_PRIVATE_DETAIL"}}"#.into())
            .unwrap();
        let error =
            wait_for_response(&receiver, 8, Instant::now() + Duration::from_secs(1)).unwrap_err();
        assert!(!error.to_string().contains("SYNTHETIC_PRIVATE_DETAIL"));
        for account in [
            Value::Null,
            json!({"type":"apiKey"}),
            json!({"type":"amazonBedrock"}),
        ] {
            assert!(
                account_metadata(RpcEnvelope {
                    id: Some(2),
                    result: Some(json!({"account":account})),
                    error: None
                })
                .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn actual_stdio_exchange_brackets_usage_and_rejects_account_or_thread_changes() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("synthetic-app-server");
        let before = json!({"type":"chatgpt","email":"synthetic@example.com","planType":"pro"});
        for mode in ["same", "changed", "wrong-thread", "thread-in-account"] {
            let after = if mode == "changed" {
                json!({"type":"chatgpt","email":"other@example.com","planType":"pro"})
            } else {
                before.clone()
            };
            let usage = if mode == "wrong-thread" || mode == "thread-in-account" {
                json!({"summary":{},"dailyUsageBuckets":[],"threadUsage":{"threadId":"wrong","estimatedUsageCreditsMicros":0,"groups":[]}})
            } else {
                json!({"summary":{"lifetimeTokens":120},"dailyUsageBuckets":[{"startDate":"2026-01-01","tokens":120}]})
            };
            let script = format!(
                "#!/bin/sh\nwhile IFS= read -r request; do\n case \"$request\" in\n *'\"refreshToken\":true'*|*'account/login'*|*'account/logout'*) exit 9;;\n *'\"id\":1,'*) printf '%s\\n' '{{\"id\":1,\"result\":{{}}}}';;\n *'\"id\":2,'*) printf '%s\\n' '{}';;\n *'\"id\":3,'*) printf '%s\\n' '{}';;\n *'\"id\":4,'*) printf '%s\\n' '{}';;\n esac\ndone\n",
                json!({"id":2,"result":{"account":before}}),
                json!({"id":3,"result":usage}),
                json!({"id":4,"result":{"account":after}})
            );
            std::fs::write(&binary, script).unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
            let result = fetch_with_binary(
                &binary,
                directory.path(),
                (mode == "wrong-thread").then_some("requested"),
            );
            if mode == "same" {
                assert_eq!(result.unwrap().summary.lifetime_tokens, Some(120));
            } else {
                assert!(result.is_err(), "{mode} was accepted");
            }
        }
    }

    #[test]
    fn decodes_the_official_profile_shape() {
        let usage: OfficialAccountUsage = serde_json::from_value(json!({
            "summary": {
                "lifetimeTokens": 1_200_000_000_u64,
                "peakDailyTokens": 120_000_000_u64,
                "longestRunningTurnSec": 600,
                "currentStreakDays": 3,
                "longestStreakDays": 7
            },
            "dailyUsageBuckets": [
                {"startDate": "2026-01-01", "tokens": 100_000_000_u64}
            ],
            "threadUsage": null
        }))
        .unwrap();
        assert_eq!(usage.summary.lifetime_tokens, Some(1_200_000_000));
        assert_eq!(usage.daily_usage_buckets[0].tokens, 100_000_000);
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
