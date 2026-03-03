use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single NDJSON message emitted by Claude Code (`--output-format stream-json`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// First message on stream: session initialisation.
    System(SystemEvent),

    /// An assistant turn (text or tool calls).
    Assistant(AssistantEvent),

    /// A user turn (tool results injected back into the conversation).
    User(UserEvent),

    /// Final result message — emitted once at the very end.
    Result(ResultEvent),

    /// Any message type not explicitly handled above.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemEvent {
    pub subtype: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssistantEvent {
    pub message: Option<AssistantMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssistantMessage {
    pub role: Option<String>,
    pub content: Option<Vec<ContentBlock>>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub usage: Option<Usage>,
}

/// A single content block inside an assistant or user message.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text output.
    Text { text: String },

    /// A tool invocation by the agent.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    /// Result returned by a tool (appears in user turn).
    ToolResult {
        tool_use_id: String,
        content: Option<Value>,
        is_error: Option<bool>,
    },

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserEvent {
    pub message: Option<UserMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserMessage {
    pub role: Option<String>,
    pub content: Option<Vec<ContentBlock>>,
}

/// Final result event, emitted once when the agent finishes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResultEvent {
    pub subtype: Option<String>,
    /// Textual output (may be empty if last turn was a tool call).
    pub result: Option<String>,
    pub session_id: Option<String>,
    pub is_error: Option<bool>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub num_turns: Option<u64>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

/// Parse a full NDJSON stream and extract the final output text and session ID.
pub fn parse_stream(data: &str) -> (String, Option<String>) {
    let mut output = String::new();
    let mut assistant_text = String::new();
    let mut session_id: Option<String> = None;

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }
        let event: AgentEvent = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        match event {
            AgentEvent::System(e) => {
                if let Some(sid) = e.session_id {
                    session_id = Some(sid);
                }
            },
            AgentEvent::Assistant(e) => {
                if let Some(msg) = e.message {
                    if let Some(blocks) = msg.content {
                        for block in blocks {
                            if let ContentBlock::Text { text } = block {
                                if !assistant_text.is_empty() {
                                    assistant_text.push('\n');
                                }
                                assistant_text.push_str(&text);
                            }
                        }
                    }
                }
            },
            AgentEvent::Result(e) => {
                if let Some(sid) = e.session_id {
                    session_id = Some(sid);
                }
                if let Some(text) = e.result {
                    output = text;
                }
            },
            _ => {},
        }
    }

    // Fall back to collected assistant text if result was empty
    if output.is_empty() && !assistant_text.is_empty() {
        output = assistant_text;
    }

    (output, session_id)
}

#[cfg(test)]
mod tests {
    use super::parse_stream;

    // session_id from System event
    #[test]
    fn test_session_id_from_system_event() {
        let input = r#"{"type":"system","subtype":"init","session_id":"sys-abc-123"}
{"type":"result","subtype":"success","result":"done","session_id":null}"#;
        let (_, sid) = parse_stream(input);
        assert_eq!(sid.as_deref(), Some("sys-abc-123"));
    }

    // session_id from Result event (no System event present)
    #[test]
    fn test_session_id_from_result_event() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}
{"type":"result","subtype":"success","result":"hi","session_id":"res-xyz-999"}"#;
        let (_, sid) = parse_stream(input);
        assert_eq!(sid.as_deref(), Some("res-xyz-999"));
    }

    // Result event session_id takes priority (overwrites System session_id)
    #[test]
    fn test_result_session_id_overwrites_system() {
        let input = r#"{"type":"system","session_id":"from-system"}
{"type":"result","result":"output","session_id":"from-result"}"#;
        let (_, sid) = parse_stream(input);
        assert_eq!(sid.as_deref(), Some("from-result"));
    }

    // Result event text takes priority over accumulated assistant text
    #[test]
    fn test_result_text_beats_assistant_text() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"assistant said this"}]}}
{"type":"result","result":"result text wins","session_id":"s1"}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "result text wins");
    }

    // Fallback to assistant text when result field is empty string
    #[test]
    fn test_fallback_when_result_is_empty_string() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"assistant fallback"}]}}
{"type":"result","result":"","session_id":"s2"}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "assistant fallback");
    }

    // Fallback to assistant text when result field is absent (null)
    #[test]
    fn test_fallback_when_result_is_null() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"only assistant"}]}}
{"type":"result","result":null,"session_id":"s3"}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "only assistant");
    }

    // Multiple assistant turns concatenated with newlines
    #[test]
    fn test_multiple_assistant_turns_concatenated() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"line one"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"line two"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"line three"}]}}
{"type":"result","result":"","session_id":null}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "line one\nline two\nline three");
    }

    // Multiple assistant turns: single turn has no separator
    #[test]
    fn test_single_assistant_turn_no_newline_prefix() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"only turn"}]}}
{"type":"result","result":null}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "only turn");
    }

    // Unknown/malformed lines are silently skipped
    #[test]
    fn test_malformed_lines_skipped() {
        let input = r#"not json at all
{"type":"assistant","message":{"content":[{"type":"text","text":"valid"}]}}
{broken json
{"type":"result","result":"final","session_id":"s4"}"#;
        let (output, sid) = parse_stream(input);
        assert_eq!(output, "final");
        assert_eq!(sid.as_deref(), Some("s4"));
    }

    // Unknown event types are silently skipped
    #[test]
    fn test_unknown_event_type_skipped() {
        let input = r#"{"type":"ping","data":"something"}
{"type":"result","result":"ok","session_id":"s5"}"#;
        let (output, sid) = parse_stream(input);
        assert_eq!(output, "ok");
        assert_eq!(sid.as_deref(), Some("s5"));
    }

    // Blank lines are skipped
    #[test]
    fn test_blank_lines_skipped() {
        let input = "\n\n{\"type\":\"result\",\"result\":\"clean\",\"session_id\":\"s6\"}\n\n";
        let (output, sid) = parse_stream(input);
        assert_eq!(output, "clean");
        assert_eq!(sid.as_deref(), Some("s6"));
    }

    // Fully empty input
    #[test]
    fn test_empty_input() {
        let (output, sid) = parse_stream("");
        assert_eq!(output, "");
        assert!(sid.is_none());
    }

    // All whitespace / blank lines only
    #[test]
    fn test_whitespace_only_input() {
        let (output, sid) = parse_stream("   \n\n  \n");
        assert_eq!(output, "");
        assert!(sid.is_none());
    }

    // No session_id fields present anywhere
    #[test]
    fn test_no_session_id_returns_none() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}
{"type":"result","result":"hi"}"#;
        let (_, sid) = parse_stream(input);
        assert!(sid.is_none());
    }

    // Assistant turn with multiple text blocks in one message
    #[test]
    fn test_multiple_text_blocks_in_one_assistant_turn() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"block A"},{"type":"text","text":"block B"}]}}
{"type":"result","result":null}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "block A\nblock B");
    }

    // Tool-use blocks in assistant turn are ignored (no text)
    #[test]
    fn test_tool_use_blocks_produce_no_text() {
        let input = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}
{"type":"result","result":null}"#;
        let (output, _) = parse_stream(input);
        assert_eq!(output, "");
    }
}
