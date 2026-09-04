use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeZone, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use thiserror::Error;

use crate::types::AttributionConfidence;

const MAX_AUTH_FILE_BYTES: u64 = 1_048_576;
const MIN_HMAC_KEY_BYTES: usize = 16;
const OPENAI_AUTH_ISSUER: &str = "https://auth.openai.com";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("failed to inspect auth file {path}: {source}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read auth file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("auth path is a symbolic link: {0}")]
    Symlink(PathBuf),
    #[error("auth path is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    #[error("auth file is too large: {actual} bytes (maximum {maximum})")]
    FileTooLarge { actual: u64, maximum: u64 },
    #[cfg(unix)]
    #[error("auth file permissions are too broad: mode {mode:o}; expected no group/other access")]
    InsecurePermissions { mode: u32 },
    #[error("auth file changed while it was being read")]
    ChangedDuringRead,
    #[error("auth JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("auth JSON does not contain an id_token")]
    MissingIdToken,
    #[error("JWT is malformed")]
    MalformedJwt,
    #[error("JWT payload uses invalid base64url encoding")]
    InvalidJwtEncoding,
    #[error("JWT payload is not a JSON object")]
    InvalidJwtPayload,
    #[error("HMAC key must contain at least {MIN_HMAC_KEY_BYTES} bytes")]
    WeakHmacKey,
    #[error("failed to initialize HMAC")]
    HmacInitialization,
}

pub type IdentityResult<T> = Result<T, IdentityError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSource {
    ChatGptUserId,
    UserId,
    Subject,
    ChatGptAccountId,
    SelectedOrganizationId,
    DefaultOrganizationId,
    TokenAccountId,
    SingleOrganizationId,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthIdentity {
    /// HMAC of person and workspace scope. It is stable across token rotation.
    pub account_fingerprint: Option<String>,
    pub person_fingerprint: Option<String>,
    pub workspace_fingerprint: Option<String>,
    /// HMAC of the credential generation. It changes when active tokens rotate.
    pub auth_epoch: String,
    pub confidence: AttributionConfidence,
    pub person_claim_source: ClaimSource,
    pub workspace_claim_source: ClaimSource,
    /// False means JWT workspace metadata disagreed with the account header.
    pub workspace_claim_consistent: bool,
    pub issuer_fingerprint: Option<String>,
    pub plan_type: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
}

/// Reads a file-only Codex auth snapshot. This function never writes, refreshes
/// or returns credentials. JWT claims are decoded locally and remain unverified.
pub fn read_auth_identity(path: impl AsRef<Path>, hmac_key: &[u8]) -> IdentityResult<AuthIdentity> {
    validate_hmac_key(hmac_key)?;
    let path = path.as_ref();
    let before = fs::symlink_metadata(path).map_err(|source| IdentityError::Inspect {
        path: path.to_path_buf(),
        source,
    })?;
    if before.file_type().is_symlink() {
        return Err(IdentityError::Symlink(path.to_path_buf()));
    }
    if !before.is_file() {
        return Err(IdentityError::NotRegularFile(path.to_path_buf()));
    }
    if before.len() > MAX_AUTH_FILE_BYTES {
        return Err(IdentityError::FileTooLarge {
            actual: before.len(),
            maximum: MAX_AUTH_FILE_BYTES,
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mode = before.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(IdentityError::InsecurePermissions { mode });
        }
    }

    let bytes = fs::read(path).map_err(|source| IdentityError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let after = fs::symlink_metadata(path).map_err(|source| IdentityError::Inspect {
        path: path.to_path_buf(),
        source,
    })?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err(IdentityError::ChangedDuringRead);
    }
    parse_auth_identity(&bytes, hmac_key)
}

pub fn parse_auth_identity(bytes: &[u8], hmac_key: &[u8]) -> IdentityResult<AuthIdentity> {
    validate_hmac_key(hmac_key)?;
    if bytes.len() as u64 > MAX_AUTH_FILE_BYTES {
        return Err(IdentityError::FileTooLarge {
            actual: bytes.len() as u64,
            maximum: MAX_AUTH_FILE_BYTES,
        });
    }
    let auth: Value = serde_json::from_slice(bytes)?;
    let id_token = auth
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(IdentityError::MissingIdToken)?;
    let claims = decode_jwt_payload(id_token)?;
    let auth_claims = claims
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object);

    let issuer = claims
        .get("iss")
        .and_then(Value::as_str)
        .unwrap_or("unknown-issuer");
    let (person, person_claim_source) = first_nonempty([
        (
            auth_claims
                .and_then(|value| value.get("chatgpt_user_id"))
                .and_then(Value::as_str),
            ClaimSource::ChatGptUserId,
        ),
        (
            auth_claims
                .and_then(|value| value.get("user_id"))
                .and_then(Value::as_str),
            ClaimSource::UserId,
        ),
        (
            claims.get("sub").and_then(Value::as_str),
            ClaimSource::Subject,
        ),
    ]);

    let token_account_id = auth
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let chatgpt_account_id = auth_claims
        .and_then(|value| value.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let workspace_claim_consistent = token_account_id
        .zip(chatgpt_account_id)
        .is_none_or(|(header, claim)| header == claim);

    // The account header is the actual quota request scope. JWT organization
    // claims are compatibility fallbacks for older token shapes.
    let (workspace, workspace_claim_source) = first_nonempty([
        (token_account_id, ClaimSource::TokenAccountId),
        (chatgpt_account_id, ClaimSource::ChatGptAccountId),
        (
            auth_claims
                .and_then(|value| value.get("selected_organization_id"))
                .and_then(Value::as_str),
            ClaimSource::SelectedOrganizationId,
        ),
        (
            auth_claims
                .and_then(|value| value.get("default_organization_id"))
                .and_then(Value::as_str),
            ClaimSource::DefaultOrganizationId,
        ),
        (
            single_organization_id(auth_claims),
            ClaimSource::SingleOrganizationId,
        ),
    ]);

    let person_fingerprint = person
        .map(|value| fingerprint(hmac_key, "codex/person/v1", &[issuer, value]))
        .transpose()?;
    let workspace_fingerprint = workspace
        .map(|value| fingerprint(hmac_key, "codex/workspace/v1", &[issuer, value]))
        .transpose()?;
    let account_fingerprint = match (&person_fingerprint, &workspace_fingerprint) {
        (Some(person), Some(workspace)) => Some(fingerprint(
            hmac_key,
            "codex/account-scope/v1",
            &[person, workspace],
        )?),
        (Some(person), None) => Some(fingerprint(
            hmac_key,
            "codex/account-scope/person-only/v1",
            &[person],
        )?),
        (None, Some(workspace)) => Some(fingerprint(
            hmac_key,
            "codex/account-scope/workspace-only/v1",
            &[workspace],
        )?),
        (None, None) => None,
    };

    let credential_material = [
        auth.pointer("/tokens/access_token").and_then(Value::as_str),
        auth.pointer("/tokens/refresh_token")
            .and_then(Value::as_str),
        Some(id_token),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    let auth_epoch = fingerprint(hmac_key, "codex/auth-epoch/v1", &credential_material)?;

    let confidence = match (person, workspace) {
        (Some(_), Some(_))
            if claims.get("iss").and_then(Value::as_str).is_some()
                && workspace_claim_consistent =>
        {
            AttributionConfidence::Verified
        }
        (Some(_), None) | (None, Some(_)) => AttributionConfidence::Inferred,
        (Some(_), Some(_)) => AttributionConfidence::Inferred,
        (None, None) => AttributionConfidence::Unknown,
    };
    let issuer_fingerprint = claims
        .get("iss")
        .and_then(Value::as_str)
        .map(|value| fingerprint(hmac_key, "codex/issuer/v1", &[value]))
        .transpose()?;
    let plan_type = auth_claims
        .and_then(|value| value.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let access_token_expires_at = auth
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .and_then(|token| decode_jwt_payload(token).ok())
        .and_then(|payload| payload.get("exp").and_then(Value::as_i64))
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single());

    Ok(AuthIdentity {
        account_fingerprint,
        person_fingerprint,
        workspace_fingerprint,
        auth_epoch,
        confidence,
        person_claim_source,
        workspace_claim_source,
        workspace_claim_consistent,
        issuer_fingerprint,
        plan_type,
        access_token_expires_at,
    })
}

fn validate_hmac_key(key: &[u8]) -> IdentityResult<()> {
    if key.len() < MIN_HMAC_KEY_BYTES {
        return Err(IdentityError::WeakHmacKey);
    }
    Ok(())
}

/// Converts a raw workspace/account header observed in Codex's own local
/// authentication log into the same pseudonymous workspace key used by
/// `read_auth_identity`. The raw identifier must never be persisted.
pub fn fingerprint_logged_workspace_id(
    raw_workspace_id: &str,
    hmac_key: &[u8],
) -> IdentityResult<String> {
    validate_hmac_key(hmac_key)?;
    fingerprint(
        hmac_key,
        "codex/workspace/v1",
        &[OPENAI_AUTH_ISSUER, raw_workspace_id],
    )
}

/// Stable placeholder used until that workspace is observed while signed in
/// and can be linked to the stronger person+workspace account fingerprint.
pub fn provisional_account_fingerprint(
    workspace_fingerprint: &str,
    hmac_key: &[u8],
) -> IdentityResult<String> {
    validate_hmac_key(hmac_key)?;
    fingerprint(
        hmac_key,
        "codex/historical-account-provisional/v1",
        &[workspace_fingerprint],
    )
}

fn decode_jwt_payload(token: &str) -> IdentityResult<Value> {
    let mut parts = token.split('.');
    let _header = parts.next().ok_or(IdentityError::MalformedJwt)?;
    let payload = parts.next().ok_or(IdentityError::MalformedJwt)?;
    let _signature = parts.next().ok_or(IdentityError::MalformedJwt)?;
    if parts.next().is_some() || payload.is_empty() {
        return Err(IdentityError::MalformedJwt);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .map_err(|_| IdentityError::InvalidJwtEncoding)?;
    if bytes.len() as u64 > MAX_AUTH_FILE_BYTES {
        return Err(IdentityError::FileTooLarge {
            actual: bytes.len() as u64,
            maximum: MAX_AUTH_FILE_BYTES,
        });
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if !value.is_object() {
        return Err(IdentityError::InvalidJwtPayload);
    }
    Ok(value)
}

fn first_nonempty<const N: usize>(
    values: [(Option<&str>, ClaimSource); N],
) -> (Option<&str>, ClaimSource) {
    values
        .into_iter()
        .find_map(|(value, source)| {
            value
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| (Some(value), source))
        })
        .unwrap_or((None, ClaimSource::Missing))
}

fn single_organization_id(auth_claims: Option<&serde_json::Map<String, Value>>) -> Option<&str> {
    let organizations = auth_claims?.get("organizations")?.as_array()?;
    if organizations.len() != 1 {
        return None;
    }
    organizations[0].get("id")?.as_str()
}

fn fingerprint(key: &[u8], domain: &str, values: &[&str]) -> IdentityResult<String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| IdentityError::HmacInitialization)?;
    update_length_prefixed(&mut mac, domain.as_bytes());
    for value in values {
        update_length_prefixed(&mut mac, value.as_bytes());
    }
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn update_length_prefixed(mac: &mut HmacSha256, value: &[u8]) {
    mac.update(&(value.len() as u64).to_be_bytes());
    mac.update(value);
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::json;
    use tempfile::NamedTempFile;

    use super::*;

    const KEY: &[u8] = b"0123456789abcdef0123456789abcdef";

    fn jwt(payload: Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{payload}.")
    }

    fn auth(user: &str, workspace: &str, plan: &str, access_generation: &str) -> Vec<u8> {
        let id_token = jwt(json!({
            "iss": "https://auth.openai.com",
            "sub": format!("sub-{user}"),
            "email": format!("{user}@example.com"),
            "https://api.openai.com/auth": {
                "chatgpt_user_id": user,
                "chatgpt_account_id": workspace,
                "chatgpt_plan_type": plan
            }
        }));
        let access_token =
            jwt(json!({ "exp": 2_000_000_000_i64, "generation": access_generation }));
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": id_token,
                "access_token": access_token,
                "refresh_token": format!("refresh-{access_generation}"),
                "account_id": workspace
            }
        }))
        .unwrap()
    }

    #[test]
    fn plan_changes_do_not_change_stable_account_fingerprint() {
        let plus = parse_auth_identity(&auth("user-1", "workspace-1", "plus", "a"), KEY).unwrap();
        let pro = parse_auth_identity(&auth("user-1", "workspace-1", "pro", "b"), KEY).unwrap();
        assert_eq!(plus.account_fingerprint, pro.account_fingerprint);
        assert_eq!(plus.workspace_fingerprint, pro.workspace_fingerprint);
        assert_ne!(plus.auth_epoch, pro.auth_epoch);
        assert_eq!(plus.confidence, AttributionConfidence::Verified);
    }

    #[test]
    fn different_workspace_changes_account_scope_but_not_person() {
        let first = parse_auth_identity(&auth("user-1", "workspace-1", "pro", "a"), KEY).unwrap();
        let second = parse_auth_identity(&auth("user-1", "workspace-2", "pro", "a"), KEY).unwrap();
        assert_eq!(first.person_fingerprint, second.person_fingerprint);
        assert_ne!(first.workspace_fingerprint, second.workspace_fingerprint);
        assert_ne!(first.account_fingerprint, second.account_fingerprint);
    }

    #[test]
    fn account_header_wins_and_mismatch_lowers_confidence() {
        let id_token = jwt(json!({
            "iss": "https://auth.openai.com",
            "sub": "user",
            "https://api.openai.com/auth": {
                "chatgpt_user_id": "user",
                "chatgpt_account_id": "jwt-workspace"
            }
        }));
        let bytes = serde_json::to_vec(&json!({
            "tokens": {
                "id_token": id_token,
                "access_token": "opaque",
                "account_id": "header-workspace"
            }
        }))
        .unwrap();
        let identity = parse_auth_identity(&bytes, KEY).unwrap();
        assert_eq!(identity.workspace_claim_source, ClaimSource::TokenAccountId);
        assert!(!identity.workspace_claim_consistent);
        assert_eq!(identity.confidence, AttributionConfidence::Inferred);
    }

    #[test]
    fn email_is_never_used_as_identity_fallback() {
        let id_token = jwt(json!({
            "iss": "https://auth.openai.com",
            "email": "same@example.com",
            "https://api.openai.com/auth": { "chatgpt_plan_type": "pro" }
        }));
        let bytes = serde_json::to_vec(&json!({
            "tokens": { "id_token": id_token, "access_token": "opaque" }
        }))
        .unwrap();
        let identity = parse_auth_identity(&bytes, KEY).unwrap();
        assert_eq!(identity.account_fingerprint, None);
        assert_eq!(identity.confidence, AttributionConfidence::Unknown);
    }

    #[test]
    fn file_reader_is_read_only_and_accepts_private_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&auth("user", "workspace", "pro", "a"))
            .unwrap();
        let before = fs::read(file.path()).unwrap();
        let identity = read_auth_identity(file.path(), KEY).unwrap();
        let after = fs::read(file.path()).unwrap();
        assert!(identity.account_fingerprint.is_some());
        assert_eq!(before, after);
    }

    #[test]
    fn weak_hmac_keys_are_rejected() {
        assert!(matches!(
            parse_auth_identity(&auth("user", "workspace", "pro", "a"), b"short"),
            Err(IdentityError::WeakHmacKey)
        ));
    }
}
