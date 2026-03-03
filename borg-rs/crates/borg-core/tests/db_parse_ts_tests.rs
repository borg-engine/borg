use borg_core::db::Db;
use borg_core::types::Task;
use chrono::Utc;

fn open_db() -> Db {
    let mut db = Db::open(":memory:").expect("open in-memory db");
    db.migrate().expect("migrate");
    db
}

fn insert_task_raw(db: &Db) -> i64 {
    let task = Task {
        id: 0,
        title: "ts-test".into(),
        description: "desc".into(),
        repo_path: "/repo".into(),
        branch: "task-ts".into(),
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
    };
    db.insert_task(&task).expect("insert_task")
}

fn corrupt_task_ts(db: &Db, task_id: i64, bad_ts: &str) {
    let conn = db.raw_conn().lock().unwrap();
    conn.execute(
        "UPDATE pipeline_tasks SET created_at = ?1 WHERE id = ?2",
        rusqlite::params![bad_ts, task_id],
    )
    .expect("corrupt created_at");
}

// ── get_task returns Err on malformed created_at ──────────────────────────────

#[test]
fn test_get_task_errors_on_malformed_timestamp() {
    let db = open_db();
    let id = insert_task_raw(&db);
    corrupt_task_ts(&db, id, "not-a-date");

    let result = db.get_task(id);
    assert!(result.is_err(), "get_task must return Err on malformed created_at");
}

#[test]
fn test_get_task_errors_on_empty_timestamp() {
    let db = open_db();
    let id = insert_task_raw(&db);
    corrupt_task_ts(&db, id, "");

    let result = db.get_task(id);
    assert!(result.is_err(), "get_task must return Err on empty created_at");
}

// ── list_active_tasks propagates the error ────────────────────────────────────

#[test]
fn test_list_active_tasks_errors_on_malformed_timestamp() {
    let db = open_db();
    let id = insert_task_raw(&db);
    corrupt_task_ts(&db, id, "2099/01/01");

    let result = db.list_active_tasks();
    assert!(
        result.is_err(),
        "list_active_tasks must return Err when a row has a malformed created_at"
    );
}

// ── valid timestamps still parse correctly ────────────────────────────────────

#[test]
fn test_get_task_succeeds_with_valid_timestamp() {
    let db = open_db();
    let id = insert_task_raw(&db);

    let result = db.get_task(id);
    assert!(result.is_ok(), "get_task must succeed with a valid timestamp");
    let task = result.unwrap().expect("task must exist");
    assert_eq!(task.id, id);
}

// ── no silent Utc::now() substitution ────────────────────────────────────────
// A row with a bad timestamp must not silently appear as "just created".

#[test]
fn test_no_silent_now_substitution_on_bad_timestamp() {
    let db = open_db();
    let id = insert_task_raw(&db);

    let before = Utc::now();
    corrupt_task_ts(&db, id, "garbage");
    let result = db.get_task(id);
    let after = Utc::now();

    match result {
        Ok(Some(task)) => {
            // If it somehow succeeded, the timestamp must NOT be a fresh Utc::now().
            // Allow a 1-second tolerance.
            let diff = (task.created_at - before).num_seconds().abs();
            assert!(
                diff > 1,
                "parse_ts must not silently substitute Utc::now() for bad timestamps; \
                 got created_at={}, before={before}",
                task.created_at,
            );
            let _ = after;
        }
        Ok(None) => panic!("task should exist"),
        Err(_) => {} // expected path: error is correctly propagated
    }
}
