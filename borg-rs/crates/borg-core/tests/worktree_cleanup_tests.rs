use borg_core::git::Git;
use std::process::Command;

fn init_repo(dir: &std::path::Path) {
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .expect("git init");
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"])
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test")
        .status()
        .expect("git commit");
}

#[test]
fn test_remove_worktree_nonexistent_returns_err() {
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());
    let git = Git::new(dir.path().to_str().unwrap());

    let result = git.remove_worktree("/nonexistent/path/that/does/not/exist");
    assert!(result.is_err(), "remove_worktree on nonexistent path must return Err");
}

#[test]
fn test_worktree_prune_returns_ok_in_valid_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());
    let git = Git::new(dir.path().to_str().unwrap());

    let result = git.exec(dir.path().to_str().unwrap(), &["worktree", "prune"]);
    assert!(result.is_ok(), "exec should not fail to spawn");
    assert!(result.unwrap().success(), "git worktree prune must succeed in a valid repo");
}

#[test]
fn test_remove_dir_all_notfound_is_ignored_pattern() {
    // Simulate the guard pattern used in cleanup: only warn on non-NotFound errors.
    let missing = std::path::Path::new("/tmp/borg-test-nonexistent-worktree-xyz");
    let result = std::fs::remove_dir_all(missing);
    if let Err(e) = result {
        assert_eq!(
            e.kind(),
            std::io::ErrorKind::NotFound,
            "missing dir should produce NotFound, not an unexpected error"
        );
    }
}

#[test]
fn test_remove_worktree_error_message_contains_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    init_repo(dir.path());
    let git = Git::new(dir.path().to_str().unwrap());

    let bogus = "/no/such/worktree";
    let err = git.remove_worktree(bogus).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(bogus) || msg.contains("worktree"),
        "error message should mention the path or operation: {msg}"
    );
}
