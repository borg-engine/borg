use std::collections::HashMap;
use std::fs;

use borg_agent::instruction::build_instruction;
use borg_core::{
    db::KnowledgeFile,
    types::{PhaseConfig, PhaseContext, RepoConfig, Task},
};
use chrono::Utc;

fn make_task(title: &str, description: &str, last_error: &str) -> Task {
    Task {
        id: 1,
        title: title.to_string(),
        description: description.to_string(),
        repo_path: String::new(),
        branch: String::new(),
        status: "implement".to_string(),
        attempt: 1,
        max_attempts: 3,
        last_error: last_error.to_string(),
        created_by: String::new(),
        notify_chat: String::new(),
        created_at: Utc::now(),
        session_id: String::new(),
        mode: String::new(),
        backend: String::new(),
    }
}

fn make_repo_config(path: &str) -> RepoConfig {
    RepoConfig {
        path: path.to_string(),
        test_cmd: String::new(),
        prompt_file: String::new(),
        mode: String::new(),
        is_self: false,
        auto_merge: false,
        lint_cmd: String::new(),
        backend: String::new(),
        repo_slug: String::new(),
    }
}

fn make_ctx(
    task: Task,
    repo_config: RepoConfig,
    worktree_path: &str,
    knowledge_files: Vec<KnowledgeFile>,
    knowledge_dir: &str,
    pending_messages: Vec<(String, String)>,
) -> PhaseContext {
    PhaseContext {
        task,
        repo_config,
        data_dir: String::new(),
        session_dir: String::new(),
        worktree_path: worktree_path.to_string(),
        oauth_token: String::new(),
        model: String::new(),
        pending_messages,
        system_prompt_suffix: String::new(),
        user_coauthor: String::new(),
        stream_tx: None,
        setup_script: String::new(),
        api_keys: HashMap::new(),
        disallowed_tools: String::new(),
        knowledge_files,
        knowledge_dir: knowledge_dir.to_string(),
        agent_network: None,
    }
}

/// All optional fields populated: knowledge files, repo prompt, task context,
/// file listing, error instruction with {ERROR}, and pending messages.
#[test]
fn test_all_fields_populated() {
    let tmp = std::env::temp_dir().join("borg_instr_test_all");
    let borg_dir = tmp.join(".borg");
    fs::create_dir_all(&borg_dir).expect("create .borg dir");
    fs::write(borg_dir.join("prompt.md"), "Project context: use Rust 2021 edition").expect("write prompt.md");

    let kb_dir = std::env::temp_dir().join("borg_instr_test_kb");
    fs::create_dir_all(&kb_dir).expect("create kb dir");
    fs::write(kb_dir.join("style.md"), "Follow the style guide").expect("write style.md");

    let tmp_str = tmp.to_string_lossy().to_string();
    let kb_str = kb_dir.to_string_lossy().to_string();

    let task = make_task("Add logging", "Add structured logging to the app", "compilation error on line 42");
    let repo_config = make_repo_config(&tmp_str);
    let ctx = make_ctx(
        task,
        repo_config,
        &tmp_str,
        vec![KnowledgeFile {
            id: 1,
            file_name: "style.md".to_string(),
            description: "Style guide".to_string(),
            size_bytes: 100,
            inline: true,
            created_at: String::new(),
        }],
        &kb_str,
        vec![("user".to_string(), "focus on the error handling".to_string())],
    );
    let phase = PhaseConfig {
        instruction: "Implement the changes.".to_string(),
        include_task_context: true,
        error_instruction: "Previous error: {ERROR}".to_string(),
        ..PhaseConfig::default()
    };

    let result = build_instruction(&ctx.task, &phase, &ctx, Some("src/main.rs\nsrc/lib.rs"));

    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::remove_dir_all(&kb_dir);

    assert!(result.contains("## Knowledge Base"), "missing knowledge section");
    assert!(result.contains("style.md"), "missing knowledge file name");
    assert!(result.contains("Follow the style guide"), "missing inline knowledge content");
    assert!(result.contains("## Project Context"), "missing repo context section");
    assert!(result.contains("use Rust 2021 edition"), "missing prompt.md content");
    assert!(result.contains("Task: Add logging"), "missing task title");
    assert!(result.contains("structured logging"), "missing task description");
    assert!(result.contains("Implement the changes."), "missing phase instruction");
    assert!(result.contains("src/main.rs"), "missing file listing");
    assert!(result.contains("Previous error: compilation error on line 42"), "missing error context");
    assert!(result.contains("focus on the error handling"), "missing pending message");
}

/// When knowledge_files is empty, no knowledge section is emitted.
#[test]
fn test_empty_knowledge_files() {
    let task = make_task("Fix bug", "Fix the null pointer", "");
    let repo_config = make_repo_config("/tmp/borg_instr_noexist_1");
    let ctx = make_ctx(task, repo_config, "/tmp/borg_instr_noexist_1", vec![], "", vec![]);
    let phase = PhaseConfig {
        instruction: "Do the fix.".to_string(),
        ..PhaseConfig::default()
    };

    let result = build_instruction(&ctx.task, &phase, &ctx, None);

    assert!(!result.contains("## Knowledge Base"), "should not emit knowledge section");
    assert!(result.contains("Do the fix."), "should still have phase instruction");
}

/// When no .borg/prompt.md exists at worktree or repo path, no project context
/// section is emitted.
#[test]
fn test_no_repo_prompt_file() {
    let task = make_task("Refactor", "Clean up the code", "");
    let repo_config = make_repo_config("/tmp/borg_instr_noexist_2");
    let ctx = make_ctx(task, repo_config, "/tmp/borg_instr_noexist_2", vec![], "", vec![]);
    let phase = PhaseConfig {
        instruction: "Refactor now.".to_string(),
        ..PhaseConfig::default()
    };

    let result = build_instruction(&ctx.task, &phase, &ctx, None);

    assert!(!result.contains("## Project Context"), "should not emit project context section");
    assert!(result.contains("Refactor now."), "should still have phase instruction");
}

/// The {ERROR} placeholder in error_instruction is replaced with task.last_error.
#[test]
fn test_error_placeholder_replaced() {
    let task = make_task("Fix tests", "Make tests pass", "test_foo failed: assertion failed at line 7");
    let repo_config = make_repo_config("/tmp/borg_instr_noexist_3");
    let ctx = make_ctx(task, repo_config, "/tmp/borg_instr_noexist_3", vec![], "", vec![]);
    let phase = PhaseConfig {
        instruction: "Fix the issues.".to_string(),
        error_instruction: "The following error occurred:\n{ERROR}\nPlease address it.".to_string(),
        ..PhaseConfig::default()
    };

    let result = build_instruction(&ctx.task, &phase, &ctx, None);

    assert!(!result.contains("{ERROR}"), "{{ERROR}} placeholder should be replaced");
    assert!(result.contains("test_foo failed: assertion failed at line 7"), "error text should appear");
    assert!(result.contains("The following error occurred:"), "error instruction prefix should be present");
    assert!(result.contains("Please address it."), "error instruction suffix should be present");
}
