use borg_agent::instruction::build_knowledge_section;
use borg_core::db::KnowledgeFile;

fn kf(file_name: &str, description: &str, inline: bool) -> KnowledgeFile {
    KnowledgeFile {
        id: 1,
        file_name: file_name.to_string(),
        description: description.to_string(),
        size_bytes: 0,
        inline,
        created_at: String::new(),
    }
}

#[test]
fn test_empty_files_returns_empty_string() {
    assert_eq!(build_knowledge_section(&[], "/any/dir"), "");
}

#[test]
fn test_non_inline_produces_reference_line() {
    let files = [kf("guide.md", "", false)];
    let result = build_knowledge_section(&files, "/irrelevant");
    assert!(result.contains("- `/knowledge/guide.md`\n"), "got: {result}");
}

#[test]
fn test_non_inline_with_description_appends_it() {
    let files = [kf("guide.md", "Developer guide", false)];
    let result = build_knowledge_section(&files, "/irrelevant");
    assert!(
        result.contains("- `/knowledge/guide.md`: Developer guide\n"),
        "got: {result}"
    );
}

#[test]
fn test_inline_unreadable_path_falls_back_to_name() {
    let files = [kf("notes.md", "", true)];
    let result = build_knowledge_section(&files, "/nonexistent/borg_test_dir");
    assert!(result.contains("- **notes.md**\n"), "got: {result}");
    assert!(!result.contains("```"), "should not embed code fence, got: {result}");
}

#[test]
fn test_inline_readable_content_embedded_in_code_fence() {
    let dir = std::env::temp_dir();
    let file_name = "borg_instruction_test_inline.md";
    let file_path = dir.join(file_name);
    std::fs::write(&file_path, "Inline knowledge content").unwrap();

    let files = [kf(file_name, "Reference material", true)];
    let result = build_knowledge_section(&files, dir.to_str().unwrap());

    let _ = std::fs::remove_file(&file_path);

    assert!(
        result.contains(&format!(
            "- **{file_name}** (Reference material):\n```\nInline knowledge content\n```\n"
        )),
        "got: {result}"
    );
}
