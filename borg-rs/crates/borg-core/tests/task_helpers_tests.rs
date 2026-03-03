use borg_core::{pipeline::task_session_dir, types::{Task, task_branch_name}};
use chrono::Utc;

fn make_task(id: i64) -> Task {
    Task {
        id,
        title: "Test".into(),
        description: String::new(),
        repo_path: "/repo".into(),
        branch: format!("task-{id}"),
        status: "backlog".into(),
        attempt: 0,
        max_attempts: 5,
        last_error: String::new(),
        created_by: "test".into(),
        notify_chat: String::new(),
        created_at: Utc::now(),
        session_id: String::new(),
        mode: "sweborg".into(),
        backend: String::new(),
        project_id: 0,
        task_type: String::new(),
        started_at: None,
        completed_at: None,
        duration_secs: None,
        review_status: None,
        revision_count: 0,
    }
}

// ── task_branch_name / Task::branch_name ─────────────────────────────────────

#[test]
fn test_branch_name_format() {
    let task = make_task(1);
    assert_eq!(task.branch_name(), "task-1");
}

#[test]
fn test_branch_name_large_id() {
    let task = make_task(42);
    assert_eq!(task.branch_name(), "task-42");
}

#[test]
fn test_branch_name_zero_id() {
    let task = make_task(0);
    assert_eq!(task.branch_name(), "task-0");
}

#[test]
fn test_branch_name_matches_format_macro() {
    for id in [1i64, 7, 100, 9999] {
        let task = make_task(id);
        assert_eq!(task.branch_name(), format!("task-{}", id));
    }
}

#[test]
fn test_task_branch_name_standalone_matches_method() {
    for id in [1i64, 5, 42, 1000] {
        let task = make_task(id);
        assert_eq!(task_branch_name(id), task.branch_name());
    }
}

#[test]
fn test_task_branch_name_standalone_format() {
    assert_eq!(task_branch_name(7), "task-7");
    assert_eq!(task_branch_name(0), "task-0");
    assert_eq!(task_branch_name(99), "task-99");
}

// ── task_session_dir ──────────────────────────────────────────────────────────

#[test]
fn test_task_session_dir_contains_task_id() {
    let path = task_session_dir(7);
    assert!(path.contains("task-7"), "path '{path}' must contain 'task-7'");
}

#[test]
fn test_task_session_dir_contains_sessions_segment() {
    let path = task_session_dir(42);
    assert!(
        path.contains("sessions"),
        "path '{path}' must contain 'sessions'"
    );
}

#[test]
fn test_task_session_dir_different_ids_are_distinct() {
    assert_ne!(task_session_dir(1), task_session_dir(2));
}

#[test]
fn test_task_session_dir_fallback_when_dir_absent() {
    // For a non-existent directory, the path still ends with the expected segment.
    let path = task_session_dir(99999);
    assert!(path.ends_with("task-99999") || path.contains("task-99999"));
}

#[test]
fn test_task_session_dir_returns_absolute_path_when_dir_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let orig_dir = std::env::current_dir().expect("cwd");

    std::env::set_current_dir(dir.path()).expect("chdir");
    std::fs::create_dir_all("store/sessions/task-55").expect("create");
    let path = task_session_dir(55);
    std::env::set_current_dir(&orig_dir).expect("restore cwd");

    assert!(
        std::path::Path::new(&path).is_absolute(),
        "canonicalized path must be absolute"
    );
    assert!(path.ends_with("task-55"));
}
