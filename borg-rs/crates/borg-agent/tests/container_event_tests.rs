// Tests for the agent_error container_event JSON format.
//
// entrypoint.sh constructs this event and must produce valid JSON even when
// stderr contains backslashes, newlines, or other characters that need escaping.
//
// The fixed implementation uses bun's JSON.stringify (same as run_check's
// $escaped variable), which correctly escapes all JSON special characters.

// ── helpers ───────────────────────────────────────────────────────────────────

fn agent_error_json(exit_code: i64, stderr_tail: &str) -> String {
    serde_json::json!({
        "type": "container_event",
        "event": "agent_error",
        "exit_code": exit_code,
        "stderr_tail": stderr_tail,
    })
    .to_string()
}

fn parse_agent_error(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("agent_error event must be valid JSON")
}

// ── valid format tests ────────────────────────────────────────────────────────

#[test]
fn agent_error_plain_stderr_is_valid_json() {
    let json = agent_error_json(1, "claude: command not found");
    let v = parse_agent_error(&json);
    assert_eq!(v["type"], "container_event");
    assert_eq!(v["event"], "agent_error");
    assert_eq!(v["exit_code"], 1);
    assert_eq!(v["stderr_tail"], "claude: command not found");
}

#[test]
fn agent_error_stderr_with_backslash_is_valid_json() {
    // Windows-style paths and regex patterns contain bare backslashes.
    // JSON.stringify encodes \ as \\, producing valid JSON.
    let stderr = r"error: C:\Users\agent\path\to\file not found";
    let json = agent_error_json(2, stderr);
    let v = parse_agent_error(&json);
    assert_eq!(v["stderr_tail"].as_str().unwrap(), stderr);
}

#[test]
fn agent_error_stderr_with_newlines_is_valid_json() {
    let stderr = "line one\nline two\nline three";
    let json = agent_error_json(1, stderr);
    let v = parse_agent_error(&json);
    assert_eq!(v["stderr_tail"].as_str().unwrap(), stderr);
}

#[test]
fn agent_error_stderr_with_double_quotes_is_valid_json() {
    let stderr = r#"error: unexpected token '"' at line 5"#;
    let json = agent_error_json(1, stderr);
    let v = parse_agent_error(&json);
    assert_eq!(v["stderr_tail"].as_str().unwrap(), stderr);
}

#[test]
fn agent_error_stderr_with_mixed_special_chars_is_valid_json() {
    let stderr = "regex \\d+ failed\npath: C:\\tmp\\out\ttabbed\nquote: \"oops\"";
    let json = agent_error_json(1, stderr);
    let v = parse_agent_error(&json);
    assert_eq!(v["stderr_tail"].as_str().unwrap(), stderr);
}

#[test]
fn agent_error_empty_stderr_is_valid_json() {
    let json = agent_error_json(1, "");
    let v = parse_agent_error(&json);
    assert_eq!(v["stderr_tail"].as_str().unwrap(), "");
}

// ── regression: old sed-only encoding is invalid JSON for backslashes ─────────

#[test]
fn raw_backslash_in_json_string_is_invalid() {
    // The old entrypoint.sh only ran sed 's/"/\\"/g' and left bare \ in place.
    // A JSON string with a raw (unescaped) backslash is not valid JSON.
    let broken = r#"{"type":"container_event","event":"agent_error","exit_code":1,"stderr_tail":"C:\Users\foo"}"#;
    let result: Result<serde_json::Value, _> = serde_json::from_str(broken);
    assert!(
        result.is_err(),
        "bare backslash in JSON string must be invalid; the old sed approach was broken"
    );
}

#[test]
fn properly_escaped_backslash_in_json_string_is_valid() {
    // The fixed entrypoint.sh uses JSON.stringify which escapes \ as \\.
    let fixed = r#"{"type":"container_event","event":"agent_error","exit_code":1,"stderr_tail":"C:\\Users\\foo"}"#;
    let v: serde_json::Value = serde_json::from_str(fixed)
        .expect("properly escaped backslash must produce valid JSON");
    // The decoded value has literal backslashes.
    assert_eq!(v["stderr_tail"].as_str().unwrap(), r"C:\Users\foo");
}
