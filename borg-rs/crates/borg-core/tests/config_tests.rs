use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn write_credentials(dir: &TempDir, expiry_ms: Option<i64>, token: &str) -> String {
    let path = dir.path().join("credentials.json");
    let json = if let Some(expiry) = expiry_ms {
        serde_json::json!({
            "claudeAiOauth": { "accessToken": token, "expiresAt": expiry }
        })
    } else {
        serde_json::json!({
            "claudeAiOauth": { "accessToken": token }
        })
    };
    std::fs::write(&path, json.to_string()).unwrap();
    path.to_string_lossy().into_owned()
}

fn past_expiry_ms() -> i64 {
    1_000_000 // far in the past
}

fn future_expiry_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        + 3_600_000 // 1 hour from now
}

#[test]
fn test_refresh_oauth_no_credentials_returns_fallback() {
    let token =
        borg_core::config::refresh_oauth_token("/nonexistent/path/.credentials.json", "fallback");
    assert_eq!(token, "fallback");
}

#[test]
fn test_refresh_oauth_no_expiry_skips_refresh_returns_file_token() {
    let dir = TempDir::new().unwrap();
    let path = write_credentials(&dir, None, "file_token");
    // No expiry → no refresh triggered, should return the file token
    let token = borg_core::config::refresh_oauth_token(&path, "old_token");
    assert_eq!(token, "file_token");
}

#[test]
fn test_refresh_oauth_future_expiry_skips_refresh() {
    let dir = TempDir::new().unwrap();
    let path = write_credentials(&dir, Some(future_expiry_ms()), "fresh_token");
    let token = borg_core::config::refresh_oauth_token(&path, "fallback");
    assert_eq!(token, "fresh_token");
}

#[test]
fn test_refresh_oauth_past_expiry_command_missing_returns_file_token() {
    // With a past expiry the refresh path is triggered, but if `claude` is not on PATH
    // (or exits with an error), the function should still return the file token.
    let dir = TempDir::new().unwrap();
    let bin_dir = TempDir::new().unwrap();

    // Fake `claude` that exits immediately with an error
    let fake = bin_dir.path().join("claude");
    std::fs::write(&fake, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.path().display(), orig_path);
    // SAFETY: single-threaded test context; no other threads read PATH concurrently here.
    unsafe { std::env::set_var("PATH", &new_path) };

    let path = write_credentials(&dir, Some(past_expiry_ms()), "stale_token");
    let token = borg_core::config::refresh_oauth_token(&path, "fallback");

    unsafe { std::env::set_var("PATH", orig_path) };

    assert_eq!(token, "stale_token");
}

#[test]
fn test_refresh_oauth_timeout_does_not_block_indefinitely() {
    // Fake `claude` that hangs; the function must return within the 5s timeout.
    let dir = TempDir::new().unwrap();
    let bin_dir = TempDir::new().unwrap();

    let fake = bin_dir.path().join("claude");
    std::fs::write(&fake, "#!/bin/sh\nsleep 60\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.path().display(), orig_path);
    unsafe { std::env::set_var("PATH", &new_path) };

    let path = write_credentials(&dir, Some(past_expiry_ms()), "stale_token");

    let start = std::time::Instant::now();
    let token = borg_core::config::refresh_oauth_token(&path, "fallback");
    let elapsed = start.elapsed();

    unsafe { std::env::set_var("PATH", orig_path) };

    // Must complete within 7 seconds (5s timeout + 2s margin)
    assert!(
        elapsed.as_secs() < 7,
        "refresh_oauth_token blocked too long: {elapsed:?}"
    );
    assert_eq!(token, "stale_token");
}
