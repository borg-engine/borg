use borg_agent::instruction::build_instruction;
use borg_core::types::{PhaseConfig, PhaseContext, RepoConfig, Task};

fn base_task() -> Task {
    Task {
        id: 1,
        title: "My Task Title".into(),
        description: "My task description.".into(),
        repo_path: String::new(),
        branch: String::new(),
        status: "implement".into(),
        attempt: 0,
        max_attempts: 3,
        last_error: String::new(),
        created_by: "test".into(),
        notify_chat: String::new(),
        created_at: Default::default(),
        session_id: String::new(),
        mode: "sweborg".into(),
        backend: String::new(),
        project_id: 0,
    }
}

fn base_phase() -> PhaseConfig {
    PhaseConfig {
        instruction: "Do the work.".into(),
        ..PhaseConfig::default()
    }
}

fn base_ctx() -> PhaseContext {
    // Use a non-existent path so read_repo_prompt returns None for all lookups,
    // and set repo_config.path == worktree_path to skip the third lookup too.
    let no_path = "/nonexistent-test-path-borg-instruction-tests".to_string();
    PhaseContext {
        task: base_task(),
        repo_config: RepoConfig {
            path: no_path.clone(),
            test_cmd: String::new(),
            prompt_file: String::new(),
            mode: "sweborg".into(),
            is_self: false,
            auto_merge: false,
            lint_cmd: String::new(),
            backend: String::new(),
            repo_slug: String::new(),
        },
        data_dir: String::new(),
        session_dir: String::new(),
        worktree_path: no_path,
        oauth_token: String::new(),
        model: String::new(),
        pending_messages: vec![],
        system_prompt_suffix: String::new(),
        user_coauthor: String::new(),
        stream_tx: None,
        setup_script: String::new(),
        api_keys: std::collections::HashMap::new(),
        disallowed_tools: String::new(),
        knowledge_files: vec![],
        knowledge_dir: String::new(),
        agent_network: None,
    }
}

// ── include_task_context ──────────────────────────────────────────────────

#[test]
fn task_context_included_when_flag_true() {
    let task = base_task();
    let phase = PhaseConfig {
        include_task_context: true,
        ..base_phase()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(result.contains("My Task Title"), "title missing: {result}");
    assert!(result.contains("My task description."), "description missing: {result}");
}

#[test]
fn task_context_omitted_when_flag_false() {
    let task = base_task();
    let phase = PhaseConfig {
        include_task_context: false,
        ..base_phase()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(!result.contains("My Task Title"), "title should be absent: {result}");
    assert!(!result.contains("My task description."), "description should be absent: {result}");
}

// ── error_instruction / last_error ───────────────────────────────────────

#[test]
fn error_substituted_when_last_error_and_instruction_set() {
    let mut task = base_task();
    task.last_error = "compilation failed on line 42".into();
    let phase = PhaseConfig {
        error_instruction: "Previous error: {ERROR}".into(),
        ..base_phase()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(
        result.contains("Previous error: compilation failed on line 42"),
        "error not substituted: {result}"
    );
    assert!(!result.contains("{ERROR}"), "placeholder not replaced: {result}");
}

#[test]
fn error_section_omitted_when_last_error_empty() {
    let task = base_task(); // last_error is ""
    let phase = PhaseConfig {
        error_instruction: "Previous error: {ERROR}".into(),
        ..base_phase()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(
        !result.contains("Previous error"),
        "error section should be absent: {result}"
    );
}

#[test]
fn error_section_omitted_when_error_instruction_empty() {
    let mut task = base_task();
    task.last_error = "some error".into();
    let phase = PhaseConfig {
        error_instruction: String::new(), // empty
        ..base_phase()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(
        !result.contains("some error"),
        "error content should be absent when error_instruction is empty: {result}"
    );
}

// ── pending_messages ──────────────────────────────────────────────────────

#[test]
fn pending_messages_appended_with_role_format() {
    let task = base_task();
    let phase = base_phase();
    let mut ctx = base_ctx();
    ctx.pending_messages = vec![
        ("user".into(), "Please add error handling.".into()),
        ("director".into(), "Focus on the auth module.".into()),
    ];

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(result.contains("[user]: Please add error handling."), "user message missing: {result}");
    assert!(
        result.contains("[director]: Focus on the auth module."),
        "director message missing: {result}"
    );
}

#[test]
fn empty_pending_messages_adds_nothing() {
    let task = base_task();
    let phase = base_phase();
    let ctx = base_ctx(); // pending_messages is empty

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(
        !result.contains("messages were sent"),
        "messages header should be absent: {result}"
    );
}

// ── file_listing ──────────────────────────────────────────────────────────

#[test]
fn file_listing_wrapped_in_code_fence() {
    let task = base_task();
    let phase = base_phase();
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, Some("src/main.rs\nsrc/lib.rs"));

    assert!(result.contains("```\nsrc/main.rs\nsrc/lib.rs```"), "code fence missing: {result}");
    assert!(result.contains("Files in repository:"), "header missing: {result}");
}

#[test]
fn empty_file_listing_omits_section() {
    let task = base_task();
    let phase = base_phase();
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, Some(""));

    assert!(!result.contains("Files in repository:"), "section should be absent for empty listing: {result}");
}

#[test]
fn none_file_listing_omits_section() {
    let task = base_task();
    let phase = base_phase();
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(!result.contains("Files in repository:"), "section should be absent when listing is None: {result}");
}

// ── phase instruction always present ─────────────────────────────────────

#[test]
fn phase_instruction_always_included() {
    let task = base_task();
    let phase = PhaseConfig {
        instruction: "Implement the feature.".into(),
        ..PhaseConfig::default()
    };
    let ctx = base_ctx();

    let result = build_instruction(&task, &phase, &ctx, None);

    assert!(result.contains("Implement the feature."), "instruction missing: {result}");
}
