use std::fs;
use std::process::Command;
use std::sync::Mutex;

use borg_core::git::{ExecResult, Git};
use tempfile::TempDir;

// Tests that change CWD must be serialized (set_current_dir is process-wide).
static CWD_MUTEX: Mutex<()> = Mutex::new(());

fn make_result(stdout: &str, stderr: &str, exit_code: i32) -> ExecResult {
    ExecResult {
        stdout: stdout.into(),
        stderr: stderr.into(),
        exit_code,
    }
}

fn init_git_repo(dir: &TempDir) {
    let status = Command::new("git")
        .args(["init", dir.path().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "git init failed");
}

// ── ExecResult::combined_output ───────────────────────────────────────────────

#[test]
fn combined_output_stdout_only() {
    let r = make_result("hello", "", 0);
    assert_eq!(r.combined_output(), "hello");
}

#[test]
fn combined_output_both_empty() {
    let r = make_result("", "", 0);
    assert_eq!(r.combined_output(), "");
}

#[test]
fn combined_output_stderr_only() {
    // stderr non-empty: format!("{}\n{}", stdout, stderr) → "\nerr"
    let r = make_result("", "err", 1);
    assert_eq!(r.combined_output(), "\nerr");
}

#[test]
fn combined_output_stdout_and_stderr() {
    let r = make_result("out", "err", 1);
    assert_eq!(r.combined_output(), "out\nerr");
}

// ── Git::rebase_in_progress ───────────────────────────────────────────────────
//
// git rev-parse --git-path rebase-merge returns a relative path (.git/rebase-merge)
// for a non-worktree repo; exists() resolves it against the process CWD.
// We hold CWD_MUTEX and temporarily set CWD to the temp repo so the
// relative path resolves correctly.

#[test]
fn rebase_in_progress_false_when_no_rebase() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    let path = dir.path().to_str().unwrap();
    let git = Git::new(path);

    let _guard = CWD_MUTEX.lock().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = git.rebase_in_progress(path).unwrap();
    std::env::set_current_dir(orig).unwrap();

    assert!(!result);
}

#[test]
fn rebase_in_progress_true_when_rebase_merge_exists() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir(dir.path().join(".git/rebase-merge")).unwrap();
    let path = dir.path().to_str().unwrap();
    let git = Git::new(path);

    let _guard = CWD_MUTEX.lock().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = git.rebase_in_progress(path).unwrap();
    std::env::set_current_dir(orig).unwrap();

    assert!(result);
}

#[test]
fn rebase_in_progress_true_when_rebase_apply_exists() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir(dir.path().join(".git/rebase-apply")).unwrap();
    let path = dir.path().to_str().unwrap();
    let git = Git::new(path);

    let _guard = CWD_MUTEX.lock().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = git.rebase_in_progress(path).unwrap();
    std::env::set_current_dir(orig).unwrap();

    assert!(result);
}

#[test]
fn rebase_in_progress_true_when_both_exist() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir(dir.path().join(".git/rebase-merge")).unwrap();
    fs::create_dir(dir.path().join(".git/rebase-apply")).unwrap();
    let path = dir.path().to_str().unwrap();
    let git = Git::new(path);

    let _guard = CWD_MUTEX.lock().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = git.rebase_in_progress(path).unwrap();
    std::env::set_current_dir(orig).unwrap();

    assert!(result);
}
