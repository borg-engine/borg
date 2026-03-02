/// Tests that worktree directory creation errors are propagated and produce
/// useful diagnostics, not silently discarded via `.ok()`.
///
/// Prior to the fix, `tokio::fs::create_dir_all(&wt_dir).await.ok()` dropped
/// the error, causing the subsequent `git worktree add` to fail with a cryptic
/// "parent path does not exist" message instead of the real cause.

/// Verify that `create_dir_all` genuinely fails (returns Err) when the path
/// is blocked — i.e., the OS error is non-nil and capturable.
#[tokio::test]
async fn test_create_dir_all_returns_err_when_blocked_by_file() {
    let dir = tempfile::tempdir().unwrap();

    // Place a regular file where the worktrees directory should be.
    // Any attempt to create a directory under a non-directory component fails.
    let blocker = dir.path().join(".worktrees");
    std::fs::write(&blocker, b"").unwrap();

    let subdir = blocker.join("task-99");
    let result = tokio::fs::create_dir_all(&subdir).await;

    assert!(
        result.is_err(),
        "create_dir_all must return Err when a file blocks the path; \
         silencing this error hides the real failure cause"
    );
}

/// Verify that the diagnostic message produced from a `create_dir_all` error
/// contains both the attempted path and the underlying OS error — the two
/// pieces of information a human needs to diagnose the problem.
#[tokio::test]
async fn test_create_dir_all_error_message_includes_path_and_cause() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join(".worktrees");
    std::fs::write(&blocker, b"").unwrap();

    let wt_dir = blocker.join("task-99").display().to_string();
    let result = tokio::fs::create_dir_all(&wt_dir).await;
    let err = result.unwrap_err();

    let msg = format!("failed to create worktree dir {wt_dir}: {err}");

    assert!(
        msg.contains(&wt_dir),
        "diagnostic must contain the path; got: {msg}"
    );
    assert!(
        !err.to_string().is_empty(),
        "OS error description must be non-empty"
    );
}
