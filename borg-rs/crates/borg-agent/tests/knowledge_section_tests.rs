use borg_agent::instruction::build_knowledge_section;
use borg_core::db::KnowledgeFile;
use std::fs;
use std::path::PathBuf;

fn kf(file_name: &str, description: &str, inline: bool) -> KnowledgeFile {
    KnowledgeFile {
        id: 1,
        file_name: file_name.to_string(),
        description: description.to_string(),
        size_bytes: 0,
        inline,
        tags: String::new(),
        category: String::new(),
        jurisdiction: String::new(),
        project_id: None,
        created_at: String::new(),
    }
}

fn write_temp_file(dir: &PathBuf, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write temp file");
}

// ─── empty files list returns empty string ───────────────────────────────────

#[test]
fn test_empty_files_returns_empty_string() {
    let result = build_knowledge_section(&[], "/any/dir");
    assert!(result.is_empty());
}

// ─── non-inline file emits /knowledge/ reference path ────────────────────────

#[test]
fn test_non_inline_emits_reference_path() {
    let files = vec![kf("guide.md", "", false)];
    let result = build_knowledge_section(&files, "/irrelevant");
    assert!(result.contains("- `/knowledge/guide.md`"), "got: {result}");
    assert!(!result.contains("```"), "should not contain fenced block, got: {result}");
}

#[test]
fn test_non_inline_with_description_includes_description() {
    let files = vec![kf("terms.pdf", "Standard terms of service", false)];
    let result = build_knowledge_section(&files, "/irrelevant");
    assert!(result.contains("Standard terms of service"), "got: {result}");
    assert!(result.contains("- `/knowledge/terms.pdf`"), "got: {result}");
}

// ─── inline file with content emits fenced code block ────────────────────────

#[test]
fn test_inline_with_content_emits_fenced_block() {
    let dir = tempdir();
    write_temp_file(&dir, "rules.txt", "Rule 1: be precise.\nRule 2: be concise.");
    let files = vec![kf("rules.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("```"), "should contain fenced block, got: {result}");
    assert!(result.contains("Rule 1: be precise."), "got: {result}");
    assert!(result.contains("Rule 2: be concise."), "got: {result}");
    cleanup(&dir);
}

#[test]
fn test_inline_with_content_and_description_wraps_in_parens() {
    let dir = tempdir();
    write_temp_file(&dir, "policy.txt", "No warranties implied.");
    let files = vec![kf("policy.txt", "Legal policy document", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("(Legal policy document)"), "got: {result}");
    assert!(result.contains("No warranties implied."), "got: {result}");
}

// ─── inline file with empty/missing content falls back to listing only ────────

#[test]
fn test_inline_empty_file_falls_back_to_listing() {
    let dir = tempdir();
    write_temp_file(&dir, "empty.txt", "");
    let files = vec![kf("empty.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("**empty.txt**"), "got: {result}");
    assert!(!result.contains("```"), "should not contain fenced block, got: {result}");
}

#[test]
fn test_inline_whitespace_only_file_falls_back_to_listing() {
    let dir = tempdir();
    write_temp_file(&dir, "ws.txt", "   \n\t\n  ");
    let files = vec![kf("ws.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("**ws.txt**"), "got: {result}");
    assert!(!result.contains("```"), "should not contain fenced block, got: {result}");
}

#[test]
fn test_inline_missing_file_falls_back_to_listing() {
    let files = vec![kf("nonexistent.txt", "", true)];
    let result = build_knowledge_section(&files, "/does/not/exist");
    assert!(result.contains("**nonexistent.txt**"), "got: {result}");
    assert!(!result.contains("```"), "should not contain fenced block, got: {result}");
}

// ─── fallback listing with description ───────────────────────────────────────

#[test]
fn test_inline_empty_file_with_description_includes_it() {
    let dir = tempdir();
    write_temp_file(&dir, "stub.txt", "");
    let files = vec![kf("stub.txt", "A stub file for testing", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("A stub file for testing"), "got: {result}");
    assert!(!result.contains("```"), "should not contain fenced block, got: {result}");
}

// ─── header is always present when files list is non-empty ───────────────────

#[test]
fn test_header_present_for_non_empty_list() {
    let files = vec![kf("ref.md", "", false)];
    let result = build_knowledge_section(&files, "/dir");
    assert!(result.contains("## Knowledge Base"), "got: {result}");
    assert!(result.contains("/knowledge/"), "got: {result}");
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "borg-kb-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}
