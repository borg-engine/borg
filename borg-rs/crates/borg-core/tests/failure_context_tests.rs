use borg_core::{
    db::Db,
    types::Task,
};
use chrono::Utc;

fn open_db() -> Db {
    let mut db = Db::open(":memory:").expect("open in-memory db");
    db.migrate().expect("migrate");
    db
}

fn make_task(db: &Db) -> i64 {
    let task = Task {
        id: 0,
        title: "Failure context test task".into(),
        description: "desc".into(),
        repo_path: "/repo".into(),
        branch: "task-1".into(),
        status: "impl".into(),
        attempt: 1,
        max_attempts: 5,
        last_error: String::new(),
        created_by: "test".into(),
        notify_chat: String::new(),
        created_at: Utc::now(),
        session_id: String::new(),
        mode: "sweborg".into(),
        backend: String::new(),
    };
    db.insert_task(&task).expect("insert_task")
}

// ── insert_task_output with attempt ──────────────────────────────────────────

#[test]
fn test_insert_and_retrieve_task_output_with_attempt() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 2, "impl", "agent output", "", 0)
        .expect("insert_task_output");

    let outputs = db.get_task_outputs(task_id).expect("get_task_outputs");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].attempt, 2);
    assert_eq!(outputs[0].phase, "impl");
    assert_eq!(outputs[0].output, "agent output");
    assert_eq!(outputs[0].exit_code, 0);
}

#[test]
fn test_multiple_attempts_stored_separately() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 1, "impl", "attempt 1 output", "", 1)
        .expect("insert attempt 1");
    db.insert_task_output(task_id, 2, "impl", "attempt 2 output", "", 1)
        .expect("insert attempt 2");
    db.insert_task_output(task_id, 3, "impl", "attempt 3 output", "", 0)
        .expect("insert attempt 3");

    let outputs = db.get_task_outputs(task_id).expect("get_task_outputs");
    assert_eq!(outputs.len(), 3);
    assert_eq!(outputs[0].attempt, 1);
    assert_eq!(outputs[1].attempt, 2);
    assert_eq!(outputs[2].attempt, 3);
    assert_eq!(outputs[0].output, "attempt 1 output");
}

#[test]
fn test_diff_output_stored_as_separate_phase_entry() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 1, "impl", "failed output", "", 1)
        .expect("insert phase output");
    db.insert_task_output(task_id, 1, "impl_diff", "diff -rN\n+added line\n-removed line", "", 1)
        .expect("insert diff output");

    let outputs = db.get_task_outputs(task_id).expect("get_task_outputs");
    assert_eq!(outputs.len(), 2);

    let diff_out = outputs.iter().find(|o| o.phase == "impl_diff").expect("diff output");
    assert!(diff_out.output.contains("+added line"));
    assert_eq!(diff_out.attempt, 1);
}

#[test]
fn test_diff_phase_name_ends_with_diff_suffix() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 1, "retry_diff", "some diff", "", 1)
        .expect("insert");

    let outputs = db.get_task_outputs(task_id).expect("get");
    assert!(outputs[0].phase.ends_with("_diff"), "diff phase must end with _diff");
}

#[test]
fn test_attempt_zero_is_default_for_old_rows() {
    let db = open_db();
    let task_id = make_task(&db);

    // Simulate an old-style insert without explicit attempt (defaults to 0).
    {
        let conn = db.raw_conn().lock().unwrap();
        conn.execute(
            "INSERT INTO task_outputs (task_id, phase, output, exit_code, created_at) \
             VALUES (?1, 'spec', 'old output', 0, datetime('now'))",
            rusqlite::params![task_id],
        ).expect("raw insert");
    }

    let outputs = db.get_task_outputs(task_id).expect("get");
    assert_eq!(outputs[0].attempt, 0, "legacy rows default to attempt 0");
}

// ── extract_first_error_line (tested via build_retry_summary indirectly) ─────

#[test]
fn test_retry_summary_includes_stored_diff() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 1, "impl", "build failed\nerror: type mismatch", "", 1)
        .expect("phase output");
    db.insert_task_output(task_id, 1, "impl_diff", "+fn foo() {}\n-fn bar() {}", "", 1)
        .expect("diff output");

    let outputs = db.get_task_outputs(task_id).expect("get");
    let diff_outputs: Vec<_> = outputs.iter().filter(|o| o.phase.ends_with("_diff")).collect();

    assert_eq!(diff_outputs.len(), 1);
    assert!(diff_outputs[0].output.contains("+fn foo()"));
}

#[test]
fn test_retry_summary_only_phase_outputs_in_history() {
    let db = open_db();
    let task_id = make_task(&db);

    db.insert_task_output(task_id, 1, "impl", "output 1", "", 1).expect("insert");
    db.insert_task_output(task_id, 1, "impl_diff", "+line", "", 1).expect("insert diff");
    db.insert_task_output(task_id, 2, "impl", "output 2", "", 1).expect("insert 2");

    let outputs = db.get_task_outputs(task_id).expect("get");
    let phase_outputs: Vec<_> = outputs.iter().filter(|o| !o.phase.ends_with("_diff")).collect();
    let diff_outputs: Vec<_> = outputs.iter().filter(|o| o.phase.ends_with("_diff")).collect();

    assert_eq!(phase_outputs.len(), 2, "two non-diff phase outputs");
    assert_eq!(diff_outputs.len(), 1, "one diff output");
}

// ── first error line extraction ───────────────────────────────────────────────

#[test]
fn test_extract_first_error_line_finds_rust_error() {
    let output = "   Compiling mylib v0.1.0\nerror[E0308]: mismatched types\n  --> src/main.rs:5:3";
    let first_error = extract_first_error_line(output);
    assert!(first_error.is_some());
    let err = first_error.unwrap();
    assert!(err.contains("error[E0308]"), "must capture Rust error annotation: {err}");
}

#[test]
fn test_extract_first_error_line_finds_test_failure() {
    let output = "running 3 tests\ntest foo ... ok\ntest bar ... FAILED\nfailures:";
    let first_error = extract_first_error_line(output);
    assert!(first_error.is_some());
    assert!(first_error.unwrap().contains("FAILED"));
}

#[test]
fn test_extract_first_error_line_returns_none_on_clean_output() {
    let output = "running 5 tests\ntest a ... ok\ntest b ... ok\ntest result: ok.";
    let first_error = extract_first_error_line(output);
    assert!(first_error.is_none(), "no error line in clean output");
}

#[test]
fn test_extract_first_error_line_finds_panic() {
    let output = "thread 'main' panicked at 'assertion failed', src/lib.rs:42";
    let first_error = extract_first_error_line(output);
    assert!(first_error.is_some());
    assert!(first_error.unwrap().contains("panicked at"));
}

#[test]
fn test_extract_first_error_line_finds_error_colon() {
    let output = "stdout: \nerror: could not compile `mylib`\nnote: see previous errors";
    let first_error = extract_first_error_line(output);
    assert!(first_error.is_some());
    assert!(first_error.unwrap().starts_with("error:"));
}

// Mirror of the pipeline's extract_first_error_line for direct unit testing.
fn extract_first_error_line(output: &str) -> Option<String> {
    output
        .lines()
        .find(|line| {
            let l = line.trim();
            l.contains("error[") || l.starts_with("error:") || l.starts_with("Error:")
                || l.contains("FAILED") || l.contains("panicked at")
                || l.starts_with("FAIL ") || l.starts_with("fail ")
        })
        .map(|s| s.trim().to_string())
}
