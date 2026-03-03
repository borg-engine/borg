use borg_agent::instruction::build_knowledge_section;
use borg_core::db::KnowledgeFile;
use std::path::PathBuf;

fn kf(id: i64, file_name: &str, description: &str, inline: bool) -> KnowledgeFile {
    KnowledgeFile {
        id,
        file_name: file_name.to_string(),
        description: description.to_string(),
        size_bytes: 0,
        inline,
        created_at: String::new(),
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("borg_ks_{}", label));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// =============================================================================
// Empty input
// =============================================================================

#[test]
fn empty_files_returns_empty_string() {
    assert!(build_knowledge_section(&[], "/any/path").is_empty());
}

// =============================================================================
// Reference (non-inline) files
// =============================================================================

#[test]
fn reference_file_no_description() {
    let files = vec![kf(1, "guide.md", "", false)];
    let result = build_knowledge_section(&files, "/any");
    assert!(result.contains("- `/knowledge/guide.md`\n"));
    assert!(!result.contains(": "));
}

#[test]
fn reference_file_with_description() {
    let files = vec![kf(1, "guide.md", "User guide", false)];
    let result = build_knowledge_section(&files, "/any");
    assert!(result.contains("- `/knowledge/guide.md`: User guide\n"));
}

// =============================================================================
// Inline files — file has content
// =============================================================================

#[test]
fn inline_file_with_content_no_description() {
    let dir = temp_dir("inline_no_desc");
    std::fs::write(dir.join("facts.txt"), "Some fact here").unwrap();

    let files = vec![kf(1, "facts.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("- **facts.txt**:\n```\nSome fact here\n```\n"));
    // No parenthesised description section
    assert!(!result.contains("()"));
}

#[test]
fn inline_file_with_content_and_description() {
    let dir = temp_dir("inline_with_desc");
    std::fs::write(dir.join("facts.txt"), "Some fact here").unwrap();

    let files = vec![kf(1, "facts.txt", "Factual info", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("- **facts.txt** (Factual info):\n```\nSome fact here\n```\n"));
}

// =============================================================================
// Inline files — empty / whitespace-only content falls back to name-only
// =============================================================================

#[test]
fn inline_empty_file_no_description_falls_back_to_name_only() {
    let dir = temp_dir("inline_empty_no_desc");
    std::fs::write(dir.join("empty.txt"), "").unwrap();

    let files = vec![kf(1, "empty.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("- **empty.txt**\n"));
    assert!(!result.contains("```"));
}

#[test]
fn inline_whitespace_only_file_with_description_falls_back() {
    let dir = temp_dir("inline_empty_with_desc");
    std::fs::write(dir.join("empty.txt"), "   \n  ").unwrap();

    let files = vec![kf(1, "empty.txt", "Should be listed", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("- **empty.txt**: Should be listed\n"));
    assert!(!result.contains("```"));
}

#[test]
fn inline_missing_file_falls_back_to_name_only() {
    // read_to_string returns empty string on error → same empty-content path
    let files = vec![kf(1, "nonexistent.txt", "", true)];
    let result = build_knowledge_section(&files, "/nonexistent/path");
    assert!(result.contains("- **nonexistent.txt**\n"));
    assert!(!result.contains("```"));
}

// =============================================================================
// Multiple files
// =============================================================================

#[test]
fn multiple_files_produce_correct_multi_entry_output() {
    let dir = temp_dir("multi");
    std::fs::write(dir.join("a.txt"), "Content A").unwrap();

    let files = vec![
        kf(1, "a.txt", "File A", true),
        kf(2, "b.md", "File B", false),
    ];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.starts_with("## Knowledge Base\n"));
    assert!(result.contains("- **a.txt** (File A):\n```\nContent A\n```\n"));
    assert!(result.contains("- `/knowledge/b.md`: File B\n"));
}

#[test]
fn multiple_reference_files_all_listed() {
    let files = vec![
        kf(1, "alpha.md", "", false),
        kf(2, "beta.md", "Beta doc", false),
        kf(3, "gamma.md", "Gamma doc", false),
    ];
    let result = build_knowledge_section(&files, "/any");
    assert!(result.contains("- `/knowledge/alpha.md`\n"));
    assert!(result.contains("- `/knowledge/beta.md`: Beta doc\n"));
    assert!(result.contains("- `/knowledge/gamma.md`: Gamma doc\n"));
}

// =============================================================================
// Path traversal prevention
// =============================================================================

#[test]
fn inline_traversal_dotdot_does_not_read_outside_knowledge_dir() {
    // Create a sentinel file one level above a temp knowledge dir
    let base = temp_dir("traversal_dotdot");
    let knowledge_dir = base.join("knowledge");
    std::fs::create_dir_all(&knowledge_dir).unwrap();
    // Place secret content outside the knowledge dir
    std::fs::write(base.join("secret.txt"), "SECRET CONTENT").unwrap();

    // file_name contains traversal; should be stripped to "secret.txt" within knowledge_dir
    // (which doesn't exist there), so content must be empty / name-only output
    let files = vec![kf(1, "../secret.txt", "", true)];
    let result = build_knowledge_section(&files, knowledge_dir.to_str().unwrap());
    assert!(!result.contains("SECRET CONTENT"), "traversal read sensitive file");
}

#[test]
fn inline_traversal_absolute_path_does_not_escape() {
    let dir = temp_dir("traversal_abs");

    // Absolute path as file_name — after stripping should just be "passwd"
    // which won't exist in our temp dir so content is empty.
    let files = vec![kf(1, "/etc/passwd", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    // Must not contain real /etc/passwd content (root: or similar)
    assert!(!result.contains("root:"), "read /etc/passwd via absolute file_name");
}

#[test]
fn inline_traversal_subdir_stripped_to_basename() {
    let dir = temp_dir("traversal_subdir");
    // Place file.txt directly in dir (not in a sub-folder)
    std::fs::write(dir.join("file.txt"), "Safe content").unwrap();

    // subdir/file.txt should be stripped to file.txt and read correctly
    let files = vec![kf(1, "subdir/file.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(result.contains("Safe content"), "basename file should be readable");
}

#[test]
fn inline_traversal_long_chain_stripped() {
    let dir = temp_dir("traversal_chain");

    let files = vec![kf(1, "../../../../etc/shadow", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());
    assert!(!result.contains("root"), "long traversal chain must not escape");
}
