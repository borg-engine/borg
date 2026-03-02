use borg_agent::instruction::build_instruction;
use borg_core::types::{PhaseConfig, PhaseContext, RepoConfig, Task};
use chrono::Utc;
use std::collections::HashMap;

fn make_task(title: &str, description: &str, last_error: &str) -> Task {
    Task {
        id: 1,
        title: title.into(),
        description: description.into(),
        repo_path: "/nonexistent".into(),
        branch: "task-1".into(),
        status: "implement".into(),
        attempt: 1,
        max_attempts: 3,
        last_error: last_error.into(),
        created_by: "test".into(),
        notify_chat: String::new(),
        created_at: Utc::now(),
        session_id: String::new(),
        mode: "sweborg".into(),
        backend: String::new(),
    }
}

fn make_phase(instruction: &str) -> PhaseConfig {
    PhaseConfig {
        instruction: instruction.into(),
        ..PhaseConfig::default()
    }
}

fn make_ctx() -> PhaseContext {
    PhaseContext {
        task: make_task("", "", ""),
        repo_config: RepoConfig {
            path: "/nonexistent".into(),
            test_cmd: String::new(),
            prompt_file: String::new(),
            mode: "sweborg".into(),
            is_self: false,
            auto_merge: false,
            lint_cmd: String::new(),
            backend: String::new(),
            repo_slug: String::new(),
        },
        data_dir: "/nonexistent".into(),
        session_dir: "/nonexistent".into(),
        worktree_path: "/nonexistent".into(),
        oauth_token: String::new(),
        model: "sonnet".into(),
        pending_messages: vec![],
        system_prompt_suffix: String::new(),
        user_coauthor: String::new(),
        stream_tx: None,
        setup_script: String::new(),
        api_keys: HashMap::new(),
        disallowed_tools: String::new(),
        knowledge_files: vec![],
        knowledge_dir: "/nonexistent".into(),
        agent_network: None,
    }
}

// ── Error section ─────────────────────────────────────────────────────────

#[test]
fn test_empty_error_leaves_no_error_section() {
    let task = make_task("T", "D", "");
    let phase = PhaseConfig {
        instruction: "Do the thing.".into(),
        error_instruction: "Previous error: {ERROR}".into(),
        ..PhaseConfig::default()
    };
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(!out.contains("Previous error"), "error section should be absent: {out}");
    assert!(!out.contains("{ERROR}"), "placeholder must not appear: {out}");
}

#[test]
fn test_empty_error_instruction_leaves_no_error_section() {
    let task = make_task("T", "D", "oops");
    let phase = PhaseConfig {
        instruction: "Do the thing.".into(),
        error_instruction: String::new(),
        ..PhaseConfig::default()
    };
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(!out.contains("oops"), "error must not appear when error_instruction is empty: {out}");
}

#[test]
fn test_error_placeholder_is_substituted() {
    let task = make_task("T", "D", "compilation failed");
    let phase = PhaseConfig {
        instruction: "Do the thing.".into(),
        error_instruction: "Fix this: {ERROR}".into(),
        ..PhaseConfig::default()
    };
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(out.contains("Fix this: compilation failed"), "placeholder not substituted: {out}");
    assert!(!out.contains("{ERROR}"), "raw placeholder must not remain: {out}");
}

// ── File listing ──────────────────────────────────────────────────────────

#[test]
fn test_file_listing_omitted_when_none() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Implement.");
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(!out.contains("Files in repository"), "file listing present but should be absent: {out}");
}

#[test]
fn test_file_listing_omitted_when_empty_string() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Implement.");
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, Some(""));
    assert!(!out.contains("Files in repository"), "file listing present for empty string: {out}");
}

#[test]
fn test_file_listing_included_when_provided() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Implement.");
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, Some("src/main.rs\nsrc/lib.rs\n"));
    assert!(out.contains("Files in repository"), "file listing missing: {out}");
    assert!(out.contains("src/main.rs"), "file not in listing: {out}");
}

// ── Task context ──────────────────────────────────────────────────────────

#[test]
fn test_include_task_context_false_omits_title_and_description() {
    let task = make_task("My Task Title", "My task description.", "");
    let phase = PhaseConfig {
        instruction: "Do work.".into(),
        include_task_context: false,
        ..PhaseConfig::default()
    };
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(!out.contains("My Task Title"), "title present but should be absent: {out}");
    assert!(!out.contains("My task description."), "description present but should be absent: {out}");
}

#[test]
fn test_include_task_context_true_includes_title_and_description() {
    let task = make_task("My Task Title", "My task description.", "");
    let phase = PhaseConfig {
        instruction: "Do work.".into(),
        include_task_context: true,
        ..PhaseConfig::default()
    };
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(out.contains("My Task Title"), "title missing: {out}");
    assert!(out.contains("My task description."), "description missing: {out}");
}

// ── Pending messages ──────────────────────────────────────────────────────

#[test]
fn test_pending_messages_appended_with_role_prefix() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Implement.");
    let mut ctx = make_ctx();
    ctx.pending_messages = vec![
        ("user".into(), "Please also add tests.".into()),
        ("director".into(), "Focus on the auth module.".into()),
    ];
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(out.contains("[user]: Please also add tests."), "user message missing: {out}");
    assert!(out.contains("[director]: Focus on the auth module."), "director message missing: {out}");
}

#[test]
fn test_no_pending_messages_leaves_no_pending_section() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Implement.");
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(!out.contains("messages were sent"), "pending section present with no messages: {out}");
}

// ── Instruction always present ────────────────────────────────────────────

#[test]
fn test_phase_instruction_always_present() {
    let task = make_task("T", "D", "");
    let phase = make_phase("Write the implementation now.");
    let ctx = make_ctx();
    let out = build_instruction(&task, &phase, &ctx, None);
    assert!(out.contains("Write the implementation now."), "phase instruction missing: {out}");
}
