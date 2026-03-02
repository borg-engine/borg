use borg_agent::instruction::build_knowledge_section;
use borg_core::db::KnowledgeFile;

fn make_file(id: i64, file_name: &str, description: &str, inline: bool) -> KnowledgeFile {
    KnowledgeFile {
        id,
        file_name: file_name.to_string(),
        description: description.to_string(),
        size_bytes: 0,
        inline,
        created_at: String::new(),
    }
}

// ── Empty slice ────────────────────────────────────────────────────────────

#[test]
fn test_empty_files_returns_empty_string() {
    assert!(build_knowledge_section(&[], "/any/dir").is_empty());
}

// ── Non-inline ─────────────────────────────────────────────────────────────

#[test]
fn test_non_inline_formats_as_knowledge_path_with_description() {
    let files = [make_file(1, "guide.md", "Usage guide", false)];
    let result = build_knowledge_section(&files, "/unused");
    assert!(result.contains("- `/knowledge/guide.md`: Usage guide\n"));
}

#[test]
fn test_non_inline_no_description_omits_colon() {
    let files = [make_file(1, "schema.sql", "", false)];
    let result = build_knowledge_section(&files, "/unused");
    assert!(result.contains("- `/knowledge/schema.sql`\n"));
    assert!(!result.contains("`schema.sql`:"));
}

// ── Inline with readable content ────────────────────────────────────────────

#[test]
fn test_inline_readable_content_embeds_fenced_block() {
    let dir = std::env::temp_dir().join(format!("borg_kb_{}_readable", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "Important content here").unwrap();

    let files = [make_file(1, "notes.txt", "Project notes", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());

    assert!(result.contains("- **notes.txt** (Project notes):\n```\nImportant content here\n```\n"));
}

#[test]
fn test_inline_readable_content_no_description() {
    let dir = std::env::temp_dir().join(format!("borg_kb_{}_nodesc", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("data.txt"), "some data").unwrap();

    let files = [make_file(1, "data.txt", "", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());

    assert!(result.contains("- **data.txt**:\n```\nsome data\n```\n"));
}

// ── Inline fallback: missing file ───────────────────────────────────────────

#[test]
fn test_inline_missing_file_falls_back_to_bullet() {
    let files = [make_file(1, "missing.txt", "Some description", true)];
    let result = build_knowledge_section(&files, "/nonexistent/path/12345");

    assert!(result.contains("- **missing.txt**: Some description\n"));
    assert!(!result.contains("```"));
}

// ── Inline fallback: empty file ─────────────────────────────────────────────

#[test]
fn test_inline_empty_file_falls_back_to_bullet() {
    let dir = std::env::temp_dir().join(format!("borg_kb_{}_empty", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("empty.txt"), "").unwrap();

    let files = [make_file(1, "empty.txt", "Empty file", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());

    assert!(result.contains("- **empty.txt**: Empty file\n"));
    assert!(!result.contains("```"));
}

// ── Header present ──────────────────────────────────────────────────────────

#[test]
fn test_header_present_when_files_non_empty() {
    let files = [make_file(1, "doc.md", "Documentation", false)];
    let result = build_knowledge_section(&files, "/unused");
    assert!(result.starts_with("## Knowledge Base\n"));
}
