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

use borg_agent::claude::extract_phase_result;

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
// EC-JSON: markers inside JSON-escaped strings (raw NDJSON) must not trigger
// =============================================================================

#[test]
fn test_json_escaped_markers_in_raw_ndjson_no_false_positive() {
    // Raw NDJSON where the model output embeds markers inside a JSON string value.
    // The `\n` sequences are 2-char JSON escapes (backslash + n), not actual newlines,
    // so the markers are NOT at the start of a line and must not be extracted.
    let raw = r#"{"type":"result","result":"---PHASE_RESULT_START---\nfake content\n---PHASE_RESULT_END---"}"#;
    assert!(
        extract_phase_result(raw).is_none(),
        "markers inside a JSON string value (JSON-escaped \\n) must not trigger extraction"
    );
}

#[test]
fn test_ndjson_assistant_message_with_embedded_markers_no_false_positive() {
    // Full NDJSON assistant event where model text discusses both markers.
    // Markers appear inside the JSON "text" string, not on their own lines.
    let raw = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"---PHASE_RESULT_START---\nmy summary\n---PHASE_RESULT_END---"}]}}"#;
    assert!(
        extract_phase_result(raw).is_none(),
        "markers inside assistant NDJSON string value must not trigger extraction"
    );
}

// =============================================================================
// EC-INLINE: markers mentioned mid-sentence must not trigger extraction
// =============================================================================

#[test]
fn test_markers_mentioned_inline_no_false_positive() {
    // Model discusses both marker names on the same line mid-sentence.
    // Neither marker is at the start of a line, so no extraction should occur.
    let text = format!("Use {START} before your result and {END} after it.");
    assert!(
        extract_phase_result(&text).is_none(),
        "markers mentioned mid-sentence on one line must not trigger extraction"
    );
}

#[test]
fn test_end_marker_mentioned_inline_after_valid_start_no_false_positive() {
    // START marker is on its own line, but END appears mid-sentence on the same
    // line as some content. The inline END must not terminate the block early;
    // the real END (on its own line) should close it.
    let text = format!(
        "{START}\nContent mentioning {END} inline; more content.\n{END}"
    );
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    // The inline END mention does not close the block early; all content is captured.
    let r = result.unwrap();
    assert!(
        r.contains("Content mentioning"),
        "content before inline END mention must be included, got: {r}"
    );
    assert!(
        r.contains("more content"),
        "content after inline END mention must be included, got: {r}"
    );
}

// =============================================================================
// EC-PARTIAL: partial start-marker substrings must not be confused with the
// full marker
// =============================================================================

#[test]
fn test_partial_start_marker_prefix_no_false_positive() {
    // Text contains the marker without its trailing dashes. This is a substring
    // of the real marker and must not trigger extraction; the valid pair that
    // follows on its own line should be returned normally.
    let partial = "---PHASE_RESULT_START"; // missing trailing ---
    let text = format!("{partial} mentioned here\n{START}\nreal content\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some(), "valid marker pair must still be found");
    assert_eq!(
        result.unwrap(),
        "real content",
        "only content from the valid pair should be returned"
    );
}

#[test]
fn test_partial_start_marker_inside_valid_content() {
    // Content between a valid pair of markers contains the start-marker prefix.
    // This must not confuse extraction; all content must be returned intact.
    let partial = "---PHASE_RESULT_START"; // missing trailing ---
    let text = format!("{START}\nContent with {partial} halfway.\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        r.contains(partial),
        "partial marker prefix inside content must be preserved, got: {r}"
    );
    assert!(
        r.contains("halfway"),
        "surrounding content must also be preserved, got: {r}"
    );
}

#[test]
fn test_partial_end_marker_inside_valid_content() {
    // Content contains the end-marker without trailing dashes.
    // This must not terminate the block early.
    let partial_end = "---PHASE_RESULT_END"; // missing trailing ---
    let text = format!("{START}\nContent with {partial_end} substring here.\n{END}");
    let result = extract_phase_result(&text);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        r.contains(partial_end),
        "partial end-marker inside content must be preserved, got: {r}"
    );
}
