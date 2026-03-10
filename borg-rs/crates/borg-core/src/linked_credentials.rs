use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::config::read_oauth_from_credentials;

pub const PROVIDER_CLAUDE: &str = "claude";
pub const PROVIDER_OPENAI: &str = "openai";
pub const LINKED_AUTH_REFRESH_WINDOW_MINS: i64 = 15;

const CLAUDE_CREDENTIALS_REL_PATH: &str = ".claude/.credentials.json";
const CODEX_AUTH_REL_PATH: &str = ".codex/auth.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedCredentialFile {
    pub path: String,
    pub contents_b64: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedCredentialBundle {
    #[serde(default)]
    pub files: Vec<LinkedCredentialFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedCredentialValidation {
    pub ok: bool,
    pub auth_kind: String,
    pub account_email: String,
    pub account_label: String,
    pub expires_at: String,
    pub last_error: String,
}

fn required_files_for_provider(provider: &str) -> Result<&'static [&'static str]> {
    match provider {
        PROVIDER_CLAUDE => Ok(&[CLAUDE_CREDENTIALS_REL_PATH]),
        PROVIDER_OPENAI => Ok(&[CODEX_AUTH_REL_PATH]),
        other => Err(anyhow!("unsupported linked credential provider: {other}")),
    }
}

fn auth_kind_for_provider(provider: &str, auth_json: Option<&serde_json::Value>) -> String {
    match provider {
        PROVIDER_CLAUDE => "claude_code_session".to_string(),
        PROVIDER_OPENAI => {
            let has_tokens = auth_json
                .and_then(|v| v.get("tokens"))
                .and_then(|v| v.as_object())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if has_tokens {
                "codex_chatgpt_session".to_string()
            } else {
                "openai_api_key".to_string()
            }
        },
        _ => String::new(),
    }
}

fn encode_file_contents(contents: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(contents)
}

fn decode_file_contents(contents_b64: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(contents_b64)
        .context("decode linked credential bundle file")
}

pub fn capture_bundle(provider: &str, home_dir: &Path) -> Result<LinkedCredentialBundle> {
    let files = required_files_for_provider(provider)?
        .iter()
        .map(|rel_path| {
            let path = home_dir.join(rel_path);
            let contents = fs::read(&path)
                .with_context(|| format!("read linked credential file {}", path.display()))?;
            Ok(LinkedCredentialFile {
                path: (*rel_path).to_string(),
                contents_b64: encode_file_contents(&contents),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LinkedCredentialBundle { files })
}

pub fn restore_bundle(bundle: &LinkedCredentialBundle, home_dir: &Path) -> Result<()> {
    for file in &bundle.files {
        let path = home_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create linked credential dir {}", parent.display()))?;
        }
        let contents = decode_file_contents(&file.contents_b64)?;
        fs::write(&path, contents)
            .with_context(|| format!("write linked credential file {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}

pub fn bundle_contains_required_files(provider: &str, bundle: &LinkedCredentialBundle) -> bool {
    required_files_for_provider(provider)
        .ok()
        .map(|required| {
            required
                .iter()
                .all(|path| bundle.files.iter().any(|file| file.path == *path))
        })
        .unwrap_or(false)
}

pub fn claude_credentials_path(home_dir: &Path) -> PathBuf {
    home_dir.join(CLAUDE_CREDENTIALS_REL_PATH)
}

pub fn codex_credentials_path(home_dir: &Path) -> PathBuf {
    home_dir.join(CODEX_AUTH_REL_PATH)
}

pub fn claude_oauth_token_from_home(home_dir: &Path) -> Option<String> {
    let path = claude_credentials_path(home_dir);
    read_oauth_from_credentials(path.to_str()?)
}

pub fn claude_expiry_from_home(home_dir: &Path) -> Option<DateTime<Utc>> {
    let path = claude_credentials_path(home_dir);
    let contents = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let expires_at_ms = value
        .get("claudeAiOauth")
        .and_then(|v| v.get("expiresAt"))
        .and_then(|v| v.as_i64())?;
    Utc.timestamp_millis_opt(expires_at_ms).single()
}

fn read_openai_auth_json(home_dir: &Path) -> Option<serde_json::Value> {
    let path = codex_credentials_path(home_dir);
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn openai_identity_from_home(home_dir: &Path) -> (String, String, String) {
    let Some(auth_json) = read_openai_auth_json(home_dir) else {
        return (String::new(), String::new(), String::new());
    };
    let email = auth_json
        .get("tokens")
        .and_then(|v| v.get("id_token"))
        .and_then(|v| v.as_str())
        .and_then(decode_jwt_payload)
        .and_then(|v| {
            v.get("email")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let expires_at = auth_json
        .get("tokens")
        .and_then(|v| v.get("id_token"))
        .and_then(|v| v.as_str())
        .and_then(decode_jwt_payload)
        .and_then(|v| v.get("exp").and_then(|value| value.as_i64()))
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_default();
    (email.clone(), email, expires_at)
}

pub async fn validate_home(provider: &str, home_dir: &Path) -> Result<LinkedCredentialValidation> {
    match provider {
        PROVIDER_CLAUDE => validate_claude_home(home_dir).await,
        PROVIDER_OPENAI => validate_openai_home(home_dir).await,
        other => Err(anyhow!("unsupported linked credential provider: {other}")),
    }
}

async fn validate_claude_home(home_dir: &Path) -> Result<LinkedCredentialValidation> {
    let mut cmd = Command::new("claude");
    cmd.args(["auth", "status", "--json"])
        .env("HOME", home_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().await.context("run claude auth status")?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();
    let ok = output.status.success()
        && value
            .get("loggedIn")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let expires_at = claude_expiry_from_home(home_dir)
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_default();
    Ok(LinkedCredentialValidation {
        ok,
        auth_kind: auth_kind_for_provider(PROVIDER_CLAUDE, Some(&value)),
        account_email: value
            .get("email")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        account_label: value
            .get("orgName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        expires_at,
        last_error: if ok {
            String::new()
        } else if !stderr.is_empty() {
            stderr
        } else {
            stdout
        },
    })
}

async fn validate_openai_home(home_dir: &Path) -> Result<LinkedCredentialValidation> {
    let codex_home = home_dir.join(".codex");
    let mut cmd = Command::new("codex");
    cmd.args(["login", "status"])
        .env("HOME", home_dir)
        .env("CODEX_HOME", &codex_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().await.context("run codex login status")?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let ok = output.status.success() && stdout.to_ascii_lowercase().contains("logged in");
    let (account_email, account_label, expires_at) = openai_identity_from_home(home_dir);
    let auth_json = read_openai_auth_json(home_dir);
    Ok(LinkedCredentialValidation {
        ok,
        auth_kind: auth_kind_for_provider(PROVIDER_OPENAI, auth_json.as_ref()),
        account_email,
        account_label,
        expires_at,
        last_error: if ok {
            String::new()
        } else if !stderr.is_empty() {
            stderr
        } else {
            stdout
        },
    })
}

pub fn should_revalidate(last_validated_at: &str, expires_at: &str) -> bool {
    let now = Utc::now();
    if last_validated_at.trim().is_empty() {
        return true;
    }
    if let Ok(ts) = DateTime::parse_from_rfc3339(last_validated_at) {
        if now
            .signed_duration_since(ts.with_timezone(&Utc))
            .num_minutes()
            >= LINKED_AUTH_REFRESH_WINDOW_MINS
        {
            return true;
        }
    } else {
        return true;
    }
    if let Ok(expiry) = DateTime::parse_from_rfc3339(expires_at) {
        return expiry.with_timezone(&Utc)
            <= now + chrono::Duration::minutes(LINKED_AUTH_REFRESH_WINDOW_MINS);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_requires_expected_paths() {
        let bundle = LinkedCredentialBundle {
            files: vec![LinkedCredentialFile {
                path: ".claude/.credentials.json".to_string(),
                contents_b64: encode_file_contents(br#"{"claudeAiOauth":{"accessToken":"x"}}"#),
            }],
        };
        assert!(bundle_contains_required_files(PROVIDER_CLAUDE, &bundle));
        assert!(!bundle_contains_required_files(PROVIDER_OPENAI, &bundle));
    }

    #[test]
    fn refresh_window_revalidates_old_or_expiring_credentials() {
        let old = (Utc::now() - chrono::Duration::minutes(16)).to_rfc3339();
        assert!(should_revalidate(&old, ""));

        let recent = Utc::now().to_rfc3339();
        let expiring = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
        assert!(should_revalidate(&recent, &expiring));

        let healthy = (Utc::now() + chrono::Duration::minutes(45)).to_rfc3339();
        assert!(!should_revalidate(&recent, &healthy));
    }

    #[test]
    fn decodes_openai_identity_from_jwt_payload() {
        let claims = serde_json::json!({
            "email": "user@example.com",
            "exp": 1_900_000_000i64,
        });
        let claims_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).unwrap());
        let token = format!("x.{claims_b64}.y");
        let payload = decode_jwt_payload(&token).unwrap();
        assert_eq!(payload["email"].as_str(), Some("user@example.com"));
    }
}
