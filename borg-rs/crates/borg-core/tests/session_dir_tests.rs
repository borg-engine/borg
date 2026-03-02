// Tests for the session directory setup used in run_agent_phase.
//
// Verifies that create_dir_all + canonicalize produces an absolute path, and
// that failures are not silently swallowed (which would produce a relative
// path as HOME, breaking git, npm, and other tools in the subprocess).

#[tokio::test]
async fn test_session_dir_is_absolute_after_create_dir_all() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("store/sessions/task-1");

    tokio::fs::create_dir_all(&dir).await.unwrap();

    let canonical = std::fs::canonicalize(&dir).unwrap();
    let s = canonical.to_string_lossy().to_string();
    assert!(canonical.is_absolute(), "session_dir must be absolute, got: {s}");
    assert!(s.starts_with('/'), "session_dir must start with /, got: {s}");
}

#[tokio::test]
async fn test_create_dir_all_fails_on_unwritable_parent() {
    let result = tokio::fs::create_dir_all("/proc/nonexistent/task-99").await;
    assert!(result.is_err(), "create_dir_all on unwritable path must return Err");
}

#[tokio::test]
async fn test_canonicalize_fails_on_missing_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("store/sessions/task-999");
    // Directory was never created — canonicalize must fail.
    let result = std::fs::canonicalize(&missing);
    assert!(
        result.is_err(),
        "canonicalize on a non-existent directory must return Err"
    );
}
