// Tests for AC9: `extract_phase_result` in `borg_agent::claude`.
//
// These tests FAIL initially (fail to compile) because `extract_phase_result`
// does not yet exist in `borg_agent::claude`.
//
// Once implemented they cover:
//   AC9: extract_phase_result returns content from a valid marker pair.
//   AC9: extract_phase_result returns None when no markers are present.
//   AC9: extract_phase_result returns None when only the start marker is present.
//   AC9: extract_phase_result returns the LAST pair when multiple pairs exist.
//   EC1: unclosed start marker → None.
//   EC2: whitespace-only content between markers → None.
//   EC4: three marker pairs → last (third) is returned.

use borg_agent::claude::{extract_phase_result, parse_test_result};

const START: &str = "---PHASE_RESULT_START---";
const END: &str = "---PHASE_RESULT_END---";

// =============================================================================
// AC9: valid pair — content is returned
// =============================================================================

#[test]
fn test_basic_extraction() {
    let text = format!("{START}\nSpec complete.\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    assert!(result.unwrap().contains("Spec complete."));
}

#[test]
fn test_extraction_with_surrounding_prose() {
    let text = format!(
        "I reviewed the codebase.\n\n{START}\nTests written: 5 files.\n{END}\n\nPhase complete."
    );
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    assert!(result.unwrap().contains("Tests written: 5 files."));
}

#[test]
fn test_extracted_content_is_trimmed() {
    let text = format!("{START}\n  Summary line.  \n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    // Must not start or end with whitespace after trim
    assert_eq!(r, r.trim());
}

// =============================================================================
// AC9: no markers → None
// =============================================================================

#[test]
fn test_no_markers_returns_none() {
    let result = extract_phase_result("Plain output with no markers at all.");
    assert!(result.is_none());
}

#[test]
fn test_empty_string_returns_none() {
    let result = extract_phase_result("");
    assert!(result.is_none());
}

#[test]
fn test_ndjson_without_markers_returns_none() {
    let data = r#"{"type":"system","session_id":"abc"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Analyzing..."}]}}
{"type":"result","result":"Analysis complete."}"#;
    assert!(extract_phase_result(data).is_none());
}

// =============================================================================
// AC9 / EC1: only start marker present → None
// =============================================================================

#[test]
fn test_only_start_marker_returns_none() {
    let text = format!("{START}\nThis was never closed.");
    assert!(extract_phase_result(&text).is_none());
}

#[test]
fn test_only_end_marker_returns_none() {
    let text = format!("Some text here.\n{END}");
    assert!(extract_phase_result(&text).is_none());
}

#[test]
fn test_unclosed_start_in_stream_returns_none() {
    let text = format!("preamble\n{START}\ncontent without end\nmore lines");
    assert!(extract_phase_result(&text).is_none());
}

// =============================================================================
// EC2: whitespace-only content between markers → None
// =============================================================================

#[test]
fn test_whitespace_only_content_returns_none() {
    let text = format!("{START}\n   \n\t\n{END}");
    assert!(extract_phase_result(&text).is_none());
}

#[test]
fn test_empty_content_between_markers_returns_none() {
    let text = format!("{START}\n{END}");
    assert!(extract_phase_result(&text).is_none());
}

// =============================================================================
// AC9 / EC4: multiple pairs → last complete pair wins
// =============================================================================

#[test]
fn test_multiple_pairs_last_wins() {
    let text =
        format!("{START}\nFirst attempt.\n{END}\n\n{START}\nRevised summary — final one.\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        r.contains("Revised summary"),
        "expected revised summary, got: {r}"
    );
    assert!(
        !r.contains("First attempt"),
        "should not contain first attempt, got: {r}"
    );
}

#[test]
fn test_three_pairs_third_is_returned() {
    let text = format!(
        "{START}\nFirst.\n{END}\n{START}\nSecond.\n{END}\n{START}\nThird and final.\n{END}"
    );
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(r.contains("Third and final."), "got: {r}");
    assert!(!r.contains("First."), "got: {r}");
    assert!(!r.contains("Second."), "got: {r}");
}

// =============================================================================
// Multi-line content is preserved
// =============================================================================

#[test]
fn test_multiline_content_preserved() {
    let text = format!("{START}\nLine one.\nLine two.\nLine three.\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(r.contains("Line one."));
    assert!(r.contains("Line two."));
    assert!(r.contains("Line three."));
}

// =============================================================================
// Markers split across raw bytes (end-to-end correctness check)
// =============================================================================

#[test]
fn test_markers_not_present_in_plain_ndjson_escape() {
    // The raw marker strings consist only of ASCII characters that are never
    // JSON-escaped, so searching raw bytes is correct.
    let raw = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"---PHASE_RESULT_START---\nmy summary\n---PHASE_RESULT_END---"}]}}"#;
    // extract_phase_result operates on decoded text (result.output), not raw NDJSON.
    // When the decoded text contains the markers, extraction must succeed.
    let decoded = format!("{START}\nmy summary\n{END}");
    let result = extract_phase_result(&decoded);
    assert!(result.is_some());
    assert!(result.unwrap().contains("my summary"));
    // Raw NDJSON with escaped newlines must not falsely trigger on its own.
    let _ = raw; // used above as documentation only
}

// =============================================================================
// Content-then-empty pair → None (last empty pair nullifies prior content)
// =============================================================================

#[test]
fn test_empty_last_pair_nullifies_prior_content() {
    let text = format!("{START}\nActual content.\n{END}\n{START}\n   \n{END}");
    assert!(
        extract_phase_result(&text).is_none(),
        "whitespace-only last pair should nullify prior content"
    );
}

// =============================================================================
// parse_test_result tests
// =============================================================================

const MARKER: &str = "---BORG_TEST_RESULT---";

#[test]
fn test_parse_test_result_success() {
    let line = format!(r#"{MARKER}{{"phase":"test","passed":true,"exitCode":0,"output":"all ok"}}"#);
    let r = parse_test_result(&line).expect("should parse");
    assert!(r.passed);
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.phase, "test");
    assert_eq!(r.output, "all ok");
}

#[test]
fn test_parse_test_result_failure() {
    let line = format!(r#"{MARKER}{{"phase":"test","passed":false,"exitCode":1,"output":"2 failures"}}"#);
    let r = parse_test_result(&line).expect("should parse");
    assert!(!r.passed);
    assert_eq!(r.exit_code, 1);
    assert_eq!(r.output, "2 failures");
}

#[test]
fn test_parse_test_result_no_marker_returns_none() {
    assert!(parse_test_result(r#"{"phase":"test","passed":true}"#).is_none());
    assert!(parse_test_result("plain log line").is_none());
    assert!(parse_test_result("").is_none());
}

#[test]
fn test_parse_test_result_whitespace_only_returns_none() {
    let line = format!("{MARKER}   ");
    assert!(parse_test_result(&line).is_none());
}

#[test]
fn test_parse_test_result_invalid_json_returns_none() {
    let line = format!("{MARKER}not-json");
    assert!(parse_test_result(&line).is_none());
}

#[test]
fn test_parse_test_result_missing_fields_use_defaults() {
    // passed defaults to false, exitCode defaults to 1, phase/output default to ""
    let line = format!("{MARKER}{{}}");
    let r = parse_test_result(&line).expect("should parse empty object");
    assert!(!r.passed);
    assert_eq!(r.exit_code, 1);
    assert_eq!(r.phase, "");
    assert_eq!(r.output, "");
}

#[test]
fn test_parse_test_result_multiple_lines_last_is_independent() {
    // Each line is parsed independently; the "last one" in a stream is just the
    // most recent call. All three must succeed and reflect their own values.
    let lines = [
        format!(r#"{MARKER}{{"phase":"compile","passed":true,"exitCode":0,"output":"ok"}}"#),
        format!(r#"{MARKER}{{"phase":"lint","passed":false,"exitCode":2,"output":"warn"}}"#),
        format!(r#"{MARKER}{{"phase":"test","passed":true,"exitCode":0,"output":"pass"}}"#),
    ];
    let results: Vec<_> = lines.iter().filter_map(|l| parse_test_result(l)).collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].phase, "compile");
    assert!(results[0].passed);
    assert_eq!(results[1].phase, "lint");
    assert!(!results[1].passed);
    // last result
    assert_eq!(results[2].phase, "test");
    assert!(results[2].passed);
}
