use borg_core::db::Db;

fn open_db() -> Db {
    let mut db = Db::open(":memory:").expect("open in-memory db");
    db.migrate().expect("migrate");
    db
}

#[test]
fn insert_knowledge_file_stores_stored_path() {
    let db = open_db();
    let id = db
        .insert_knowledge_file("report.pdf", "/data/knowledge/123_abc_report.pdf", "a report", 1024, false)
        .expect("insert");
    let file = db.get_knowledge_file(id).expect("get").expect("exists");
    assert_eq!(file.file_name, "report.pdf");
    assert_eq!(file.stored_path, "/data/knowledge/123_abc_report.pdf");
}

#[test]
fn two_uploads_same_name_produce_distinct_stored_paths() {
    let db = open_db();
    let id1 = db
        .insert_knowledge_file("doc.txt", "/knowledge/111_aaa_doc.txt", "", 10, false)
        .expect("insert 1");
    let id2 = db
        .insert_knowledge_file("doc.txt", "/knowledge/222_bbb_doc.txt", "", 10, false)
        .expect("insert 2");

    let f1 = db.get_knowledge_file(id1).expect("get 1").expect("exists");
    let f2 = db.get_knowledge_file(id2).expect("get 2").expect("exists");

    assert_eq!(f1.file_name, f2.file_name, "display names match");
    assert_ne!(f1.stored_path, f2.stored_path, "stored paths must differ");
}

#[test]
fn list_knowledge_files_includes_stored_path() {
    let db = open_db();
    db.insert_knowledge_file("a.md", "/k/ts1_x_a.md", "", 5, true).expect("insert");
    let files = db.list_knowledge_files().expect("list");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].stored_path, "/k/ts1_x_a.md");
    assert_eq!(files[0].file_name, "a.md");
}

#[test]
fn backwards_compat_empty_stored_path() {
    // Rows inserted with empty stored_path (legacy) should still deserialize.
    let db = open_db();
    {
        let conn = db.raw_conn().lock().unwrap();
        conn.execute(
            "INSERT INTO knowledge_files (file_name, description, size_bytes, inline) VALUES ('old.txt','',42,0)",
            [],
        )
        .unwrap();
    }
    let files = db.list_knowledge_files().expect("list");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name, "old.txt");
    assert!(files[0].stored_path.is_empty(), "legacy row has empty stored_path");
}
