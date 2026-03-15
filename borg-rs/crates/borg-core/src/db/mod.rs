mod cron;
mod knowledge;
mod misc;
mod projects;
mod tasks;
mod workspaces;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json;

use crate::{
    linked_credentials::LinkedCredentialBundle,
    pgcompat as pg,
    pgcompat::{params, Connection, ConnectionGuard, Mutex, OptionalExtension},
    types::{Proposal, QueueEntry, Task},
};

const SCHEMA_SQL: &str = include_str!("../../../../schema.pg.sql");

pub struct Db {
    conn: Mutex<Connection>,
}

// ── Auxiliary types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProjectTaskCounts {
    pub active: i64,
    pub review: i64,
    pub done: i64,
    pub failed: i64,
    pub total: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskOutput {
    pub id: i64,
    pub task_id: i64,
    pub phase: String,
    pub output: String,
    pub raw_stream: String,
    pub exit_code: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskMessage {
    pub id: i64,
    pub task_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub delivered_phase: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoRow {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub mode: String,
    pub backend: Option<String>,
    pub test_cmd: String,
    pub prompt_file: String,
    pub auto_merge: bool,
    pub repo_slug: String,
}

#[derive(serde::Serialize)]
pub struct LegacyEvent {
    pub id: i64,
    pub ts: i64,
    pub level: String,
    pub category: String,
    pub message: String,
    pub metadata: String,
}

#[derive(serde::Serialize)]
pub struct ChatMessage {
    pub id: String,
    pub chat_jid: String,
    pub sender: Option<String>,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub is_from_me: bool,
    pub is_bot_message: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_stream: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ApiKeyEntry {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<i64>,
    pub owner: String,
    pub provider: String,
    pub key_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomMcpServerRow {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub label: String,
    pub command: String,
    pub args_json: String,
    pub env_keys: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LinkedCredentialEntry {
    pub id: i64,
    pub user_id: i64,
    pub provider: String,
    pub auth_kind: String,
    pub account_email: String,
    pub account_label: String,
    pub status: String,
    pub expires_at: String,
    pub last_validated_at: String,
    pub last_used_at: String,
    pub last_error: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct LinkedCredentialSecret {
    pub entry: LinkedCredentialEntry,
    pub bundle: LinkedCredentialBundle,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceRow {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub kind: String,
    pub owner_user_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceMembershipRow {
    pub workspace_id: i64,
    pub name: String,
    pub slug: String,
    pub kind: String,
    pub role: String,
    pub is_default: bool,
    pub created_at: String,
}

#[derive(serde::Serialize)]
pub struct CitationVerification {
    pub id: i64,
    pub task_id: i64,
    pub citation_text: String,
    pub citation_type: String,
    pub status: String,
    pub source: String,
    pub treatment: String,
    pub checked_at: String,
    pub created_at: String,
}

pub struct RegisteredGroup {
    pub jid: String,
    pub name: String,
    pub folder: String,
    pub trigger_pattern: String,
    pub requires_trigger: bool,
}

pub struct ChatAgentRun {
    pub id: i64,
    pub jid: String,
    pub status: String,
    pub transport: String,
    pub original_id: String,
    pub trigger_msg_id: String,
    pub folder: String,
    pub output: String,
    pub new_session_id: String,
    pub last_msg_timestamp: String,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsageSummary {
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
    pub message_count: i64,
    pub task_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectRow {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub mode: String,
    pub repo_path: String,
    pub client_name: String,
    pub case_number: String,
    pub jurisdiction: String,
    pub matter_type: String,
    pub opposing_counsel: String,
    pub deadline: Option<String>,
    pub privilege_level: String,
    pub status: String,
    pub session_privileged: bool,
    pub default_template_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProjectShareRow {
    pub id: i64,
    pub project_id: i64,
    pub user_id: i64,
    pub role: String,
    pub granted_by: Option<i64>,
    pub username: String,
    pub display_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SharedProjectRow {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub mode: String,
    pub repo_path: String,
    pub client_name: String,
    pub case_number: String,
    pub jurisdiction: String,
    pub matter_type: String,
    pub opposing_counsel: String,
    pub deadline: Option<String>,
    pub privilege_level: String,
    pub status: String,
    pub session_privileged: bool,
    pub default_template_id: Option<i64>,
    pub created_at: String,
    pub share_role: String,
    pub workspace_name: String,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProjectShareLinkRow {
    pub id: i64,
    pub project_id: i64,
    pub token: String,
    pub label: String,
    pub expires_at: String,
    pub created_by: Option<i64>,
    pub revoked: bool,
    pub created_at: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ProjectFileRow {
    pub id: i64,
    pub project_id: i64,
    pub file_name: String,
    pub source_path: String,
    pub stored_path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub extracted_text: String,
    pub content_hash: String,
    pub privileged: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ProjectFileMetaRow {
    pub id: i64,
    pub project_id: i64,
    pub file_name: String,
    pub source_path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub privileged: bool,
    pub has_text: bool,
    pub text_chars: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, Clone, Default)]
pub struct ProjectFileStats {
    pub project_id: i64,
    pub total_files: i64,
    pub total_bytes: i64,
    pub privileged_files: i64,
    pub text_files: i64,
    pub text_chars: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ProjectFilePageCursor {
    pub created_at: String,
    pub id: i64,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct KnowledgeFile {
    pub id: i64,
    pub workspace_id: i64,
    pub file_name: String,
    pub description: String,
    pub size_bytes: i64,
    pub inline: bool,
    pub tags: String,
    pub category: String,
    pub jurisdiction: String,
    pub project_id: Option<i64>,
    pub user_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct KnowledgeRepo {
    pub id: i64,
    pub workspace_id: i64,
    pub user_id: Option<i64>,
    pub url: String,
    pub name: String,
    pub local_path: String,
    pub status: String,
    pub error_msg: String,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct AuditEvent {
    pub id: i64,
    pub task_id: Option<i64>,
    pub project_id: Option<i64>,
    pub actor: String,
    pub kind: String,
    pub payload: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct FtsResult {
    pub project_id: i64,
    pub task_id: i64,
    pub file_path: String,
    pub title_snippet: String,
    pub content_snippet: String,
    pub rank: f64,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct CloudConnection {
    pub id: i64,
    pub project_id: i64,
    /// "dropbox" | "google_drive" | "onedrive"
    pub provider: String,
    pub access_token: String,
    pub refresh_token: String,
    /// ISO 8601 expiry timestamp
    pub token_expiry: String,
    pub account_email: String,
    pub account_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct UploadSession {
    pub id: i64,
    pub project_id: i64,
    pub file_name: String,
    pub mime_type: String,
    pub file_size: i64,
    pub chunk_size: i64,
    pub total_chunks: i64,
    pub uploaded_bytes: i64,
    pub is_zip: bool,
    pub privileged: bool,
    pub status: String,
    pub stored_path: String,
    pub error: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ThemeTerm {
    pub term: String,
    pub occurrences: i64,
    pub document_count: i64,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ThemeSummary {
    pub documents_scanned: i64,
    pub tokens_scanned: i64,
    pub keywords: Vec<ThemeTerm>,
    pub phrases: Vec<ThemeTerm>,
}

// ── Timestamp helpers ─────────────────────────────────────────────────────

fn parse_ts(s: &str) -> DateTime<Utc> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| ndt.and_utc())
        .unwrap_or_else(|e| {
            tracing::warn!("failed to parse timestamp '{s}': {e}");
            Utc::now()
        })
}

fn now_str() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn unique_slug(base: &str, suffix: i64) -> String {
    let slug = slugify(base);
    if suffix <= 0 {
        if slug.is_empty() {
            "workspace".to_string()
        } else {
            slug
        }
    } else if slug.is_empty() {
        format!("workspace-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

fn row_to_workspace(row: &pg::Row<'_>) -> pg::Result<WorkspaceRow> {
    Ok(WorkspaceRow {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        kind: row.get(3)?,
        owner_user_id: row.get(4)?,
        created_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
    })
}

fn row_to_knowledge(row: &pg::Row<'_>) -> pg::Result<KnowledgeFile> {
    let inline_int: i64 = row.get(5)?;
    Ok(KnowledgeFile {
        id: row.get(0)?,
        workspace_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
        file_name: row.get(2)?,
        description: row.get(3)?,
        size_bytes: row.get(4)?,
        inline: inline_int != 0,
        created_at: row.get(6)?,
        tags: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        category: row
            .get::<_, Option<String>>(8)?
            .unwrap_or_else(|| "general".to_string()),
        jurisdiction: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
        project_id: row.get::<_, Option<i64>>(10)?,
        user_id: row.get::<_, Option<i64>>(11)?,
    })
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "been"
            | "being"
            | "but"
            | "by"
            | "can"
            | "could"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "if"
            | "in"
            | "into"
            | "is"
            | "it"
            | "its"
            | "may"
            | "might"
            | "must"
            | "not"
            | "of"
            | "on"
            | "or"
            | "our"
            | "shall"
            | "should"
            | "that"
            | "the"
            | "their"
            | "there"
            | "these"
            | "they"
            | "this"
            | "those"
            | "to"
            | "under"
            | "upon"
            | "was"
            | "were"
            | "will"
            | "with"
            | "would"
            | "you"
            | "your"
    )
}

fn tokenize_for_themes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
            if current.len() >= 40 {
                out.push(current.clone());
                current.clear();
            }
        } else if !current.is_empty() {
            if current.len() >= 3 && !is_stopword(&current) {
                out.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && current.len() >= 3 && !is_stopword(&current) {
        out.push(current);
    }
    out
}

fn push_theme_term(out: &mut Vec<ThemeTerm>, term: String, occurrences: i64, document_count: i64) {
    out.push(ThemeTerm {
        term,
        occurrences,
        document_count,
    });
}

fn row_to_cloud_connection(row: &pg::Row<'_>) -> pg::Result<CloudConnection> {
    Ok(CloudConnection {
        id: row.get(0)?,
        project_id: row.get(1)?,
        provider: row.get(2)?,
        access_token: row.get(3)?,
        refresh_token: row.get(4)?,
        token_expiry: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        account_email: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        account_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        created_at: row.get::<_, String>(8).map(|s| parse_ts(&s))?,
    })
}

fn row_to_upload_session(row: &pg::Row<'_>) -> pg::Result<UploadSession> {
    let is_zip: i64 = row.get(8)?;
    let privileged: i64 = row.get(9)?;
    Ok(UploadSession {
        id: row.get(0)?,
        project_id: row.get(1)?,
        file_name: row.get(2)?,
        mime_type: row.get(3)?,
        file_size: row.get(4)?,
        chunk_size: row.get(5)?,
        total_chunks: row.get(6)?,
        uploaded_bytes: row.get(7)?,
        is_zip: is_zip != 0,
        privileged: privileged != 0,
        status: row.get(10)?,
        stored_path: row.get::<_, Option<String>>(11)?.unwrap_or_default(),
        error: row.get::<_, Option<String>>(12)?.unwrap_or_default(),
        created_at: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
        updated_at: row.get::<_, Option<String>>(14)?.unwrap_or_default(),
    })
}

// ── Row mappers ───────────────────────────────────────────────────────────

const TASK_COLS: &str = "id, title, description, repo_path, branch, status, attempt, \
    max_attempts, last_error, created_by, notify_chat, created_at, \
    session_id, mode, backend, workspace_id, project_id, task_type, requires_exhaustive_corpus_review, \
    started_at, completed_at, duration_secs, review_status, revision_count, updated_at, chat_thread";

fn row_to_task(row: &pg::Row<'_>) -> pg::Result<Task> {
    let created_at_str: String = row.get(11)?;
    let started_at: Option<String> = row.get(19)?;
    let completed_at: Option<String> = row.get(20)?;
    let updated_at_str: String = row.get(24)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        repo_path: row.get(3)?,
        branch: row.get(4)?,
        status: row.get(5)?,
        attempt: row.get(6)?,
        max_attempts: row.get(7)?,
        last_error: row.get(8)?,
        created_by: row.get(9)?,
        notify_chat: row.get(10)?,
        created_at: parse_ts(&created_at_str),
        updated_at: parse_ts(&updated_at_str),
        session_id: row.get(12)?,
        mode: row.get(13)?,
        backend: row.get::<_, Option<String>>(14)?.unwrap_or_default(),
        workspace_id: row.get::<_, Option<i64>>(15)?.unwrap_or(0),
        project_id: row.get::<_, Option<i64>>(16)?.unwrap_or(0),
        task_type: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
        requires_exhaustive_corpus_review: row.get::<_, Option<i64>>(18)?.unwrap_or(0) != 0,
        started_at: started_at.map(|s| parse_ts(&s)),
        completed_at: completed_at.map(|s| parse_ts(&s)),
        duration_secs: row.get(21)?,
        review_status: row.get(22)?,
        revision_count: row.get::<_, Option<i64>>(23)?.unwrap_or(0),
        chat_thread: row.get::<_, Option<String>>(25)?.unwrap_or_default(),
    })
}

fn row_to_proposal(row: &pg::Row<'_>) -> pg::Result<Proposal> {
    let created_at_str: String = row.get(6)?;
    Ok(Proposal {
        id: row.get(0)?,
        repo_path: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        rationale: row.get(4)?,
        status: row.get(5)?,
        created_at: parse_ts(&created_at_str),
        triage_score: row.get(7)?,
        triage_impact: row.get(8)?,
        triage_feasibility: row.get(9)?,
        triage_risk: row.get(10)?,
        triage_effort: row.get(11)?,
        triage_reasoning: row.get(12)?,
    })
}

fn row_to_queue_entry(row: &pg::Row<'_>) -> pg::Result<QueueEntry> {
    let queued_at_str: String = row.get(5)?;
    Ok(QueueEntry {
        id: row.get(0)?,
        task_id: row.get(1)?,
        branch: row.get(2)?,
        repo_path: row.get(3)?,
        status: row.get(4)?,
        queued_at: parse_ts(&queued_at_str),
        pr_number: row.get(6)?,
    })
}

fn row_to_task_output(row: &pg::Row<'_>) -> pg::Result<TaskOutput> {
    let created_at_str: String = row.get(6)?;
    Ok(TaskOutput {
        id: row.get(0)?,
        task_id: row.get(1)?,
        phase: row.get(2)?,
        output: row.get(3)?,
        raw_stream: row.get(4)?,
        exit_code: row.get(5)?,
        created_at: parse_ts(&created_at_str),
    })
}

fn row_to_task_message(row: &pg::Row<'_>) -> pg::Result<TaskMessage> {
    let created_at_str: String = row.get(4)?;
    Ok(TaskMessage {
        id: row.get(0)?,
        task_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        created_at: parse_ts(&created_at_str),
        delivered_phase: row.get(5)?,
    })
}

fn row_to_repo(row: &pg::Row<'_>) -> pg::Result<RepoRow> {
    let auto_merge_int: i64 = row.get(7)?;
    Ok(RepoRow {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        mode: row.get(3)?,
        backend: row.get(4)?,
        test_cmd: row.get(5)?,
        prompt_file: row.get(6)?,
        auto_merge: auto_merge_int != 0,
        repo_slug: row.get(8).unwrap_or_default(),
    })
}

fn row_to_chat_agent_run(row: &pg::Row<'_>) -> pg::Result<ChatAgentRun> {
    Ok(ChatAgentRun {
        id: row.get(0)?,
        jid: row.get(1)?,
        status: row.get(2)?,
        transport: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        original_id: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        trigger_msg_id: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        folder: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        output: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        new_session_id: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
        last_msg_timestamp: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
        started_at: row.get::<_, Option<String>>(10)?.unwrap_or_default(),
        completed_at: row.get(11)?,
    })
}

fn row_to_legacy_event(row: &pg::Row<'_>) -> pg::Result<LegacyEvent> {
    Ok(LegacyEvent {
        id: row.get(0)?,
        ts: row.get(1)?,
        level: row.get(2)?,
        category: row.get(3)?,
        message: row.get(4)?,
        metadata: row.get(5)?,
    })
}

const PROJECT_COLS: &str = "id, workspace_id, name, mode, repo_path, client_name, case_number, jurisdiction, \
    matter_type, opposing_counsel, deadline, privilege_level, status, default_template_id, created_at, session_privileged";

fn row_to_project(row: &pg::Row<'_>) -> pg::Result<ProjectRow> {
    let created_at_str: String = row.get(14)?;
    let session_privileged_int: i64 = row.get(15)?;
    Ok(ProjectRow {
        id: row.get(0)?,
        workspace_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
        name: row.get(2)?,
        mode: row.get(3)?,
        repo_path: row.get(4)?,
        client_name: row.get(5)?,
        case_number: row.get(6)?,
        jurisdiction: row.get(7)?,
        matter_type: row.get(8)?,
        opposing_counsel: row.get(9)?,
        deadline: row.get(10)?,
        privilege_level: row.get(11)?,
        status: row.get(12)?,
        session_privileged: session_privileged_int != 0,
        default_template_id: row.get(13)?,
        created_at: parse_ts(&created_at_str),
    })
}

const PROJECT_FILE_COLS: &str = "id, project_id, file_name, source_path, stored_path, mime_type, size_bytes, extracted_text, content_hash, created_at, privileged";
const PROJECT_FILE_META_COLS: &str = "id, project_id, file_name, source_path, mime_type, size_bytes, privileged, created_at, length(extracted_text)::BIGINT";

fn row_to_project_file(row: &pg::Row<'_>) -> pg::Result<ProjectFileRow> {
    let created_at_str: String = row.get(9)?;
    let privileged_int: i64 = row.get(10)?;
    Ok(ProjectFileRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        file_name: row.get(2)?,
        source_path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        stored_path: row.get(4)?,
        mime_type: row.get(5)?,
        size_bytes: row.get(6)?,
        extracted_text: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        content_hash: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
        privileged: privileged_int != 0,
        created_at: parse_ts(&created_at_str),
    })
}

fn row_to_project_file_meta(row: &pg::Row<'_>) -> pg::Result<ProjectFileMetaRow> {
    let created_at_str: String = row.get(7)?;
    let privileged_int: i64 = row.get(6)?;
    let text_chars: i64 = row.get::<_, Option<i64>>(8)?.unwrap_or(0);
    Ok(ProjectFileMetaRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        file_name: row.get(2)?,
        source_path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        mime_type: row.get(4)?,
        size_bytes: row.get(5)?,
        privileged: privileged_int != 0,
        has_text: text_chars > 0,
        text_chars,
        created_at: parse_ts(&created_at_str),
    })
}

fn row_to_tool_call(row: &pg::Row<'_>) -> pg::Result<crate::tool_calls::ToolCallEvent> {
    Ok(crate::tool_calls::ToolCallEvent {
        id: row.get(0)?,
        task_id: row.get(1)?,
        chat_key: row.get(2)?,
        run_id: row.get(3)?,
        tool_name: row.get(4)?,
        input_summary: row.get(5)?,
        output_summary: row.get(6)?,
        started_at: row.get(7)?,
        duration_ms: row.get(8)?,
        success: row.get(9)?,
        error: row.get(10)?,
    })
}

// ── Db impl ───────────────────────────────────────────────────────────────

impl Db {
    pub fn open(database_url: &str) -> Result<Self> {
        let conn = Connection::open(database_url)
            .with_context(|| format!("failed to open Postgres database at {database_url:?}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&mut self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute_batch(SCHEMA_SQL)
            .context("failed to apply clean-break Postgres schema")?;
        Self::backfill_workspaces(&conn).context("workspace backfill")?;
        Ok(())
    }

    fn get_or_create_workspace(
        conn: &ConnectionGuard,
        name: &str,
        kind: &str,
        owner_user_id: Option<i64>,
        preferred_slug: &str,
    ) -> Result<i64> {
        let existing = if owner_user_id.is_some() {
            conn.query_row(
                "SELECT id FROM workspaces WHERE owner_user_id = ?1 AND kind = ?2 ORDER BY id ASC LIMIT 1",
                params![owner_user_id, kind],
                |row| row.get(0),
            )
            .optional()?
        } else {
            conn.query_row(
                "SELECT id FROM workspaces WHERE slug = ?1 AND kind = ?2 ORDER BY id ASC LIMIT 1",
                params![preferred_slug, kind],
                |row| row.get(0),
            )
            .optional()?
        };
        if let Some(id) = existing {
            return Ok(id);
        }
        let slug = if preferred_slug.trim().is_empty() {
            unique_slug(name, 0)
        } else {
            preferred_slug.to_string()
        };
        conn.execute_returning_id(
            "INSERT INTO workspaces (name, slug, kind, owner_user_id) VALUES (?1, ?2, ?3, ?4)",
            params![name, slug, kind, owner_user_id],
        )
        .context("insert workspace")
    }

    fn backfill_workspaces(conn: &ConnectionGuard) -> Result<()> {
        let mut stmt = conn.prepare(
            "SELECT id, username, display_name FROM users \
             WHERE default_workspace_id IS NULL ORDER BY id ASC",
        )?;
        let users = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<pg::Result<Vec<_>>>()?;

        for (user_id, username, display_name) in users {
            let workspace_name = if display_name.trim().is_empty() {
                format!("{username} Personal")
            } else {
                format!("{display_name} Personal")
            };
            let workspace_id = Self::get_or_create_workspace(
                conn,
                &workspace_name,
                "personal",
                Some(user_id),
                &unique_slug(&format!("{username}-personal"), 0),
            )?;
            conn.execute(
                "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, 'owner') \
                 ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
                params![workspace_id, user_id],
            )?;
            conn.execute(
                "UPDATE users SET default_workspace_id = ?1 WHERE id = ?2",
                params![workspace_id, user_id],
            )?;
        }

        let legacy_counts: i64 = conn
            .query_row(
                "SELECT \
                    (SELECT COUNT(*) FROM projects WHERE workspace_id IS NULL) + \
                    (SELECT COUNT(*) FROM pipeline_tasks WHERE workspace_id IS NULL) + \
                    (SELECT COUNT(*) FROM knowledge_files WHERE workspace_id IS NULL) + \
                    (SELECT COUNT(*) FROM api_keys WHERE workspace_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let system_workspace_id =
            Self::get_or_create_workspace(conn, "System Workspace", "system", None, "system")?;

        if legacy_counts > 0 {
            let legacy_workspace_id = Self::get_or_create_workspace(
                conn,
                "Legacy Shared",
                "shared",
                None,
                "legacy-shared",
            )?;
            let mut members = conn.prepare("SELECT id, is_admin FROM users ORDER BY id ASC")?;
            for row in members.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?))
            })? {
                let (user_id, is_admin) = row?;
                let role = if is_admin { "admin" } else { "member" };
                conn.execute(
                    "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, ?3) \
                     ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
                    params![legacy_workspace_id, user_id, role],
                )?;
            }
            conn.execute(
                "UPDATE projects SET workspace_id = ?1 WHERE workspace_id IS NULL",
                params![legacy_workspace_id],
            )?;
            conn.execute(
                "UPDATE pipeline_tasks SET workspace_id = COALESCE((SELECT workspace_id FROM projects WHERE projects.id = pipeline_tasks.project_id), ?1) \
                 WHERE workspace_id IS NULL",
                params![legacy_workspace_id],
            )?;
            conn.execute(
                "UPDATE knowledge_files SET workspace_id = COALESCE((SELECT workspace_id FROM projects WHERE projects.id = knowledge_files.project_id), ?1) \
                 WHERE workspace_id IS NULL",
                params![legacy_workspace_id],
            )?;
            conn.execute(
                "UPDATE api_keys SET workspace_id = ?1 WHERE workspace_id IS NULL",
                params![legacy_workspace_id],
            )?;
        } else {
            conn.execute(
                "UPDATE pipeline_tasks SET workspace_id = COALESCE((SELECT workspace_id FROM projects WHERE projects.id = pipeline_tasks.project_id), ?1) \
                 WHERE workspace_id IS NULL",
                params![system_workspace_id],
            )?;
        }

        Ok(())
    }

    // ── Config ────────────────────────────────────────────────────────────

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .context("get_config")?;
        Ok(result)
    }

    pub fn ensure_config(&self, key: &str, value: &str) -> Result<()> {
        if self.get_config(key)?.is_none() {
            self.set_config(key, value)?;
        }
        Ok(())
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let updated_at = now_str();
        conn.execute(
            "INSERT INTO config (key, value, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, updated_at],
        )
        .context("set_config")?;
        Ok(())
    }

    // ── Timing state (persisted across restarts) ──────────────────────────

    pub fn get_ts(&self, key: &str) -> i64 {
        self.get_config(key)
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    pub fn set_ts(&self, key: &str, value: i64) {
        let _ = self.set_config(key, &value.to_string());
    }

}
