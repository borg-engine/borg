mod dispatch;
mod integration;
pub(crate) mod legal_guards;
mod phases;
mod seed;

use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

pub use crate::types::PipelineEvent;
use crate::{
    agent::AgentBackend,
    config::Config,
    db::Db,
    git::Git,
    linked_credentials::{
        capture_bundle, claude_oauth_token_from_home, restore_bundle, should_revalidate,
        validate_home, PROVIDER_CLAUDE, PROVIDER_OPENAI,
    },
    modes::get_mode,
    registry::PluginRegistry,
    sandbox::{Sandbox, SandboxMode},
    stream::TaskStreamManager,
    types::{
        BenchmarkPhaseState, ContainerTestResult, IntegrationType, PhaseCompletionVerdict,
        PhaseConfig, PhaseContext, PhaseHistoryEntry, PhaseOutput, PhaseType, PipelineMode,
        PipelineStateSnapshot, Proposal, RepoConfig, SeedOutputType, Task,
    },
};

/// Derive a compile-only check command from a test command, if possible.
/// For `cargo test` commands, returns the same command with `--no-run` appended.
pub fn derive_compile_check(test_cmd: &str) -> Option<String> {
    let trimmed = test_cmd.trim();
    if !trimmed.contains("cargo test") {
        return None;
    }
    if trimmed.contains("--no-run") {
        return Some(trimmed.to_string());
    }
    Some(format!("{trimmed} --no-run"))
}

pub struct Pipeline {
    pub db: Arc<Db>,
    pub registry: Arc<PluginRegistry>,
    pub config: Arc<Config>,
    pub ai_request_count: Arc<AtomicU64>,
    pub sandbox: Sandbox,
    pub sandbox_mode: SandboxMode,
    pub event_tx: broadcast::Sender<PipelineEvent>,
    pub stream_manager: Arc<TaskStreamManager>,
    pub force_restart: Arc<std::sync::atomic::AtomicBool>,
    /// Per-(repo_path, seed_name) last-run timestamp for independent per-seed cooldowns.
    seed_cooldowns: Mutex<HashMap<(String, String), i64>>,
    pub chat_event_tx: Option<broadcast::Sender<String>>,
    pub(crate) last_self_update_secs: std::sync::atomic::AtomicI64,
    last_cache_prune_secs: std::sync::atomic::AtomicI64,
    last_session_prune_secs: std::sync::atomic::AtomicI64,
    pub(crate) startup_heads: HashMap<String, String>,
    in_flight: Mutex<HashSet<i64>>,
    in_flight_repos: Mutex<HashSet<String>>,
    /// Per-task last agent dispatch timestamp (epoch seconds) for rate limiting.
    last_agent_dispatch: Mutex<HashMap<i64, i64>>,
    /// Per-task deferred retry unlock timestamp (epoch seconds).
    retry_not_before: Mutex<HashMap<i64, i64>>,
    /// Prevents overlapping seed runs (seeding is spawned in background).
    seeding_active: std::sync::atomic::AtomicBool,
    /// Tracks repeated phase-failure signatures per task to detect stuck loops.
    failure_signatures: std::sync::Mutex<HashMap<(i64, String), (String, u32)>>,
    /// Whether the borg-agent-net Docker bridge network was successfully created at startup.
    pub agent_network_available: bool,
    pub embed_registry: crate::knowledge::EmbeddingRegistry,
    /// Set to true during graceful shutdown — prevents dispatching new tasks.
    pub draining: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Deserialize)]
struct GithubIssueLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GithubIssue {
    number: i64,
    title: String,
    #[serde(default)]
    body: String,
    url: String,
    #[serde(default)]
    labels: Vec<GithubIssueLabel>,
}

impl Pipeline {
    fn task_ready_for_dispatch(&self, task: &Task) -> bool {
        let Some(mode) = self.resolve_mode(&task.mode) else {
            let err = format!("unknown pipeline mode: {}", task.mode);
            error!("task #{}: {err}", task.id);
            let _ = self.db.update_task_status(task.id, "failed", Some(&err));
            return false;
        };
        let Some(phase) = mode.get_phase(&task.status) else {
            return false;
        };
        if phase.phase_type == PhaseType::HumanReview {
            return false;
        }
        if let Some(wait_s) = phase.wait_s {
            let ready_at = task.updated_at + chrono::Duration::seconds(wait_s.max(0));
            if Utc::now() < ready_at {
                return false;
            }
        }
        true
    }

    fn task_session_dir_rel(task: &Task) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        task.repo_path.hash(&mut hasher);
        format!("store/sessions/task-{}-{:016x}", task.id, hasher.finish())
    }

    fn task_session_dir(task: &Task) -> String {
        let rel = Self::task_session_dir_rel(task);
        std::fs::canonicalize(&rel)
            .unwrap_or_else(|_| std::path::PathBuf::from(&rel))
            .to_string_lossy()
            .to_string()
    }

    fn custom_modes_from_db(&self) -> Vec<PipelineMode> {
        let raw = match self.db.get_config("custom_modes") {
            Ok(Some(v)) => v,
            _ => return Vec::new(),
        };
        serde_json::from_str::<Vec<PipelineMode>>(&raw).unwrap_or_default()
    }

    fn resolve_mode(&self, name: &str) -> Option<PipelineMode> {
        get_mode(name).or_else(|| {
            self.custom_modes_from_db()
                .into_iter()
                .find(|m| m.name == name)
        })
    }

    pub fn new(
        db: Arc<Db>,
        registry: Arc<PluginRegistry>,
        config: Arc<Config>,
        sandbox_mode: SandboxMode,
        force_restart: Arc<std::sync::atomic::AtomicBool>,
        agent_network_available: bool,
        ai_request_count: Arc<AtomicU64>,
    ) -> (Self, broadcast::Receiver<PipelineEvent>) {
        let (tx, rx) = broadcast::channel(256);
        // Capture git HEAD for each watched repo at startup (used for self-update detection)
        let mut startup_heads = HashMap::new();
        for repo in &config.watched_repos {
            if repo.is_self {
                if let Ok(head) = crate::git::Git::new(&repo.path).rev_parse_head() {
                    startup_heads.insert(repo.path.clone(), head);
                }
            }
        }
        let seed_cooldowns = db.get_seed_cooldowns().unwrap_or_default();
        let p = Self {
            db,
            registry,
            config,
            ai_request_count,
            sandbox: Sandbox,
            sandbox_mode,
            event_tx: tx,
            stream_manager: TaskStreamManager::new(),
            chat_event_tx: None,
            force_restart,
            seed_cooldowns: Mutex::new(seed_cooldowns),
            last_self_update_secs: std::sync::atomic::AtomicI64::new(0),
            last_cache_prune_secs: std::sync::atomic::AtomicI64::new(0),
            last_session_prune_secs: std::sync::atomic::AtomicI64::new(0),
            startup_heads,
            in_flight: Mutex::new(HashSet::new()),
            in_flight_repos: Mutex::new(HashSet::new()),
            last_agent_dispatch: Mutex::new(HashMap::new()),
            retry_not_before: Mutex::new(HashMap::new()),
            seeding_active: std::sync::atomic::AtomicBool::new(false),
            failure_signatures: std::sync::Mutex::new(HashMap::new()),
            agent_network_available,
            embed_registry: crate::knowledge::EmbeddingRegistry::from_env(),
            draining: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        (p, rx)
    }

    // ── Backend resolution ────────────────────────────────────────────────

    /// Select the agent backend for a task: task override → repo override → global.
    /// Returns None if the resolved backend name isn't registered (missing API key, etc).
    fn resolve_backend(&self, task: &Task) -> Option<Arc<dyn AgentBackend>> {
        let name = self.selected_backend_name(task);
        if let Some(b) = self.registry.get_backend(&name) {
            return Some(Arc::clone(b));
        }
        warn!(
            task_id = task.id,
            backend = %name,
            "backend not registered (missing API key?), skipping task"
        );
        None
    }

    async fn run_backend_phase(
        &self,
        backend: &Arc<dyn AgentBackend>,
        task: &Task,
        phase: &PhaseConfig,
        ctx: PhaseContext,
    ) -> Result<PhaseOutput> {
        // Ensure agent CLI tools are current (no-ops if updated within 24h)
        crate::tool_update::ensure_agent_tools_updated().await;
        self.ai_request_count.fetch_add(1, Ordering::Relaxed);
        backend.run_phase(task, phase, ctx).await
    }

    // ── Small helpers ─────────────────────────────────────────────────────

    pub fn active_agent_count(&self) -> usize {
        self.in_flight.try_lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Resolve repo config for a task, filling in defaults if not found.
    fn repo_config(&self, task: &Task) -> RepoConfig {
        self.config
            .watched_repos
            .iter()
            .find(|r| r.path == task.repo_path)
            .cloned()
            .unwrap_or_else(|| RepoConfig {
                path: task.repo_path.clone(),
                test_cmd: String::new(),
                prompt_file: String::new(),
                mode: task.mode.clone(),
                is_self: false,
                auto_merge: true,
                lint_cmd: String::new(),
                backend: String::new(),
                repo_slug: String::new(),
            })
    }

    /// Resolve the backend name that will be used for this task.
    fn selected_backend_name(&self, task: &Task) -> String {
        if !task.backend.is_empty() {
            return task.backend.clone();
        }
        if let Some(repo) = self
            .config
            .watched_repos
            .iter()
            .find(|r| r.path == task.repo_path)
        {
            if !repo.backend.is_empty() {
                return repo.backend.clone();
            }
        }
        self.config.pipeline.backend.clone()
    }

    fn repo_lint_cmd(&self, repo_path: &str, _worktree_path: &str) -> Option<String> {
        let repo = self
            .config
            .watched_repos
            .iter()
            .find(|r| r.path == repo_path)?;
        let lint_cmd = repo.lint_cmd.trim();
        if lint_cmd.is_empty() {
            None
        } else {
            Some(lint_cmd.to_string())
        }
    }

    fn task_wall_timeout_s(&self) -> u64 {
        // Whole-task timeout should be materially above per-command timeouts.
        (self.config.agent_timeout_s.max(300) as u64)
            .saturating_mul(3)
            .max(900)
    }

    fn retry_backoff_secs(&self, task_id: i64, attempt: i64, error: &str) -> Option<i64> {
        let class = classify_retry_error(error);
        let exp = ((attempt - 1).max(0) as u32).min(6);
        let secs = match class {
            RetryClass::Resource => (30_i64 * (1_i64 << exp)).min(600),
            RetryClass::Transient => (15_i64 * (1_i64 << exp)).min(300),
            _ => return None,
        };
        let now = Utc::now().timestamp();
        let unlock_at = now + secs;
        if let Ok(mut m) = self.retry_not_before.try_lock() {
            m.insert(task_id, unlock_at);
        }
        Some(secs)
    }

    fn should_defer_retry(&self, task_id: i64) -> Option<i64> {
        let now = Utc::now().timestamp();
        let map = match self.retry_not_before.try_lock() {
            Ok(m) => m,
            Err(_) => return Some(5),
        };
        let unlock_at = *map.get(&task_id)?;
        if unlock_at > now {
            Some(unlock_at - now)
        } else {
            None
        }
    }

    fn pipeline_tmp_dir(&self) -> PathBuf {
        PathBuf::from(format!("{}/tmp", self.config.data_dir))
    }

    fn ensure_tmp_capacity(&self, task_id: i64, phase: &str) -> Result<()> {
        const MIN_TMP_FREE_BYTES: u64 = 512 * 1024 * 1024;
        const MIN_TMP_FREE_INODES: u64 = 5_000;
        const MAX_TMP_INODE_USED_PCT: f64 = 85.0;

        let is_healthy = |h: &TmpHealth| {
            h.inode_used_pct < MAX_TMP_INODE_USED_PCT
                && h.free_bytes >= MIN_TMP_FREE_BYTES
                && h.free_inodes >= MIN_TMP_FREE_INODES
        };

        let before = tmp_health("/tmp");
        if before.as_ref().is_some_and(is_healthy) {
            return Ok(());
        }

        let msg = if let Some(h) = before {
            format!(
                "Self-heal: low /tmp capacity before {phase} (task #{task_id}): inode_used={:.1}% free_inodes={} free_bytes={}MB",
                h.inode_used_pct,
                h.free_inodes,
                h.free_bytes / (1024 * 1024)
            )
        } else {
            format!("Self-heal: could not read /tmp statvfs before {phase} (task #{task_id})")
        };
        warn!("{msg}");
        self.notify(&self.config.pipeline.admin_chat, &msg);

        let removed_tmp = cleanup_tmp_prefixes("/tmp", &["borg-rebase-task-", "borg-", "task-"]);
        let pipeline_tmp = self.pipeline_tmp_dir();
        std::fs::create_dir_all(&pipeline_tmp).ok();
        let removed_pipeline_tmp = cleanup_tmp_prefixes(
            &pipeline_tmp.to_string_lossy(),
            &["borg-rebase-task-", "borg-", "task-"],
        );

        let after = tmp_health("/tmp");
        if after.as_ref().is_some_and(is_healthy) {
            if let Some(h) = after {
                let healed = format!(
                    "Self-heal success: cleaned tmp artifacts ({removed_tmp} in /tmp, {removed_pipeline_tmp} in {}) now inode_used={:.1}% free_inodes={} free_bytes={}MB",
                    pipeline_tmp.display(),
                    h.inode_used_pct,
                    h.free_inodes,
                    h.free_bytes / (1024 * 1024)
                );
                info!("{healed}");
                self.notify(&self.config.pipeline.admin_chat, &healed);
            }
            return Ok(());
        }

        if let Some(h) = after {
            anyhow::bail!(
                "tmp still unhealthy after self-heal before {phase}: inode_used={:.1}% free_inodes={} free_bytes={}MB",
                h.inode_used_pct,
                h.free_inodes,
                h.free_bytes / (1024 * 1024)
            );
        }
        anyhow::bail!("tmp still unhealthy after self-heal before {phase}");
    }

    fn maybe_self_heal_tmp(&self) {
        const HEAL_INTERVAL_S: i64 = 120;
        let now = Utc::now().timestamp();
        let last = self.db.get_ts("last_tmp_self_heal_ts");
        if now - last < HEAL_INTERVAL_S {
            return;
        }
        self.db.set_ts("last_tmp_self_heal_ts", now);
        let _ = self.ensure_tmp_capacity(0, "tick_guardrail");
    }

    /// Resolve the GitHub token for a task: per-user setting → global config → `gh auth token`.
    /// Returns (token, is_user_token). When is_user_token is false, the token belongs to
    /// the Borg service account and PRs should attribute the requesting user.
    fn resolve_gh_token(&self, created_by: &str) -> (String, bool) {
        if !created_by.is_empty() {
            if let Ok(Some((uid, _, _, _, _))) = self.db.get_user_by_username(created_by) {
                if let Ok(Some(tok)) = self.db.get_user_setting(uid, "github_token") {
                    if !tok.is_empty() {
                        return (tok, true);
                    }
                }
            }
        }
        if !self.config.git.github_token.is_empty() {
            return (self.config.git.github_token.clone(), false);
        }
        let tok = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        (tok, false)
    }

    /// Read git co-author settings from DB (runtime-editable), falling back to Config.
    fn git_coauthor_settings(&self) -> (bool, String) {
        let claude_coauthor = self
            .db
            .get_config("git_claude_coauthor")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(self.config.git.claude_coauthor);
        let user_coauthor = self
            .db
            .get_config("git_user_coauthor")
            .ok()
            .flatten()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| self.config.git.user_coauthor.clone());
        (claude_coauthor, user_coauthor)
    }

    /// Build system prompt suffix for co-author instructions.
    fn build_system_prompt_suffix(claude_coauthor: bool, user_coauthor: &str) -> String {
        let mut s = String::new();
        if !claude_coauthor {
            s.push_str("Do not add Co-Authored-By trailers to commit messages.");
        }
        if !user_coauthor.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str("Git author is configured via environment variables — do not override with --author.");
        }
        s
    }

    /// Append user co-author trailer to a commit message if configured.
    fn with_user_coauthor(message: &str, user_coauthor: &str) -> String {
        if user_coauthor.is_empty() {
            return message.to_string();
        }
        format!("{message}\n\nCo-Authored-By: {user_coauthor}")
    }
    // ── Test runner ───────────────────────────────────────────────────────

    pub(crate) async fn run_test_command_for_task(
        &self,
        task: &Task,
        dir: &str,
        cmd: &str,
    ) -> Result<TestOutput> {
        self.ensure_tmp_capacity(task.id, "run_test_command")?;
        self.run_test_command(dir, cmd).await
    }

    pub(crate) async fn run_test_command(&self, dir: &str, cmd: &str) -> Result<TestOutput> {
        self.ensure_tmp_capacity(0, "run_test_command")?;
        let tmp_dir = self.pipeline_tmp_dir();
        std::fs::create_dir_all(&tmp_dir).ok();
        let timeout = std::time::Duration::from_secs(self.config.agent_timeout_s.max(300) as u64);
        let output = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(dir)
                .env("TMPDIR", tmp_dir.to_string_lossy().to_string())
                .output(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "run_test_command timed out after {}s: {cmd}",
                timeout.as_secs()
            )
        })?
        .context("run test command")?;

        Ok(TestOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(1),
        })
    }

    /// Run a test command inside a fresh Docker container (for validate phase in Docker mode).
    /// Clones the task branch and runs `cmd` directly via bash — no claude agent involved.
    async fn run_test_in_container(&self, task: &Task, cmd: &str) -> Result<TestOutput> {
        self.ensure_tmp_capacity(task.id, "run_test_in_container")?;
        let timeout = std::time::Duration::from_secs(self.config.agent_timeout_s.max(300) as u64);
        let repo_name = std::path::Path::new(&task.repo_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let branch = format!("task-{}", task.id);
        let host_mirror = format!("{}/mirrors/{repo_name}.git", self.config.data_dir);
        let container_mirror = format!("/mirrors/{repo_name}.git");

        // Shallow clone — test containers only need the branch tip.
        // Wrap a value in single quotes with internal single quotes escaped.
        fn sq(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let repo_url_q = sq(&task.repo_path);
        let branch_q = sq(&branch);
        let cmd_q = sq(cmd);
        let container_mirror_q = sq(&container_mirror);
        let clone_cmd = if std::path::Path::new(&host_mirror).exists() {
            format!(
                "git clone --depth 1 --single-branch --reference {container_mirror_q} {repo_url_q} /workspace/repo"
            )
        } else {
            format!("git clone --depth 1 --single-branch {repo_url_q} /workspace/repo")
        };
        let bash_script = format!(
            "set -e; {clone_cmd} && cd /workspace/repo && git checkout {branch_q} && {cmd_q}"
        );
        let bash_cmd = vec!["bash".to_string(), "-c".to_string(), bash_script];

        let mut binds: Vec<(String, String, bool)> = Vec::new();
        if std::path::Path::new(&host_mirror).exists() {
            binds.push((host_mirror, container_mirror, true));
        }
        let binds_ref: Vec<(&str, &str, bool)> = binds
            .iter()
            .map(|(h, c, ro)| (h.as_str(), c.as_str(), *ro))
            .collect();
        let volumes_owned: Vec<(String, String)> = vec![
            (
                format!("borg-cache-{repo_name}-target"),
                "/workspace/repo/target".to_string(),
            ),
            (
                format!("borg-cache-{repo_name}-cargo-registry"),
                "/home/bun/.cargo/registry".to_string(),
            ),
        ];
        let volumes_ref: Vec<(&str, &str)> = volumes_owned
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();

        let network = if self.agent_network_available {
            Some(Sandbox::AGENT_NETWORK)
        } else {
            None
        };
        let output = tokio::time::timeout(
            timeout,
            Sandbox::docker_command(
                &self.config.container.image,
                &binds_ref,
                &volumes_ref,
                "",
                &bash_cmd,
                &[],
                self.config.container.memory_mb,
                self.config.container.cpus,
                network,
            )
            .kill_on_drop(true)
            .output(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "run_test_in_container timed out after {}s",
                timeout.as_secs()
            )
        })?
        .context("run_test_in_container")?;

        Ok(TestOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(1),
        })
    }

    // ── Notify + event broadcast ──────────────────────────────────────────

    pub fn notify(&self, chat_id: &str, message: &str) {
        if chat_id.is_empty() {
            return;
        }
        self.emit(PipelineEvent::Notify {
            chat_id: chat_id.to_string(),
            message: message.to_string(),
        });
    }

    fn emit(&self, event: PipelineEvent) {
        let _ = self.event_tx.send(event);
    }
}

fn is_negative_sign_recommendation(normalized: &str) -> bool {
    legal_guards::is_negative_sign_recommendation(normalized)
}

fn detect_benchmark_clarification_escape(text: &str) -> Option<String> {
    legal_guards::detect_benchmark_clarification_escape(text)
}

fn issue_seed_marker(url: &str) -> String {
    format!("Source issue: {}", url.trim())
}

fn trim_issue_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "No issue body provided.".to_string();
    }
    const MAX_CHARS: usize = 2000;
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_string();
    }
    let clipped: String = trimmed.chars().take(MAX_CHARS).collect();
    format!("{clipped}...")
}

fn failure_repeat_block_threshold(error: &str) -> u32 {
    legal_guards::failure_repeat_block_threshold(error)
}

// ── Private helpers ───────────────────────────────────────────────────────────

pub(crate) struct TestOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryClass {
    Resource,
    Transient,
    Conflict,
    Authentication,
    Other,
}

fn container_result_as_test_output(
    results: &[ContainerTestResult],
    phase: &str,
) -> Option<TestOutput> {
    results
        .iter()
        .find(|r| r.phase == phase)
        .map(|r| TestOutput {
            stdout: r.output.clone(),
            stderr: String::new(),
            exit_code: r.exit_code,
        })
}

fn extract_blocks(text: &str, start_marker: &str, end_marker: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find(start_marker) {
        remaining = &remaining[start + start_marker.len()..];
        if let Some(end) = remaining.find(end_marker) {
            blocks.push(remaining[..end].trim().to_string());
            remaining = &remaining[end + end_marker.len()..];
        } else {
            break;
        }
    }
    blocks
}

fn tmp_inode_usage_percent(path: &str) -> Option<f64> {
    tmp_health(path).map(|h| h.inode_used_pct)
}

#[derive(Debug, Clone, Copy)]
struct TmpHealth {
    inode_used_pct: f64,
    free_inodes: u64,
    free_bytes: u64,
}

fn tmp_health(path: &str) -> Option<TmpHealth> {
    let c_path = CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat as *mut libc::statvfs) };
    if rc != 0 || stat.f_files == 0 {
        return None;
    }
    let used = stat.f_files.saturating_sub(stat.f_ffree);
    let inode_used_pct = (used as f64) * 100.0 / (stat.f_files as f64);
    Some(TmpHealth {
        inode_used_pct,
        free_inodes: stat.f_ffree,
        free_bytes: stat.f_bavail.saturating_mul(stat.f_frsize),
    })
}

fn cleanup_tmp_prefixes(base: &str, prefixes: &[&str]) -> usize {
    let mut removed = 0usize;
    let Ok(entries) = std::fs::read_dir(base) else {
        return removed;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !prefixes.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let path = entry.path();
        let res = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if res.is_ok() {
            removed += 1;
        }
    }
    removed
}

fn classify_retry_error(error: &str) -> RetryClass {
    let err = error.to_ascii_lowercase();
    if err.contains("invalid authentication credentials")
        || err.contains("\"type\":\"authentication_error\"")
        || err.contains("authentication_error")
        || err.contains("unauthorized")
        || (err.contains("api key")
            && (err.contains("invalid") || err.contains("missing") || err.contains("incorrect")))
        || (err.contains("oauth token")
            && (err.contains("invalid") || err.contains("expired") || err.contains("revoked")))
    {
        return RetryClass::Authentication;
    }
    if err.contains("no space left on device")
        || err.contains("failed to copy file")
        || err.contains("inode")
        || err.contains("cannot create temp")
        || err.contains("resource temporarily unavailable")
        || err.contains("too many open files")
    {
        return RetryClass::Resource;
    }
    if err.contains("could not resolve host")
        || err.contains("temporary failure in name resolution")
        || err.contains("network is unreachable")
        || err.contains("connection reset")
        || err.contains("timed out")
        || err.contains("timeout")
        || err.contains("rate limit")
        || err.contains("http 502")
        || err.contains("http 503")
    {
        return RetryClass::Transient;
    }
    if err.contains("merge conflict")
        || err.contains("behind main")
        || err.contains("not mergeable")
        || err.contains("could not apply")
        || err.contains("conflict")
    {
        return RetryClass::Conflict;
    }
    RetryClass::Other
}

#[derive(Debug, Clone)]
struct ComplianceFinding {
    check_id: String,
    severity: &'static str,
    issue: String,
    source_url: String,
    as_of: String,
}

use legal_guards::LegalRetrievalTrace;

fn legal_retrieval_protocol_trigger(
    task: &Task,
    phase: &PhaseConfig,
    stats: &crate::db::ProjectFileStats,
) -> Option<&'static str> {
    legal_guards::legal_retrieval_protocol_trigger(task, phase, stats)
}

fn prior_retrieval_protocol_passed_from_structured_data(value: &serde_json::Value) -> Option<bool> {
    legal_guards::prior_retrieval_protocol_passed_from_structured_data(value)
}

fn should_reuse_prior_retrieval_pass(
    task: &Task,
    prior_passed: bool,
    current_passed: bool,
) -> bool {
    legal_guards::should_reuse_prior_retrieval_pass(task, prior_passed, current_passed)
}

fn should_offer_retrieval_reuse_guidance(task: &Task, prior_passed: bool) -> bool {
    legal_guards::should_offer_retrieval_reuse_guidance(task, prior_passed)
}

fn clarification_resume_question(
    prior_report: Option<&serde_json::Value>,
    last_error: &str,
) -> Option<String> {
    legal_guards::clarification_resume_question(prior_report, last_error)
}

fn inspect_legal_retrieval_trace(raw_stream: &str) -> LegalRetrievalTrace {
    legal_guards::inspect_legal_retrieval_trace(raw_stream)
}

fn run_compliance_pack(profile: &str, text: &str) -> Vec<ComplianceFinding> {
    let as_of = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if text.trim().is_empty() {
        return vec![ComplianceFinding {
            check_id: "output_present".into(),
            severity: "high",
            issue: "No prior phase output found to evaluate.".into(),
            source_url: "".into(),
            as_of,
        }];
    }

    let lower = text.to_lowercase();
    let mut findings = Vec::new();

    if !lower.contains("regulatory considerations") {
        findings.push(ComplianceFinding {
            check_id: "regulatory_section".into(),
            severity: "medium",
            issue: "Missing `Regulatory Considerations` section.".into(),
            source_url: "".into(),
            as_of: as_of.clone(),
        });
    }
    if !(lower.contains("as of ") || lower.contains("as-of")) {
        findings.push(ComplianceFinding {
            check_id: "as_of_date".into(),
            severity: "medium",
            issue: "Missing an explicit as-of date for regulatory statements.".into(),
            source_url: "".into(),
            as_of: as_of.clone(),
        });
    }
    if !(lower.contains("http://") || lower.contains("https://")) {
        findings.push(ComplianceFinding {
            check_id: "source_links".into(),
            severity: "high",
            issue: "Missing source URLs for regulatory references.".into(),
            source_url: "".into(),
            as_of: as_of.clone(),
        });
    }

    match profile {
        "uk_sra" => {
            if !(lower.contains("sra") || lower.contains("solicitors regulation authority")) {
                findings.push(ComplianceFinding {
                    check_id: "uk_sra_reference".into(),
                    severity: "high",
                    issue: "UK profile selected but no SRA reference found.".into(),
                    source_url: "https://www.sra.org.uk/solicitors/standards-regulations/".into(),
                    as_of: as_of.clone(),
                });
            }
        },
        "us_prof_resp" => {
            if !(lower.contains("model rule")
                || lower.contains("professional conduct")
                || lower.contains("state bar"))
            {
                findings.push(ComplianceFinding {
                    check_id: "us_model_rules_reference".into(),
                    severity: "high",
                    issue: "US profile selected but no Model Rules/state professional-conduct reference found.".into(),
                    source_url: "https://www.americanbar.org/groups/professional_responsibility/publications/model_rules_of_professional_conduct/".into(),
                    as_of: as_of.clone(),
                });
            }
        },
        _ => {
            findings.push(ComplianceFinding {
                check_id: "profile_supported".into(),
                severity: "high",
                issue: format!(
                    "Unknown compliance profile `{profile}` (supported: uk_sra, us_prof_resp)."
                ),
                source_url: "".into(),
                as_of,
            });
        },
    }

    findings
}

fn compliance_should_block(enforcement: &str, findings: &[ComplianceFinding]) -> bool {
    !findings.is_empty() && enforcement == "block"
}

fn extract_field(block: &str, field: &str) -> Option<String> {
    let mut lines = block.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix(field) {
            let mut parts = vec![rest.trim()];
            // Collect continuation lines until the next field (word: pattern)
            while let Some(&next) = lines.peek() {
                if looks_like_field_key(next) {
                    break;
                }
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                lines.next();
            }
            let val: Vec<&str> = parts.into_iter().filter(|s| !s.is_empty()).collect();
            if !val.is_empty() {
                return Some(val.join("\n"));
            }
        }
    }
    None
}

fn parse_triage_item(
    item: &serde_json::Value,
) -> Option<(i64, i64, i64, i64, i64, i64, &str, bool)> {
    let get_i64 = |k: &str| item.get(k).and_then(|v| v.as_i64());
    let p_id = get_i64("id")?;
    let impact = get_i64("impact")?;
    let feasibility = get_i64("feasibility")?;
    let risk = get_i64("risk")?;
    let effort = get_i64("effort")?;
    let score = get_i64("score")?;
    let reasoning = item.get("reasoning").and_then(|v| v.as_str()).unwrap_or("");
    let should_dismiss = item
        .get("dismiss")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Some((
        p_id,
        impact,
        feasibility,
        risk,
        effort,
        score,
        reasoning,
        should_dismiss,
    ))
}

fn no_merge_guardrail_baseline(
    queued_count: i64,
    last_release_ts: i64,
    backlog_started_ts: i64,
    now: i64,
) -> (Option<i64>, i64) {
    if queued_count <= 0 {
        return (None, 0);
    }
    if last_release_ts > 0 {
        return (Some(last_release_ts), 0);
    }
    let started = if backlog_started_ts > 0 {
        backlog_started_ts
    } else {
        now
    };
    (Some(started), started)
}

/// Collect session directory paths under `sessions_dir` that are stale and
/// eligible for removal.
///
/// A directory named `task-{N}` is stale when:
/// - It is not in `skip_ids` (i.e. not currently in-flight), AND
/// - Its age (seconds since task creation, or since mtime if the task is not
///   in the DB) is >= `max_age_secs`.
///
/// Exposed as a free function so it can be unit-tested without a Pipeline.
pub fn collect_stale_session_dirs(
    sessions_dir: &str,
    now_secs: i64,
    max_age_secs: i64,
    skip_ids: &HashSet<i64>,
    task_created_at: impl Fn(i64) -> Option<i64>,
) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return vec![];
    };
    let mut stale = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let Some(id_str) = name_str.strip_prefix("task-") else {
            continue;
        };
        let Ok(task_id) = id_str.parse::<i64>() else {
            continue;
        };
        if skip_ids.contains(&task_id) {
            continue;
        }
        let age_secs = match task_created_at(task_id) {
            Some(created_at) => now_secs.saturating_sub(created_at),
            None => {
                // Orphaned dir: fall back to filesystem mtime
                entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| now_secs.saturating_sub(d.as_secs() as i64))
                    .unwrap_or(max_age_secs + 1) // unknown age → treat as stale
            },
        };
        if age_secs >= max_age_secs {
            stale.push(entry.path());
        }
    }
    stale
}

fn looks_like_field_key(line: &str) -> bool {
    let trimmed = line.trim();
    if let Some(colon) = trimmed.find(':') {
        let key = &trimmed[..colon];
        !key.is_empty()
            && !key.contains(' ')
            && key.chars().next().is_some_and(|c| c.is_alphabetic())
    } else {
        false
    }
}

#[cfg(test)]
mod seeding_toctou_tests {
    use std::{
        collections::HashSet,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use tokio::sync::Mutex;

    /// Replicates the fixed "check-and-set" logic so we can test it in
    /// isolation without constructing a full Pipeline.
    async fn try_activate_seeding(
        in_flight: &Mutex<HashSet<i64>>,
        seeding_active: &AtomicBool,
    ) -> bool {
        let guard = in_flight.lock().await;
        if guard.is_empty() {
            seeding_active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        } else {
            false
        }
    }

    #[tokio::test]
    async fn seeding_does_not_start_when_in_flight_is_nonempty() {
        let in_flight = Mutex::new(HashSet::from([42i64]));
        let seeding_active = AtomicBool::new(false);

        let activated = try_activate_seeding(&in_flight, &seeding_active).await;

        assert!(
            !activated,
            "should not activate seeding while tasks are in-flight"
        );
        assert!(
            !seeding_active.load(Ordering::Acquire),
            "seeding_active must stay false"
        );
    }

    #[tokio::test]
    async fn seeding_starts_when_in_flight_is_empty() {
        let in_flight = Mutex::new(HashSet::new());
        let seeding_active = AtomicBool::new(false);

        let activated = try_activate_seeding(&in_flight, &seeding_active).await;

        assert!(
            activated,
            "should activate seeding when no tasks are in-flight"
        );
        assert!(
            seeding_active.load(Ordering::Acquire),
            "seeding_active must be set to true"
        );
    }

    #[tokio::test]
    async fn seeding_does_not_double_start_when_already_active() {
        let in_flight = Mutex::new(HashSet::new());
        let seeding_active = AtomicBool::new(true); // already running

        let activated = try_activate_seeding(&in_flight, &seeding_active).await;

        assert!(!activated, "CAS must fail when seeding is already active");
        assert!(
            seeding_active.load(Ordering::Acquire),
            "seeding_active must remain true"
        );
    }

    /// Regression: the in_flight lock must be held during the CAS.
    /// Simulate the race: after acquiring the lock and confirming emptiness,
    /// a concurrent task insertion should not be possible before the CAS
    /// completes because we hold the same lock.
    #[tokio::test]
    async fn in_flight_lock_held_prevents_concurrent_insertion() {
        let in_flight = Arc::new(Mutex::new(HashSet::new()));
        let seeding_active = Arc::new(AtomicBool::new(false));

        // Spawn a task that holds the in_flight lock and tries to insert
        // while try_activate_seeding is in its critical section.
        let in_flight2 = Arc::clone(&in_flight);
        let seeding_active2 = Arc::clone(&seeding_active);

        // First: activate seeding (acquires + holds lock, does CAS, drops lock).
        let activated = try_activate_seeding(&in_flight, &seeding_active).await;
        assert!(activated);

        // Now insert a task into in_flight to simulate a concurrent dispatch.
        in_flight2.lock().await.insert(99);

        // seeding_active is already true; a second call must fail even though
        // in_flight is now non-empty (belt-and-suspenders).
        let activated2 = try_activate_seeding(&in_flight2, &seeding_active2).await;
        assert!(
            !activated2,
            "must not activate again while seeding is running"
        );
    }
}

#[cfg(test)]
mod legal_retrieval_protocol_tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{
        clarification_resume_question, inspect_legal_retrieval_trace,
        legal_retrieval_protocol_trigger, prior_retrieval_protocol_passed_from_structured_data,
        should_offer_retrieval_reuse_guidance, should_reuse_prior_retrieval_pass,
    };
    use crate::{
        db::ProjectFileStats,
        types::{PhaseConfig, Task},
    };

    fn sample_task(task_type: &str, title: &str, description: &str) -> Task {
        Task {
            id: 1,
            title: title.to_string(),
            description: description.to_string(),
            repo_path: String::new(),
            branch: String::new(),
            status: "implement".into(),
            attempt: 0,
            max_attempts: 3,
            last_error: String::new(),
            created_by: "test".into(),
            notify_chat: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            session_id: String::new(),
            mode: "lawborg".into(),
            backend: String::new(),
            workspace_id: 0,
            project_id: 42,
            task_type: task_type.to_string(),
            requires_exhaustive_corpus_review: false,
            started_at: None,
            completed_at: None,
            duration_secs: None,
            review_status: None,
            revision_count: 0,
            chat_thread: String::new(),
        }
    }

    #[test]
    fn inspects_mcp_tool_trace_for_exhaustive_protocol() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"get_document_categories","input":{"project_id":42}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"list_documents","input":{"project_id":42}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"search_documents","input":{"query":"indemnification clause","project_id":42}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"search_documents","input":{"query":"hold harmless obligation","project_id":42}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"check_coverage","input":{"query":"indemnification clause","project_id":42}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"read_document","input":{"file_id":7,"project_id":42}}]}}"#;

        let trace = inspect_legal_retrieval_trace(raw);

        assert_eq!(trace.category_calls, 1);
        assert_eq!(trace.inventory_calls, 1);
        assert_eq!(trace.search_calls, 2);
        assert_eq!(trace.coverage_calls, 1);
        assert_eq!(trace.full_document_reads, 1);
        assert_eq!(trace.distinct_search_queries.len(), 2);
    }

    #[test]
    fn inspects_webfetch_and_staged_reads_for_exhaustive_protocol() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"WebFetch","input":{"url":"http://127.0.0.1:3131/api/borgsearch/files?project_id=42&limit=50"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"WebFetch","input":{"url":"http://127.0.0.1:3131/api/borgsearch/query?q=limitation%20of%20liability&project_id=42"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"WebFetch","input":{"url":"http://127.0.0.1:3131/api/borgsearch/query?q=consequential%20damages&project_id=42"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"WebFetch","input":{"url":"http://127.0.0.1:3131/api/borgsearch/coverage?q=limitation%20of%20liability&project_id=42"}}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"project_files/01-master-services-agreement.txt"}}]}}"#;

        let trace = inspect_legal_retrieval_trace(raw);

        assert_eq!(trace.inventory_calls, 1);
        assert_eq!(trace.search_calls, 2);
        assert_eq!(trace.coverage_calls, 1);
        assert_eq!(trace.full_document_reads, 1);
        assert_eq!(trace.distinct_search_queries.len(), 2);
        assert!(trace
            .search_queries
            .iter()
            .any(|q| q.contains("limitation%20of%20liability")));
    }

    #[test]
    fn only_enforces_for_exhaustive_legal_tasks_with_project_corpus() {
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };
        let stats = ProjectFileStats {
            project_id: 42,
            text_files: 5,
            ..Default::default()
        };
        let task = sample_task(
            "contract_review",
            "Review vendor agreement",
            "Review the legal documents in this repository for playbook deviations.",
        );
        assert_eq!(
            legal_retrieval_protocol_trigger(&task, &phase, &stats),
            Some("heuristic_task_type")
        );

        let non_exhaustive = sample_task(
            "",
            "Research Delaware implied covenant doctrine",
            "Find recent authorities on the implied covenant of good faith.",
        );
        assert_eq!(
            legal_retrieval_protocol_trigger(&non_exhaustive, &phase, &stats),
            None
        );
    }

    #[test]
    fn explicit_exhaustive_flag_overrides_heuristics() {
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };
        let stats = ProjectFileStats {
            project_id: 42,
            text_files: 5,
            ..Default::default()
        };
        let mut task = sample_task(
            "",
            "Research Delaware implied covenant doctrine",
            "Find recent authorities on the implied covenant of good faith.",
        );
        task.requires_exhaustive_corpus_review = true;

        assert_eq!(
            legal_retrieval_protocol_trigger(&task, &phase, &stats),
            Some("explicit")
        );
    }

    #[test]
    fn reads_prior_retrieval_protocol_pass_state() {
        let payload = json!({
            "retrieval_protocol": {
                "passed": true
            }
        });

        assert_eq!(
            prior_retrieval_protocol_passed_from_structured_data(&payload),
            Some(true)
        );
    }

    #[test]
    fn clarification_resume_can_reuse_prior_passed_retrieval() {
        let mut task = sample_task(
            "benchmark_analysis",
            "legal-ew-003",
            "Benchmark clarification retry",
        );
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 1;
        task.last_error = "Material fact missing: runtime setting unresolved.\n\nQuestion: Is GenAssist enabled on the live BoroughCare queue?"
            .into();

        assert!(
            should_reuse_prior_retrieval_pass(&task, true, false),
            "clarification-driven retry should be able to reuse prior exhaustive review"
        );
        assert!(
            should_offer_retrieval_reuse_guidance(&task, true),
            "clarification-driven retries should also get prompt-level reuse guidance"
        );
        let prior_report = json!({});
        assert_eq!(
            clarification_resume_question(Some(&prior_report), &task.last_error).as_deref(),
            Some("Is GenAssist enabled on the live BoroughCare queue?")
        );
    }

    #[test]
    fn clarification_resume_reuses_prior_pass_with_non_colon_material_fact_prefix() {
        let mut task = sample_task(
            "benchmark_analysis",
            "legal-ew-003",
            "Benchmark clarification retry",
        );
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 2;
        task.last_error = "Material fact missing — Mariner notice position\n\nQuestion: Has either side served notice and when does it expire?"
            .into();

        assert!(
            should_reuse_prior_retrieval_pass(&task, true, false),
            "clarification retries should not depend on a colon after the material-fact prefix"
        );
    }

    #[test]
    fn ordinary_retry_reuses_prior_passed_retrieval() {
        let mut task = sample_task("benchmark_analysis", "legal-ew-003", "Ordinary retry");
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 1;
        task.last_error = "Compile fix failed".into();

        assert!(
            should_reuse_prior_retrieval_pass(&task, true, false),
            "retries should reuse prior retrieval pass since corpus is unchanged"
        );
    }

    #[test]
    fn clarification_guard_retry_can_reuse_prior_passed_retrieval() {
        let mut task = sample_task(
            "benchmark_analysis",
            "legal-ew-003",
            "Clarification guard retry",
        );
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 1;
        task.last_error = "Benchmark clarification guard failed.\nThe task output still treats an unresolved pre-sign/pre-close fact as a caveat instead of blocking for clarification."
            .into();

        assert!(
            should_reuse_prior_retrieval_pass(&task, true, false),
            "clarification-guard retries should not need to rerun an already-passed exhaustive review"
        );
    }

    #[test]
    fn summarized_fresh_retry_still_reuses_prior_passed_retrieval() {
        let mut task = sample_task(
            "benchmark_analysis",
            "legal-ew-003",
            "Summarized clarification retry",
        );
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 4;
        task.last_error = "FRESH RETRY — previous approaches failed. Summary of attempts:\n\
\n\
Attempt 1 (implement): blocked clarification already happened.\n\
\n\
Latest error:\n\
Benchmark clarification guard failed.\n\
The task output still treats an unresolved pre-sign/pre-close fact as a caveat instead of blocking for clarification."
            .into();

        assert!(
            should_reuse_prior_retrieval_pass(&task, true, false),
            "fresh-retry summaries should preserve clarification-driven retrieval reuse"
        );
    }

    #[test]
    fn clarification_guard_retry_offers_prompt_level_reuse_guidance() {
        let mut task = sample_task(
            "benchmark_analysis",
            "legal-ew-003",
            "Clarification guard retry",
        );
        task.mode = "legal".into();
        task.requires_exhaustive_corpus_review = true;
        task.attempt = 1;
        task.last_error = "Benchmark clarification guard failed.\nThe task output still treats an unresolved pre-sign/pre-close fact as a caveat instead of blocking for clarification."
            .into();

        assert!(
            should_offer_retrieval_reuse_guidance(&task, true),
            "clarification-guard retries should get prompt-level reuse guidance too"
        );
        let prior_report = json!({});
        assert!(
            clarification_resume_question(Some(&prior_report), &task.last_error).is_none(),
            "guard failures do not carry a question unless the error text includes one"
        );
    }

    #[test]
    fn clarification_resume_question_prefers_persisted_benchmark_state() {
        let prior_report = json!({
            "benchmark_state": {
                "status": "blocked_for_clarification",
                "question": "Does the full call-off contain a separate CoC consent?"
            }
        });

        assert_eq!(
            clarification_resume_question(
                Some(&prior_report),
                "Material fact missing\n\nQuestion: Old question from last_error"
            )
            .as_deref(),
            Some("Does the full call-off contain a separate CoC consent?")
        );
    }
}

#[cfg(test)]
mod phase_completion_verdict_tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::tempdir;

    use super::Pipeline;
    use crate::types::{PhaseCompletionVerdict, PhaseConfig, Task};

    fn sample_task() -> Task {
        Task {
            id: 7,
            title: "Implement gate".into(),
            description: "Ensure the phase gate is robust.".into(),
            repo_path: String::new(),
            branch: String::new(),
            status: "implement".into(),
            attempt: 2,
            max_attempts: 5,
            last_error: String::new(),
            created_by: "test".into(),
            notify_chat: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            session_id: String::new(),
            mode: "sweborg".into(),
            backend: String::new(),
            workspace_id: 0,
            project_id: 0,
            task_type: String::new(),
            requires_exhaustive_corpus_review: false,
            started_at: None,
            completed_at: None,
            duration_secs: None,
            review_status: None,
            revision_count: 0,
            chat_thread: String::new(),
        }
    }

    fn sample_phase() -> PhaseConfig {
        PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_phase_completion_verdict_from_workdir() {
        let dir = tempdir().expect("tempdir");
        let borg_dir = dir.path().join(".borg");
        fs::create_dir_all(&borg_dir).expect("create .borg");
        fs::write(
            borg_dir.join("phase-verdict.json"),
            r#"{"task_id":7,"phase":"implement","attempt":2,"gate_token":"gate-123","ready_to_advance":true,"rationale":"checked request","missing_requirements":[]}"#,
        )
        .expect("write verdict");

        let verdict = Pipeline::read_phase_completion_verdict(dir.path().to_str().unwrap())
            .expect("verdict should parse");

        assert_eq!(verdict.task_id, 7);
        assert_eq!(verdict.phase, "implement");
        assert_eq!(verdict.attempt, 2);
        assert_eq!(verdict.gate_token, "gate-123");
        assert!(verdict.ready_to_advance);
        assert_eq!(verdict.rationale, "checked request");
        assert!(verdict.missing_requirements.is_empty());
        assert!(
            !borg_dir.join("phase-verdict.json").exists(),
            "verdict file should be consumed so stale approvals cannot be reused"
        );
    }

    #[test]
    fn invalid_phase_completion_verdict_returns_none() {
        let dir = tempdir().expect("tempdir");
        let borg_dir = dir.path().join(".borg");
        fs::create_dir_all(&borg_dir).expect("create .borg");
        fs::write(borg_dir.join("phase-verdict.json"), "{not-json").expect("write invalid verdict");

        let verdict = Pipeline::read_phase_completion_verdict(dir.path().to_str().unwrap());

        assert!(verdict.is_none(), "malformed verdict should be rejected");
        assert!(
            !borg_dir.join("phase-verdict.json").exists(),
            "invalid verdict file should be removed after evaluation"
        );
    }

    #[test]
    fn mismatched_phase_completion_verdict_is_rejected() {
        let task = sample_task();
        let phase = sample_phase();
        let verdict = PhaseCompletionVerdict {
            task_id: task.id,
            phase: phase.name.clone(),
            attempt: task.attempt,
            gate_token: "old-token".into(),
            ready_to_advance: true,
            rationale: "checked request".into(),
            missing_requirements: Vec::new(),
        };

        let err =
            Pipeline::validate_phase_completion_verdict(&verdict, &task, &phase, "fresh-token")
                .expect_err("stale gate token must be rejected");

        assert!(
            err.contains("gate token mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn positive_phase_completion_verdict_cannot_list_missing_requirements() {
        let task = sample_task();
        let phase = sample_phase();
        let verdict = PhaseCompletionVerdict {
            task_id: task.id,
            phase: phase.name.clone(),
            attempt: task.attempt,
            gate_token: "fresh-token".into(),
            ready_to_advance: true,
            rationale: "checked request".into(),
            missing_requirements: vec!["still need tests".into()],
        };

        let err =
            Pipeline::validate_phase_completion_verdict(&verdict, &task, &phase, "fresh-token")
                .expect_err("inconsistent positive verdict must be rejected");

        assert!(
            err.contains("missing_requirements"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(test)]
mod benchmark_phase_state_tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::tempdir;

    use super::Pipeline;
    use crate::types::{
        AgentSignal, BenchmarkClaimState, BenchmarkPhaseState, BenchmarkUncertaintyState,
        PhaseConfig, Task,
    };

    fn sample_task() -> Task {
        Task {
            id: 17,
            title: "legal-ew-003".into(),
            description: "benchmark".into(),
            repo_path: String::new(),
            branch: "task-17".into(),
            status: "implement".into(),
            attempt: 2,
            max_attempts: 5,
            last_error: String::new(),
            created_by: "test".into(),
            notify_chat: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            session_id: String::new(),
            mode: "legal".into(),
            backend: String::new(),
            workspace_id: 0,
            project_id: 1,
            task_type: "benchmark_analysis".into(),
            requires_exhaustive_corpus_review: true,
            started_at: None,
            completed_at: None,
            duration_secs: None,
            review_status: None,
            revision_count: 0,
            chat_thread: String::new(),
        }
    }

    fn sample_phase() -> PhaseConfig {
        PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_benchmark_phase_state_from_workdir() {
        let dir = tempdir().expect("tempdir");
        let borg_dir = dir.path().join(".borg");
        fs::create_dir_all(&borg_dir).expect("create .borg");
        fs::write(
            borg_dir.join("benchmark-state.json"),
            r#"{"task_id":17,"phase":"implement","attempt":2,"gate_token":"gate-123","status":"ready","rationale":"No unresolved sign-dispositive fact remains.","clarification_type":"","material_fact":"","question":"","uncertainties":[],"claims":[]}"#,
        )
        .expect("write state");

        let state = Pipeline::read_benchmark_phase_state(dir.path().to_str().unwrap())
            .expect("benchmark-state should parse");

        assert_eq!(state.task_id, 17);
        assert_eq!(state.phase, "implement");
        assert_eq!(state.attempt, 2);
        assert_eq!(state.gate_token, "gate-123");
        assert_eq!(state.status, "ready");
        assert!(
            !borg_dir.join("benchmark-state.json").exists(),
            "benchmark-state file should be consumed so stale state cannot be reused"
        );
    }

    #[test]
    fn blocked_benchmark_state_requires_blocked_signal_and_matching_reason_code() {
        let task = sample_task();
        let phase = sample_phase();
        let state = BenchmarkPhaseState {
            task_id: task.id,
            phase: phase.name.clone(),
            attempt: task.attempt,
            gate_token: "gate-123".into(),
            status: "blocked_for_clarification".into(),
            rationale: "The full call-off is unavailable and could change the sign recommendation.".into(),
            clarification_type: "missing_complete_document".into(),
            material_fact: "Whether the full BoroughCare call-off contains a separate CoC consent right.".into(),
            question: "Does the full BoroughCare call-off contain a separate change-of-control consent right?".into(),
            uncertainties: vec![BenchmarkUncertaintyState {
                issue: "BoroughCare call-off completeness".into(),
                missing_fact: "Separate CoC clause in the missing call-off sections".into(),
                uncertainty_type: "missing_complete_document".into(),
                support_status: "partial_record".into(),
                operative_status: "unclear".into(),
                changes_sign: true,
                changes_close_only: false,
                allocable_to_spa_structure: false,
                requires_counterparty_input: false,
                requires_missing_document: true,
                depends_on_partial_record: true,
                recommended_treatment: "blocked_clarification".into(),
                justification: "The visible extract is incomplete and the missing sections could contain a separate consent trigger.".into(),
            }],
            claims: vec![],
        };

        let invalid_signal = AgentSignal {
            status: "blocked".into(),
            reason: "Material fact missing".into(),
            reason_code: String::new(),
            question: state.question.clone(),
        };
        let err = Pipeline::validate_benchmark_phase_state(
            &state,
            &task,
            &phase,
            "gate-123",
            &invalid_signal,
        )
        .expect_err("missing reason_code should be rejected");
        assert!(err.contains("reason_code"), "unexpected error: {err}");

        let valid_signal = AgentSignal {
            status: "blocked".into(),
            reason: "Material fact missing".into(),
            reason_code: "missing_complete_document".into(),
            question: state.question.clone(),
        };
        Pipeline::validate_benchmark_phase_state(&state, &task, &phase, "gate-123", &valid_signal)
            .expect("matching blocked signal should be accepted");
    }

    #[test]
    fn structured_state_guard_rejects_definitive_claim_on_unresolved_fact() {
        let task = sample_task();
        let phase = sample_phase();
        let state = BenchmarkPhaseState {
            task_id: task.id,
            phase: phase.name.clone(),
            attempt: task.attempt,
            gate_token: "gate-123".into(),
            status: "ready".into(),
            rationale: "Draft completed.".into(),
            clarification_type: String::new(),
            material_fact: String::new(),
            question: String::new(),
            uncertainties: vec![BenchmarkUncertaintyState {
                issue: "Live GenAssist runtime".into(),
                missing_fact: "Whether BoroughCare queues are live in GenAssist".into(),
                uncertainty_type: "operational_fact".into(),
                support_status: "unavailable".into(),
                operative_status: "unclear".into(),
                changes_sign: true,
                changes_close_only: false,
                allocable_to_spa_structure: false,
                requires_counterparty_input: false,
                requires_missing_document: false,
                depends_on_partial_record: false,
                recommended_treatment: "provisional_only".into(),
                justification: "The visible papers only show intended controls, not live runtime."
                    .into(),
            }],
            claims: vec![BenchmarkClaimState {
                claim: "Signing can proceed on current facts.".into(),
                claim_type: "sign_recommendation".into(),
                support_status: "inferred".into(),
                depends_on_unresolved_fact: true,
                safe_to_state_definitively: true,
                supporting_artifacts: vec!["DOC-005".into()],
            }],
        };

        let err = Pipeline::enforce_legal_benchmark_state_guard(&task, &phase, &state)
            .expect("state guard should reject when sign-changing uncertainty has no support");

        assert!(
            err.contains("changes the sign/close recommendation"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(test)]
mod legal_benchmark_clarification_guard_tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::tempdir;

    use super::{detect_benchmark_clarification_escape, failure_repeat_block_threshold, Pipeline};
    use crate::types::{PhaseConfig, Task};

    fn sample_task() -> Task {
        Task {
            id: 11,
            title: "legal-ew-003".into(),
            description: "benchmark".into(),
            repo_path: String::new(),
            branch: "task-1".into(),
            status: "implement".into(),
            attempt: 0,
            max_attempts: 5,
            last_error: String::new(),
            created_by: "test".into(),
            notify_chat: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            session_id: String::new(),
            mode: "legal".into(),
            backend: String::new(),
            workspace_id: 0,
            project_id: 1,
            task_type: "benchmark_analysis".into(),
            requires_exhaustive_corpus_review: true,
            started_at: None,
            completed_at: None,
            duration_secs: None,
            review_status: None,
            revision_count: 0,
            chat_thread: String::new(),
        }
    }

    #[test]
    fn detects_supportable_with_pre_sign_confirmations_escape_hatch() {
        let text = "Sign position: Sign on 13 March is supportable with two pre-sign seller confirmations.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect unresolved pre-sign confirmation language");

        assert!(
            excerpt.contains("supportable"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn does_not_flag_outputs_without_unresolved_pre_sign_confirmation_language() {
        let text = "Sign position: Do not sign until TitanBank written approval is in hand.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "clean blocked-style recommendation should not trip the guard"
        );
    }

    #[test]
    fn does_not_flag_recommendation_with_explicit_independence_signal() {
        let text = "The sign recommendation is stable whichever way each open question resolves. TitanBank Procurement/Legal confirmation and BoroughCare configuration do not depend on first receiving an answer before signing.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "recommendation explicitly declared independent of unresolved facts should not trigger guard"
        );
    }

    #[test]
    fn detects_questions_for_seller_routed_to_post_close_or_pre_close_remediation() {
        let text = "Proceed to sign. GenAssist configuration remains unconfirmed from the corpus, but that is a pre-close remediation action and the questions for seller do not change the recommendation.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect remediation-based escape hatch");

        assert!(
            excerpt.contains("pre-close remediation"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn detects_can_sign_subject_to_pre_sign_requirements_escape_hatch() {
        let text = "CAN SIGN on 13 March 2026, subject to the following. Pre-sign requirements: seller to provide written disclosure on the unresolved GenAssist question and produce the full BoroughCare call-off.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect can-sign subject-to phrasing");

        assert!(
            excerpt.contains("CAN SIGN"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn detects_signing_can_proceed_with_pre_sign_confirmation_escape_hatch() {
        let text = "Signing can proceed on 13 March 2026, subject to two pre-signing actions being taken today. Obtain seller written confirmation of the GenAssist live configuration for BoroughCare queue at runtime before signing. No hard blockers to signing have been identified on current facts.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect signing-can-proceed confirmation language");

        assert!(
            excerpt.contains("Signing can proceed"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn detects_recommended_position_sign_on_conditioned_on_answers_escape_hatch() {
        let text = "Recommended position: sign on 13 March subject to SPA protections. The BoroughCare recommendation is conditioned on management-presentation answers about the live GenAssist configuration and written approvals.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect sign-on conditioned-on-answers language");

        assert!(
            excerpt.contains("Recommended position: sign on 13 March"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn does_not_flag_same_in_all_scenarios_with_independence_signal() {
        let text = "The sign recommendation is the same in all scenarios. It does not depend on resolving the open factual questions before execution because the SPA architecture is calibrated for the worst case.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "independence signals ('same in all scenarios', 'does not depend on') should prevent guard from firing"
        );
    }

    #[test]
    fn does_not_flag_same_recommendation_with_whichever_way_signal() {
        let text = "The sign recommendation is the same whichever way the unresolved BoroughCare configuration question resolves. It is not a blocker and does not change the recommendation.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "independence signal ('whichever way') should prevent guard from firing"
        );
    }

    #[test]
    fn detects_no_sign_blockers_but_pre_sign_requirement_escape_hatch() {
        let text = "There are no absolute sign-blockers on the current corpus. The sign-and-fix route is viable. However, two issues must be resolved before sign.";

        let excerpt = detect_benchmark_clarification_escape(text)
            .expect("guard should detect no-sign-blockers with pre-sign requirements");

        assert!(
            excerpt.contains("no absolute sign-blockers")
                || excerpt.contains("sign-and-fix route is viable"),
            "unexpected excerpt: {excerpt}"
        );
    }

    #[test]
    fn does_not_flag_enforcement_status_risk_allocated_to_unqualified_warranty() {
        let text = "Recommended sign-off position: proceed only on the stated pre-sign BoroughCare remediation. Management is not aware of any step-in notice, suspension notice, breach notice, or enforcement communication, but the buyer has no independent enforcement-status confirmation. The SPA warranty on this point must not be qualified by a knowledge limitation and the specific indemnity covers any undisclosed pre-sign enforcement communication.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "enforcement-status caveat allocated to an unqualified warranty should not trip the clarification guard"
        );
    }

    #[test]
    fn does_not_flag_tail_diligence_items_that_explicitly_do_not_change_sign() {
        let text = "Mariner Card Services should be disclosed before sign to inform price mechanics, but the sign recommendation does not depend on how it resolves. Beacon Retail Finance and the other customers should be reviewed before sign as a matter of diligence hygiene; the sign recommendation holds regardless of what provisions are found because any consent requirements are managed as a CP or post-close item.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "non-dispositive tail diligence and price-mechanics items should not trip the clarification guard"
        );
    }

    #[test]
    fn benchmark_guard_reads_written_deliverables() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("advice_memo.md"),
            "Recommended sign-off position\n\nSign is supportable with pre-sign seller confirmation of the live GenAssist state.",
        )
        .expect("write advice memo");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        let error = Pipeline::enforce_legal_benchmark_clarification_guard(
            &task,
            &phase,
            dir.path().to_str().expect("path"),
            "",
        )
        .expect("deliverable text should trigger benchmark guard");

        assert!(
            error.contains("Benchmark clarification guard failed"),
            "unexpected error: {error}"
        );
        assert!(
            error.contains("advice_memo.md"),
            "unexpected error source: {error}"
        );
    }

    #[test]
    fn benchmark_guard_is_scoped_to_legal_benchmark_tasks() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("advice_memo.md"),
            "Sign is supportable with pre-sign seller confirmation.",
        )
        .expect("write advice memo");

        let mut task = sample_task();
        task.task_type = "contract_review".into();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        assert!(
            Pipeline::enforce_legal_benchmark_clarification_guard(
                &task,
                &phase,
                dir.path().to_str().expect("path"),
                ""
            )
            .is_none(),
            "non-benchmark legal tasks should not use the benchmark clarification guard"
        );
    }

    #[test]
    fn clarification_guard_failures_block_stuck_loops_faster() {
        assert_eq!(
            failure_repeat_block_threshold(
                "Benchmark clarification guard failed.\nThe task output still treats ..."
            ),
            2
        );
        assert_eq!(
            failure_repeat_block_threshold("ordinary transient error"),
            3
        );
    }

    #[test]
    fn does_not_flag_do_not_sign_recommendation_with_pre_sign_remediation() {
        let text = "Recommended sign-off position: Do not sign in the current confirmed state. \
            Two material breaches of the NorthCounty call-off are active. \
            Pre-sign action required: LedgerLoop must immediately disable GenAssist on \
            the BoroughCare queue profile and disclose the confirmed state to NorthCounty \
            before signing can be revisited.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "a definitive 'do not sign' recommendation with remediation steps should not trip the guard"
        );
    }

    #[test]
    fn does_not_flag_signing_not_supportable_with_pre_sign_conditions() {
        let text = "Sign position: signing is not supportable unless pre-sign conditions \
            are met. Seller confirmation of GenAssist state is required before sign can be revisited.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "a 'not supportable' recommendation should not trip the guard"
        );
    }

    #[test]
    fn does_not_flag_cannot_sign_with_open_questions() {
        let text = "Recommended sign-off position: cannot sign. There are open questions \
            that must be resolved as pre-sign conditions. Clarification on the TitanBank \
            approval notice status is pending.";

        assert!(
            detect_benchmark_clarification_escape(text).is_none(),
            "a 'cannot sign' recommendation should not trip the guard"
        );
    }

    #[test]
    fn does_not_flag_negative_recommendation_in_deliverable_file() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("advice_memo.md"),
            "## Recommended sign-off position\n\n\
             **Do not sign in the current confirmed state.** Pre-sign action required: \
             disable GenAssist and confirm NorthCounty disclosure before signing can be revisited.",
        )
        .expect("write advice memo");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        assert!(
            Pipeline::enforce_legal_benchmark_clarification_guard(
                &task,
                &phase,
                dir.path().to_str().expect("path"),
                ""
            )
            .is_none(),
            "deliverable with 'do not sign' recommendation should not trip the guard"
        );
    }

    #[test]
    fn does_not_flag_companion_file_when_bundle_has_negative_recommendation() {
        let dir = tempdir().expect("tempdir");
        // intake_note has trigger patterns but no negative recommendation
        fs::write(
            dir.path().join("intake_note.md"),
            "# Intake Note\n\n\
             Sign recommendation depends on confirming GenAssist status before signing.\n\
             Subject to pre-sign remediation actions.",
        )
        .expect("write intake note");
        // advice_memo has the negative recommendation
        fs::write(
            dir.path().join("advice_memo.md"),
            "## Recommended sign-off position\n\n\
             **Do not sign.** Two material breaches are active.",
        )
        .expect("write advice memo");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        assert!(
            Pipeline::enforce_legal_benchmark_clarification_guard(
                &task,
                &phase,
                dir.path().to_str().expect("path"),
                ""
            )
            .is_none(),
            "companion files should be exempt when the bundle contains a negative recommendation"
        );
    }

    #[test]
    fn skips_phase_output_when_deliverable_files_exist() {
        let dir = tempdir().expect("tempdir");
        // advice_memo recommends signing (positive) with pre-sign conditions
        fs::write(
            dir.path().join("advice_memo.md"),
            "## Sign position\n\n\
             Sign on 13 March is supportable. Subject to pre-sign confirmation \
             of GenAssist renewal status from seller.",
        )
        .expect("write advice memo");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        // Phase output echoes sign vocabulary from describing what was done
        let phase_output = "Retrieval protocol satisfied. Sign recommendation produced. \
            Pre-sign conditions identified and confirmation pending from seller.";

        let result = Pipeline::enforce_legal_benchmark_clarification_guard(
            &task,
            &phase,
            dir.path().to_str().expect("path"),
            phase_output,
        );
        // Guard should fire on advice_memo.md (the deliverable), not phase output
        if let Some(ref msg) = result {
            assert!(
                !msg.contains("Source: phase output"),
                "phase output should be skipped when deliverable files exist; got: {}",
                msg
            );
        }
    }

    #[test]
    fn does_not_flag_intake_note_when_advice_memo_exists() {
        let dir = tempdir().expect("tempdir");
        // intake_note naturally contains sign timing and unresolved facts
        fs::write(
            dir.path().join("intake_note.md"),
            "# Intake Note\n\n## Known facts\nSign on 13 March timetable.\n\
             ## Missing facts\nGenAssist runtime status not confirmed. \
             Subject to pre-sign confirmation from seller.",
        )
        .expect("write intake note");
        // advice_memo makes a clean recommendation
        fs::write(
            dir.path().join("advice_memo.md"),
            "## Recommended sign-off position\n\n\
             Sign tonight. TitanBank ST 7 managed as CP to close.",
        )
        .expect("write advice memo");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        let result = Pipeline::enforce_legal_benchmark_clarification_guard(
            &task,
            &phase,
            dir.path().to_str().expect("path"),
            "",
        );
        if let Some(ref msg) = result {
            assert!(
                !msg.contains("Source: intake_note.md"),
                "intake_note.md should not be checked; got: {}",
                msg
            );
        }
    }

    #[test]
    fn checks_phase_output_when_no_recommendation_files() {
        let dir = tempdir().expect("tempdir");
        // Only a non-recommendation file present
        fs::write(
            dir.path().join("intake_note.md"),
            "# Intake Note\nFact summary only.",
        )
        .expect("write intake note");

        let task = sample_task();
        let phase = PhaseConfig {
            name: "implement".into(),
            ..Default::default()
        };

        let phase_output = "Sign position: Sign on 13 March is supportable with \
            pre-sign confirmation of GenAssist status pending.";

        let result = Pipeline::enforce_legal_benchmark_clarification_guard(
            &task,
            &phase,
            dir.path().to_str().expect("path"),
            phase_output,
        );
        assert!(
            result.is_some(),
            "phase output should be checked when no recommendation files exist"
        );
        assert!(
            result.unwrap().contains("Source: phase output"),
            "should attribute to phase output"
        );
    }

    #[test]
    fn still_detects_supportable_escape_despite_pre_sign_language() {
        let text = "Sign position: Sign on 13 March is supportable with two pre-sign \
            seller confirmations on GenAssist and TitanBank.";

        assert!(
            detect_benchmark_clarification_escape(text).is_some(),
            "a 'supportable with pre-sign confirmations' escape should still be caught"
        );
    }

    #[test]
    fn classifies_provider_authentication_errors_separately() {
        let sample = r#"Failed to authenticate. API Error: 401 {"type":"error","error":{"type":"authentication_error","message":"Invalid authentication credentials"}}"#;
        assert_eq!(
            super::classify_retry_error(sample),
            super::RetryClass::Authentication
        );
    }

    #[test]
    fn session_dirs_do_not_collide_for_same_task_id_across_repo_paths() {
        let mut first = sample_task();
        first.repo_path = "/tmp/borg-a/.worktrees/task-1".into();

        let mut second = sample_task();
        second.repo_path = "/tmp/borg-b/.worktrees/task-1".into();

        assert_ne!(
            super::Pipeline::task_session_dir_rel(&first),
            super::Pipeline::task_session_dir_rel(&second),
            "fresh databases that reuse task ids must not share on-disk session state"
        );
    }
}

#[cfg(test)]
mod guardrail_alert_tests {
    use super::no_merge_guardrail_baseline;

    #[test]
    fn queue_absence_clears_no_merge_baseline() {
        let (baseline, next_started) = no_merge_guardrail_baseline(0, 0, 123, 900);

        assert_eq!(baseline, None);
        assert_eq!(next_started, 0);
    }

    #[test]
    fn first_backlog_without_merge_starts_timer_now() {
        let (baseline, next_started) = no_merge_guardrail_baseline(5, 0, 0, 900);

        assert_eq!(baseline, Some(900));
        assert_eq!(next_started, 900);
    }

    #[test]
    fn existing_backlog_without_merge_preserves_first_seen_time() {
        let (baseline, next_started) = no_merge_guardrail_baseline(5, 0, 600, 900);

        assert_eq!(baseline, Some(600));
        assert_eq!(next_started, 600);
    }

    #[test]
    fn last_release_takes_precedence_and_clears_backlog_timer() {
        let (baseline, next_started) = no_merge_guardrail_baseline(5, 750, 600, 900);

        assert_eq!(baseline, Some(750));
        assert_eq!(next_started, 0);
    }
}

