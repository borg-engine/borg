use std::fs;

use borg_core::config::{codex_has_credentials, read_oauth_from_credentials};
use tempfile::TempDir;

fn write(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_string()
}

// ── codex_has_credentials ─────────────────────────────────────────────────────

#[test]
fn codex_missing_file_returns_false() {
    assert!(!codex_has_credentials("/nonexistent/path/auth.json"));
}

#[test]
fn codex_invalid_json_returns_false() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "auth.json", "not json at all");
    assert!(!codex_has_credentials(&path));
}

#[test]
fn codex_valid_access_token_returns_true() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "auth.json", r#"{"tokens":{"access_token":"abc123"}}"#);
    assert!(codex_has_credentials(&path));
}

#[test]
fn codex_empty_access_token_returns_false() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "auth.json", r#"{"tokens":{"access_token":""}}"#);
    assert!(!codex_has_credentials(&path));
}

#[test]
fn codex_missing_tokens_key_returns_false() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "auth.json", r#"{"other":"value"}"#);
    assert!(!codex_has_credentials(&path));
}

#[test]
fn codex_missing_access_token_key_returns_false() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "auth.json", r#"{"tokens":{"refresh_token":"xyz"}}"#);
    assert!(!codex_has_credentials(&path));
}

// ── read_oauth_from_credentials ───────────────────────────────────────────────

#[test]
fn oauth_missing_file_returns_none() {
    assert!(read_oauth_from_credentials("/nonexistent/credentials.json").is_none());
}

#[test]
fn oauth_invalid_json_returns_none() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "creds.json", "{bad json}");
    assert!(read_oauth_from_credentials(&path).is_none());
}

#[test]
fn oauth_prefers_claude_ai_oauth_access_token() {
    let dir = TempDir::new().unwrap();
    let path = write(
        &dir,
        "creds.json",
        r#"{"claudeAiOauth":{"accessToken":"claude-tok"},"oauthToken":"fallback-tok"}"#,
    );
    assert_eq!(
        read_oauth_from_credentials(&path),
        Some("claude-tok".to_string())
    );
}

#[test]
fn oauth_falls_back_to_oauth_token_when_no_claude_ai_oauth() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "creds.json", r#"{"oauthToken":"fallback-tok"}"#);
    assert_eq!(
        read_oauth_from_credentials(&path),
        Some("fallback-tok".to_string())
    );
}

#[test]
fn oauth_returns_none_when_neither_key_present() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "creds.json", r#"{"someOtherField":"value"}"#);
    assert!(read_oauth_from_credentials(&path).is_none());
}

#[test]
fn oauth_empty_string_access_token_returns_empty_string() {
    // read_oauth_from_credentials returns the string as-is; callers filter emptiness
    let dir = TempDir::new().unwrap();
    let path = write(
        &dir,
        "creds.json",
        r#"{"claudeAiOauth":{"accessToken":""}}"#,
    );
    assert_eq!(
        read_oauth_from_credentials(&path),
        Some(String::new())
    );
}
