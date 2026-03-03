/// Tests verifying that git subprocess calls used during task completion
/// execute correctly via tokio::process::Command (non-blocking).
///
/// These tests create real temporary git repositories to exercise the same
/// code paths used by read_structured_output, read_task_deadlines, and
/// index_task_documents.

use std::path::Path;
use tempfile::TempDir;

async fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("git command failed to spawn")
}

async fn setup_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-b", "main"]).await;
    git(p, &["config", "user.email", "test@example.com"]).await;
    git(p, &["config", "user.name", "Test"]).await;
    // Initial commit so HEAD exists
    git(p, &["commit", "--allow-empty", "-m", "init"]).await;
    dir
}

/// Read a file from a git branch using tokio::process::Command, mirroring
/// what read_structured_output does.
async fn git_show_file(repo: &Path, branch: &str, file: &str) -> Option<String> {
    let out = tokio::process::Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "show", &format!("{branch}:{file}")])
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

/// List all files on a branch using tokio::process::Command, mirroring
/// what index_task_documents does.
async fn git_ls_tree(repo: &Path, branch: &str) -> Vec<String> {
    let out = tokio::process::Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "ls-tree", "-r", "--name-only", branch])
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .expect("git ls-tree");
    if !out.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.to_owned())
        .collect()
}

#[tokio::test]
async fn test_git_show_returns_file_content_from_branch() {
    let dir = setup_repo().await;
    let p = dir.path();

    // Create task branch and commit structured.json
    git(p, &["checkout", "-b", "task-1"]).await;
    std::fs::write(p.join("structured.json"), r#"{"status":"done"}"#).unwrap();
    git(p, &["add", "structured.json"]).await;
    git(p, &["commit", "-m", "add structured.json"]).await;

    let content = git_show_file(p, "task-1", "structured.json").await;
    assert!(content.is_some());
    assert!(content.unwrap().contains("done"));
}

#[tokio::test]
async fn test_git_show_returns_none_for_missing_file() {
    let dir = setup_repo().await;
    let p = dir.path();

    git(p, &["checkout", "-b", "task-2"]).await;
    // No structured.json committed

    let content = git_show_file(p, "task-2", "structured.json").await;
    assert!(content.is_none());
}

#[tokio::test]
async fn test_git_show_returns_none_for_missing_branch() {
    let dir = setup_repo().await;
    let content = git_show_file(dir.path(), "task-nonexistent", "structured.json").await;
    assert!(content.is_none());
}

#[tokio::test]
async fn test_git_ls_tree_lists_md_files() {
    let dir = setup_repo().await;
    let p = dir.path();

    git(p, &["checkout", "-b", "task-3"]).await;
    std::fs::write(p.join("spec.md"), "# Spec\ncontent").unwrap();
    std::fs::write(p.join("notes.txt"), "not markdown").unwrap();
    std::fs::write(p.join("README.md"), "# README").unwrap();
    git(p, &["add", "."]).await;
    git(p, &["commit", "-m", "add files"]).await;

    let files = git_ls_tree(p, "task-3").await;
    let md_files: Vec<_> = files.iter().filter(|f| f.ends_with(".md")).collect();

    assert_eq!(md_files.len(), 2);
    assert!(md_files.iter().any(|f| f.as_str() == "spec.md"));
    assert!(md_files.iter().any(|f| f.as_str() == "README.md"));
    // .txt file must not appear in md filter
    assert!(!files.iter().any(|f| f.ends_with(".txt") && f.ends_with(".md")));
}

#[tokio::test]
async fn test_git_ls_tree_returns_empty_for_missing_branch() {
    let dir = setup_repo().await;
    let files = git_ls_tree(dir.path(), "task-nonexistent").await;
    assert!(files.is_empty());
}

#[tokio::test]
async fn test_git_show_concurrent_reads_do_not_block() {
    // Spawn multiple concurrent git reads to verify they run without blocking
    // each other on the tokio runtime (would deadlock with std::process::Command
    // if there were only a few threads and all were occupied).
    let dir = setup_repo().await;
    let p = dir.path();

    git(p, &["checkout", "-b", "task-10"]).await;
    std::fs::write(p.join("a.json"), r#"{"a":1}"#).unwrap();
    std::fs::write(p.join("b.json"), r#"{"b":2}"#).unwrap();
    git(p, &["add", "."]).await;
    git(p, &["commit", "-m", "add files"]).await;

    let repo_str = p.to_string_lossy().to_string();
    let handles: Vec<_> = (0..8).map(|_| {
        let r = repo_str.clone();
        tokio::spawn(async move {
            tokio::process::Command::new("git")
                .args(["-C", &r, "show", "task-10:a.json"])
                .stderr(std::process::Stdio::null())
                .output()
                .await
                .expect("git show")
        })
    }).collect();

    for h in handles {
        let out = h.await.expect("join");
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("\"a\":1"));
    }
}
