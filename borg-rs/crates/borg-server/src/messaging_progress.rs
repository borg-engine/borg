use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use borg_agent::event::{AgentEvent, ContentBlock};
use borg_core::{sidecar::Sidecar, telegram::Telegram};
use chrono::Utc;
use rand::Rng;
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    sync::{broadcast, oneshot},
    task::JoinHandle,
};

const FLUSH_INTERVAL: Duration = Duration::from_secs(2);
const TYPING_INTERVAL: Duration = Duration::from_secs(5);
const START_NOTICE_DELAY: Duration = Duration::from_secs(2);
const MAX_LINES_PER_MESSAGE: usize = 4;

#[derive(Clone)]
pub(crate) enum MessagingProgressSink {
    Telegram {
        client: Arc<Telegram>,
        chat_id: i64,
        reply_to: Option<i64>,
    },
    Discord {
        sidecar: Arc<Sidecar>,
        chat_id: String,
        reply_to: Option<String>,
    },
    WhatsApp {
        sidecar: Arc<Sidecar>,
        chat_id: String,
        quote_id: Option<String>,
    },
    Slack {
        sidecar: Arc<Sidecar>,
        chat_id: String,
        reply_to: Option<String>,
    },
}

impl MessagingProgressSink {
    async fn send_message(&self, text: &str) {
        match self {
            Self::Telegram {
                client,
                chat_id,
                reply_to,
            } => {
                let _ = client.send_plain_message(*chat_id, text, *reply_to).await;
            },
            Self::Discord {
                sidecar,
                chat_id,
                reply_to,
            } => sidecar.send_discord(chat_id, text, reply_to.as_deref()),
            Self::WhatsApp {
                sidecar,
                chat_id,
                quote_id,
            } => sidecar.send_whatsapp(chat_id, text, quote_id.as_deref()),
            Self::Slack {
                sidecar,
                chat_id,
                reply_to,
            } => sidecar.send_slack(chat_id, text, reply_to.as_deref()),
        }
    }

    async fn send_typing(&self) {
        match self {
            Self::Telegram {
                client, chat_id, ..
            } => {
                let _ = client.send_typing(*chat_id).await;
            },
            Self::Discord {
                sidecar, chat_id, ..
            } => sidecar.send_discord_typing(chat_id),
            Self::WhatsApp {
                sidecar, chat_id, ..
            } => sidecar.send_whatsapp_typing(chat_id),
            Self::Slack {
                sidecar, chat_id, ..
            } => sidecar.send_slack_typing(chat_id),
        }
    }
}

pub(crate) struct ProgressForwarder {
    stop_tx: Option<oneshot::Sender<()>>,
    join: JoinHandle<()>,
}

impl ProgressForwarder {
    pub(crate) async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.join.await;
    }
}

#[derive(Deserialize)]
struct StreamEnvelope {
    #[serde(rename = "type")]
    event_type: Option<String>,
    thread: Option<String>,
    run_id: Option<String>,
    data: Option<String>,
}

pub(crate) fn new_chat_run_id() -> String {
    format!(
        "{}-{:016x}",
        Utc::now().timestamp_millis(),
        rand::thread_rng().gen::<u64>()
    )
}

pub(crate) fn spawn_chat_progress_forwarder(
    chat_event_tx: &broadcast::Sender<String>,
    thread: String,
    run_id: String,
    sink: MessagingProgressSink,
) -> ProgressForwarder {
    let mut rx = chat_event_tx.subscribe();
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let mut pending: Vec<String> = Vec::new();
        let mut started = false;
        let started_at = Instant::now();
        let mut flush_tick = tokio::time::interval(FLUSH_INTERVAL);
        let mut typing_tick = tokio::time::interval(TYPING_INTERVAL);
        let mut tool_names_by_id: HashMap<String, String> = HashMap::new();

        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    flush_pending(&sink, &mut pending, &mut started).await;
                    break;
                }
                _ = flush_tick.tick() => {
                    if !started && pending.is_empty() && started_at.elapsed() >= START_NOTICE_DELAY {
                        sink.send_message("Working on it. I'll post progress updates here as I go.").await;
                        started = true;
                    }
                    flush_pending(&sink, &mut pending, &mut started).await;
                }
                _ = typing_tick.tick() => {
                    sink.send_typing().await;
                }
                recv = rx.recv() => {
                    match recv {
                        Ok(payload) => {
                            let mut new_lines = parse_progress_lines(&payload, &thread, &run_id, &mut tool_names_by_id);
                            if !new_lines.is_empty() {
                                pending.append(&mut new_lines);
                                if pending.len() >= MAX_LINES_PER_MESSAGE {
                                    flush_pending(&sink, &mut pending, &mut started).await;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            if pending.is_empty() {
                                pending.push("Skipped some intermediate updates while catching up.".to_string());
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });

    ProgressForwarder {
        stop_tx: Some(stop_tx),
        join,
    }
}

async fn flush_pending(
    sink: &MessagingProgressSink,
    pending: &mut Vec<String>,
    started: &mut bool,
) {
    if pending.is_empty() {
        return;
    }
    let message = render_progress_message(pending, !*started);
    sink.send_message(&message).await;
    pending.clear();
    *started = true;
}

fn render_progress_message(lines: &[String], include_intro: bool) -> String {
    let mut out = String::new();
    if include_intro {
        out.push_str("Working on it.\n\n");
    }
    if lines.len() == 1 {
        out.push_str(&lines[0]);
        return out;
    }
    out.push_str("Progress update\n");
    for line in lines {
        out.push_str("- ");
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn parse_progress_lines(
    payload: &str,
    expected_thread: &str,
    expected_run_id: &str,
    tool_names_by_id: &mut HashMap<String, String>,
) -> Vec<String> {
    let envelope: StreamEnvelope = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    if envelope.event_type.as_deref() != Some("chat_stream") {
        return Vec::new();
    }
    if envelope.thread.as_deref() != Some(expected_thread) {
        return Vec::new();
    }
    if envelope.run_id.as_deref() != Some(expected_run_id) {
        return Vec::new();
    }
    let line = match envelope.data {
        Some(line) => line,
        None => return Vec::new(),
    };
    let event: AgentEvent = match serde_json::from_str(&line) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    match event {
        AgentEvent::Assistant(event) => event
            .message
            .and_then(|message| message.content)
            .into_iter()
            .flatten()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    tool_names_by_id.insert(id, name.clone());
                    Some(format_tool_line(&name, &input))
                },
                _ => None,
            })
            .collect(),
        AgentEvent::User(event) => event
            .message
            .and_then(|message| message.content)
            .into_iter()
            .flatten()
            .filter_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let tool_name = tool_names_by_id.get(&tool_use_id)?;
                    summarize_tool_result(tool_name, content.as_ref(), is_error.unwrap_or(false))
                },
                _ => None,
            })
            .collect(),
        AgentEvent::Result(event) => {
            if event.result.as_deref().unwrap_or("").trim().is_empty() {
                Vec::new()
            } else {
                vec!["Finalizing answer.".to_string()]
            }
        },
        _ => Vec::new(),
    }
}

fn format_tool_line(tool: &str, input: &Value) -> String {
    let label = tool_display_name(tool);
    let (summary, detail) = format_tool_input(tool, input);
    match (summary.is_empty(), detail.is_empty()) {
        (false, false) => format!("{label}: {summary} | {detail}"),
        (false, true) => format!("{label}: {summary}"),
        (true, false) => format!("{label}: {detail}"),
        (true, true) => label,
    }
}

fn tool_display_name(tool: &str) -> String {
    match tool {
        "Read" => "Read file".to_string(),
        "Write" => "Created file".to_string(),
        "Edit" => "Edited file".to_string(),
        "Bash" => "Ran command".to_string(),
        "Grep" => "Searched for".to_string(),
        "Glob" => "Found files".to_string(),
        "WebFetch" | "web_fetch" => "Fetched page".to_string(),
        "WebSearch" | "web_search" => "Searched web".to_string(),
        "ToolSearch" => "Tool search".to_string(),
        "Task" => "Created task".to_string(),
        "Agent" => "Sub-agent".to_string(),
        "mcp__borg__search_documents" | "search_documents" => "BorgSearch".to_string(),
        "mcp__borg__list_documents" | "list_documents" => "BorgSearch list".to_string(),
        "mcp__borg__read_document" | "read_document" => "BorgSearch read".to_string(),
        "mcp__borg__check_coverage" | "check_coverage" => "BorgSearch coverage".to_string(),
        "mcp__borg__get_document_categories" | "get_document_categories" => {
            "BorgSearch categories".to_string()
        },
        "mcp__borg__create_task" | "create_task" => "Borg task".to_string(),
        "mcp__borg__get_task_status" | "get_task_status" => "Task status".to_string(),
        "mcp__borg__list_project_tasks" | "list_project_tasks" => "Project tasks".to_string(),
        "mcp__borg__list_services" | "list_services" => "Tool inventory".to_string(),
        other if other.starts_with("mcp__") => format_unknown_mcp_tool_name(other),
        other => other.to_string(),
    }
}

fn format_unknown_mcp_tool_name(tool: &str) -> String {
    let Some(rest) = tool.strip_prefix("mcp__") else {
        return tool.to_string();
    };
    let mut parts = rest.splitn(2, "__");
    let server = parts.next().unwrap_or_default();
    let action = parts.next().unwrap_or_default();
    let server_label = match server {
        "borg" => "BorgSearch",
        "lawborg" => "LawBorg",
        other => other,
    };
    if action.is_empty() {
        return server_label.to_string();
    }
    format!("{server_label} {}", action.replace('_', " "))
}

fn format_tool_input(tool: &str, input: &Value) -> (String, String) {
    let Some(obj) = input.as_object() else {
        return (String::new(), scalar_to_string(input));
    };

    match tool {
        "Bash" => (
            string_field(obj, "description"),
            truncate(string_field(obj, "command"), 180),
        ),
        "Read" => {
            let file_path = string_field(obj, "file_path");
            let offset = obj.get("offset").and_then(Value::as_i64);
            let limit = obj.get("limit").and_then(Value::as_i64).unwrap_or(200);
            let suffix = offset
                .map(|start| format!("lines {}-{}", start, start + limit))
                .unwrap_or_default();
            let detail = if suffix.is_empty() {
                String::new()
            } else {
                suffix
            };
            (file_path, detail)
        },
        "Write" => (string_field(obj, "file_path"), String::new()),
        "Edit" => (
            string_field(obj, "file_path"),
            truncate(
                format_old_string(obj.get("old_string").and_then(Value::as_str)),
                180,
            ),
        ),
        "Glob" | "Grep" => (string_field(obj, "pattern"), string_field(obj, "path")),
        "WebFetch" | "web_fetch" => (string_field(obj, "url"), String::new()),
        "Task" | "Agent" => (
            string_field(obj, "description"),
            truncate(string_field(obj, "prompt"), 180),
        ),
        _ if is_search_tool(tool) => {
            let queries = collect_search_queries(input);
            if !queries.is_empty() {
                (truncate(queries.join(" | "), 160), String::new())
            } else {
                summarize_object_input(obj)
            }
        },
        _ if tool.starts_with("mcp__") => {
            let query = non_empty_first(&[
                string_field(obj, "query"),
                string_field(obj, "q"),
                string_field(obj, "name"),
                string_field(obj, "document_id"),
                string_field(obj, "file_id"),
                string_field(obj, "id"),
            ]);
            if !query.is_empty() {
                (truncate(query, 160), String::new())
            } else {
                summarize_object_input(obj)
            }
        },
        _ => summarize_object_input(obj),
    }
}

fn summarize_tool_result(tool: &str, content: Option<&Value>, is_error: bool) -> Option<String> {
    let display = tool_display_name(tool);
    let text = tool_result_text(content)?;
    let summary = if is_error {
        truncate(text.replace('\n', " "), 180)
    } else if matches!(tool, "mcp__borg__search_documents" | "search_documents") {
        summarize_search_result(&text)
            .unwrap_or_else(|| generic_result_summary(&text).unwrap_or_else(|| "done".to_string()))
    } else if matches!(tool, "mcp__borg__list_documents" | "list_documents") {
        summarize_list_documents_result(&text)
            .unwrap_or_else(|| generic_result_summary(&text).unwrap_or_else(|| "done".to_string()))
    } else if matches!(tool, "mcp__borg__check_coverage" | "check_coverage") {
        summarize_coverage_result(&text)
            .unwrap_or_else(|| generic_result_summary(&text).unwrap_or_else(|| "done".to_string()))
    } else if matches!(tool, "mcp__borg__read_document" | "read_document") {
        summarize_read_result(&text)
            .unwrap_or_else(|| generic_result_summary(&text).unwrap_or_else(|| "done".to_string()))
    } else {
        generic_result_summary(&text)?
    };
    let prefix = if is_error {
        format!("{display} error")
    } else {
        format!("{display} result")
    };
    Some(format!("{prefix}: {summary}"))
}

fn tool_result_text(content: Option<&Value>) -> Option<String> {
    let value = content?;
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(|text| text.to_string())
                        .or_else(|| item.as_str().map(|text| text.to_string()))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        },
        _ => Some(value.to_string()),
    }
}

fn summarize_search_result(text: &str) -> Option<String> {
    if text.contains("\"status\": \"no_project_corpus\"") {
        return Some("no project corpus attached".to_string());
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(items) = value.as_array() {
            return Some(match items.len() {
                0 => "no results".to_string(),
                1 => "1 result".to_string(),
                n => format!("{n} results"),
            });
        }
    }
    generic_result_summary(text)
}

fn summarize_list_documents_result(text: &str) -> Option<String> {
    if text.contains("\"status\": \"no_project_corpus\"") {
        return Some("no project corpus attached".to_string());
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(items) = value.as_array() {
            return Some(match items.len() {
                0 => "no documents".to_string(),
                1 => "1 document listed".to_string(),
                n => format!("{n} documents listed"),
            });
        }
        if let Some(total) = value.get("total").and_then(Value::as_i64) {
            return Some(format!("{total} documents"));
        }
    }
    let lower = text.to_lowercase();
    if lower.contains("no files found") || lower.contains("no documents") {
        return Some("no documents".to_string());
    }
    generic_result_summary(text)
}

fn summarize_coverage_result(text: &str) -> Option<String> {
    if text.contains("\"status\": \"no_project_corpus\"") {
        return Some("no project corpus attached".to_string());
    }
    let matched = capture_json_i64(text, "Matched").or_else(|| capture_json_i64(text, "matched"));
    let unmatched =
        capture_json_i64(text, "Not matched").or_else(|| capture_json_i64(text, "unmatched"));
    if let (Some(matched), Some(unmatched)) = (matched, unmatched) {
        return Some(format!("{matched} matched, {unmatched} unmatched"));
    }
    if let Some(summary) = parse_coverage_report_text(text) {
        return Some(summary);
    }
    generic_result_summary(text)
}

fn summarize_read_result(text: &str) -> Option<String> {
    if text.contains("\"status\": \"no_project_corpus\"") {
        return Some("no project corpus attached".to_string());
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(name) = value
            .get("file_name")
            .or_else(|| value.get("name"))
            .and_then(Value::as_str)
        {
            return Some(format!("opened {}", truncate(name.to_string(), 120)));
        }
    }
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("File: ") {
            return Some(format!("opened {}", truncate(name.trim().to_string(), 120)));
        }
    }
    generic_result_summary(text)
}

fn capture_json_i64(text: &str, key: &str) -> Option<i64> {
    if !text.trim_start().starts_with('{') && !text.trim_start().starts_with('[') {
        return None;
    }
    let value = serde_json::from_str::<Value>(text).ok()?;
    match value {
        Value::Object(obj) => obj.get(key).and_then(Value::as_i64),
        _ => None,
    }
}

fn parse_coverage_report_text(text: &str) -> Option<String> {
    let mut matched = None;
    let mut unmatched = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Matched: ") {
            matched = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<i64>().ok());
        } else if let Some(rest) = line.strip_prefix("Not matched: ") {
            unmatched = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<i64>().ok());
        }
    }
    match (matched, unmatched) {
        (Some(matched), Some(unmatched)) => {
            Some(format!("{matched} matched, {unmatched} unmatched"))
        },
        _ => None,
    }
}

fn generic_result_summary(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lines = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if lines <= 1 {
        return Some(truncate(trimmed.replace('\n', " "), 160));
    }
    Some(format!("{lines} lines"))
}

fn format_old_string(old: Option<&str>) -> String {
    match old {
        Some(value) if !value.is_empty() => format!("replacing: {}", value),
        _ => String::new(),
    }
}

fn summarize_object_input(input: &serde_json::Map<String, Value>) -> (String, String) {
    let priority_keys = [
        "description",
        "recipient_name",
        "name",
        "title",
        "query",
        "q",
        "url",
        "command",
        "pattern",
        "path",
        "file_path",
        "prompt",
    ];

    for key in priority_keys {
        if let Some(value) = input.get(key) {
            let text = format_key_value(value).trim().to_string();
            if !text.is_empty() {
                let mut remaining = input.clone();
                remaining.remove(key);
                let detail = if remaining.is_empty() {
                    String::new()
                } else {
                    truncate(serde_json::to_string(&remaining).unwrap_or_default(), 200)
                };
                return (truncate(text, 160), detail);
            }
        }
    }

    let mut entries = Vec::new();
    for (key, value) in input {
        let rendered = format_key_value(value).trim().to_string();
        if !rendered.is_empty() {
            entries.push(format!("{key}: {rendered}"));
        }
    }
    if let Some(first) = entries.first() {
        let detail = if entries.len() > 1 {
            truncate(entries[1..].join("  "), 200)
        } else {
            String::new()
        };
        return (truncate(first.clone(), 160), detail);
    }

    (
        String::new(),
        truncate(serde_json::to_string(input).unwrap_or_default(), 200),
    )
}

fn format_key_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(format_key_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(obj) => {
            let queries = collect_search_queries(value);
            if !queries.is_empty() {
                return queries.join(" | ");
            }
            let scalar_entries = obj
                .iter()
                .filter_map(|(key, item)| {
                    let rendered = scalar_to_string(item);
                    if rendered.is_empty() {
                        None
                    } else {
                        Some(format!("{key}: {rendered}"))
                    }
                })
                .take(2)
                .collect::<Vec<_>>();
            scalar_entries.join("  ")
        },
        _ => String::new(),
    }
}

fn collect_search_queries(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed.to_string()]
            }
        },
        Value::Array(items) => items.iter().flat_map(collect_search_queries).collect(),
        Value::Object(obj) => {
            for key in ["query", "q", "term", "text"] {
                let direct = string_field(obj, key);
                if !direct.is_empty() {
                    return vec![direct];
                }
            }
            for key in ["search_query", "queries", "requests", "items", "payload"] {
                if let Some(nested) = obj.get(key) {
                    let queries = collect_search_queries(nested);
                    if !queries.is_empty() {
                        return queries;
                    }
                }
            }
            Vec::new()
        },
        _ => Vec::new(),
    }
}

fn string_field(obj: &serde_json::Map<String, Value>, key: &str) -> String {
    obj.get(key).map(scalar_to_string).unwrap_or_default()
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        _ => String::new(),
    }
}

fn non_empty_first(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn is_search_tool(tool: &str) -> bool {
    matches!(tool, "WebSearch" | "ToolSearch" | "web_search")
        || matches!(tool, "mcp__borg__search_documents" | "search_documents")
}

fn truncate(value: String, max: usize) -> String {
    if value.chars().count() <= max {
        return value;
    }
    let mut truncated = value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_borgsearch_tool_use() {
        let data = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "mcp__borg__search_documents",
                    "input": {
                        "query": "indemnification clause",
                        "project_id": 42
                    }
                }]
            }
        })
        .to_string();
        let line = serde_json::json!({
            "type": "chat_stream",
            "thread": "telegram:1",
            "run_id": "run-1",
            "data": data,
        })
        .to_string();
        let mut tool_names_by_id = HashMap::new();
        let lines = parse_progress_lines(&line, "telegram:1", "run-1", &mut tool_names_by_id);
        assert_eq!(lines, vec!["BorgSearch: indemnification clause"]);
        assert_eq!(
            tool_names_by_id.get("toolu_1").map(String::as_str),
            Some("mcp__borg__search_documents")
        );
    }

    #[test]
    fn summarizes_coverage_tool_result() {
        let data = serde_json::json!({
            "type": "user",
            "message": {
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_1",
                    "content": [{
                        "type": "text",
                        "text": "## Coverage Report: \"indemnification\"\n\nTotal documents: 10\nMatched: 6 (60%)\nNot matched: 4\n"
                    }]
                }]
            }
        })
        .to_string();
        let line = serde_json::json!({
            "type": "chat_stream",
            "thread": "telegram:1",
            "run_id": "run-1",
            "data": data,
        })
        .to_string();
        let mut tool_names_by_id = HashMap::new();
        tool_names_by_id.insert(
            "toolu_1".to_string(),
            "mcp__borg__check_coverage".to_string(),
        );
        let lines = parse_progress_lines(&line, "telegram:1", "run-1", &mut tool_names_by_id);
        assert_eq!(
            lines,
            vec!["BorgSearch coverage result: 6 matched, 4 unmatched"]
        );
    }
}
