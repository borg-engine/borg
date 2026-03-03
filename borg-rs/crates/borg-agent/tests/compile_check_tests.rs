use borg_agent::claude::derive_compile_check;

#[test]
fn rust_cargo_test_cmd_appends_no_run() {
    let result = derive_compile_check("cargo test");
    assert_eq!(result, Some("cargo test --no-run".to_string()));
}

#[test]
fn rust_cargo_test_cmd_with_flags_appends_no_run() {
    let result = derive_compile_check("cargo test --workspace");
    assert_eq!(result, Some("cargo test --workspace --no-run".to_string()));
}

#[test]
fn typescript_bun_test_cmd_returns_tsc() {
    let result = derive_compile_check("bun test");
    assert_eq!(result, Some("tsc --noEmit".to_string()));
}

#[test]
fn typescript_bun_run_test_cmd_returns_tsc() {
    let result = derive_compile_check("bun run test");
    assert_eq!(result, Some("tsc --noEmit".to_string()));
}

#[test]
fn unknown_cmd_returns_none() {
    let result = derive_compile_check("pytest");
    assert_eq!(result, None);
}

#[test]
fn empty_cmd_returns_none() {
    let result = derive_compile_check("");
    assert_eq!(result, None);
}

#[test]
fn whitespace_only_cmd_returns_none() {
    let result = derive_compile_check("   ");
    assert_eq!(result, None);
}
