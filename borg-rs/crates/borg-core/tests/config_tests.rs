use borg_core::config::{codex_has_credentials, read_oauth_from_credentials};
use std::io::Write;
use tempfile::NamedTempFile;

fn write_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

// codex_has_credentials

#[test]
fn codex_has_credentials_returns_true_for_valid_token() {
    let f = write_temp(r#"{"tokens":{"access_token":"tok123"}}"#);
    assert!(codex_has_credentials(f.path().to_str().unwrap()));
}

#[test]
fn codex_has_credentials_returns_false_for_empty_token() {
    let f = write_temp(r#"{"tokens":{"access_token":""}}"#);
    assert!(!codex_has_credentials(f.path().to_str().unwrap()));
}

#[test]
fn codex_has_credentials_returns_false_for_missing_access_token_key() {
    let f = write_temp(r#"{"tokens":{}}"#);
    assert!(!codex_has_credentials(f.path().to_str().unwrap()));
}

#[test]
fn codex_has_credentials_returns_false_for_missing_tokens_key() {
    let f = write_temp(r#"{"other":"value"}"#);
    assert!(!codex_has_credentials(f.path().to_str().unwrap()));
}

#[test]
fn codex_has_credentials_returns_false_for_missing_file() {
    assert!(!codex_has_credentials("/tmp/borg-test-nonexistent-file-xyz.json"));
}

#[test]
fn codex_has_credentials_returns_false_for_malformed_json() {
    let f = write_temp("not valid json {{{");
    assert!(!codex_has_credentials(f.path().to_str().unwrap()));
}

// read_oauth_from_credentials

#[test]
fn read_oauth_returns_claude_ai_oauth_access_token() {
    let f = write_temp(r#"{"claudeAiOauth":{"accessToken":"oauth-tok"}}"#);
    assert_eq!(
        read_oauth_from_credentials(f.path().to_str().unwrap()),
        Some("oauth-tok".to_string())
    );
}

#[test]
fn read_oauth_falls_back_to_root_oauth_token() {
    let f = write_temp(r#"{"oauthToken":"root-tok"}"#);
    assert_eq!(
        read_oauth_from_credentials(f.path().to_str().unwrap()),
        Some("root-tok".to_string())
    );
}

#[test]
fn read_oauth_prefers_claude_ai_oauth_over_root() {
    let f = write_temp(r#"{"claudeAiOauth":{"accessToken":"primary"},"oauthToken":"fallback"}"#);
    assert_eq!(
        read_oauth_from_credentials(f.path().to_str().unwrap()),
        Some("primary".to_string())
    );
}

#[test]
fn read_oauth_returns_none_when_neither_field_exists() {
    let f = write_temp(r#"{"unrelated":"data"}"#);
    assert_eq!(read_oauth_from_credentials(f.path().to_str().unwrap()), None);
}

#[test]
fn read_oauth_returns_none_for_missing_file() {
    assert_eq!(
        read_oauth_from_credentials("/tmp/borg-test-nonexistent-oauth.json"),
        None
    );
}

#[test]
fn read_oauth_returns_none_for_malformed_json() {
    let f = write_temp("{ bad json");
    assert_eq!(read_oauth_from_credentials(f.path().to_str().unwrap()), None);
}
