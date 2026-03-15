use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use super::*;

impl Pipeline {
    pub(crate) fn make_context(
        &self,
        task: &Task,
        work_dir: String,
        session_dir: String,
        pending_messages: Vec<(String, String)>,
    ) -> PhaseContext {
        let (claude_coauthor, user_coauthor) = self.git_coauthor_settings();
        let system_prompt_suffix =
            Self::build_system_prompt_suffix(claude_coauthor, &user_coauthor);
        let setup_script = if self.config.container.setup.is_empty() {
            String::new()
        } else {
            std::fs::canonicalize(&self.config.container.setup)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| self.config.container.setup.clone())
        };
        let mut api_keys = std::collections::HashMap::new();
        let workspace_owner =
            (task.workspace_id > 0).then(|| format!("workspace:{}", task.workspace_id));
        let user_owner = (!task.created_by.is_empty()).then(|| task.created_by.clone());
        for provider in [
            "lexisnexis",
            "lexmachina",
            "intelligize",
            "westlaw",
            "clio",
            "imanage",
            "netdocuments",
            "congress",
            "openstates",
            "canlii",
            "regulations_gov",
            "shovels",
            "plaid_client_id",
            "plaid_secret",
            "plaid_env",
        ] {
            let resolved = workspace_owner
                .as_deref()
                .and_then(|owner| self.db.get_api_key_exact(owner, provider).ok().flatten())
                .or_else(|| {
                    user_owner
                        .as_deref()
                        .and_then(|owner| self.db.get_api_key_exact(owner, provider).ok().flatten())
                })
                .or_else(|| self.db.get_api_key_exact("global", provider).ok().flatten());
            if let Some(key) = resolved {
                api_keys.insert(provider.to_string(), key);
            }
        }
        let mut disallowed_tools = self
            .db
            .get_config("pipeline_disallowed_tools")
            .ok()
            .flatten()
            .unwrap_or_default();
        let knowledge_query = format!("{} {} {}", task.title, task.description, task.task_type);
        let knowledge_files = self
            .db
            .list_knowledge_file_page(Some(&knowledge_query), None, None, 80, 0)
            .map(|(files, _)| files)
            .unwrap_or_default();
        let knowledge_dir = format!("{}/knowledge", self.config.data_dir);
        let knowledge_repo_paths = self
            .db
            .list_all_knowledge_repos()
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.status == "ready" && !r.local_path.is_empty())
            .map(|r| r.local_path)
            .collect::<Vec<_>>();
        let isolated = task.mode == "lawborg" || task.mode == "legal";
        if isolated
            && task.project_id > 0
            && self
                .db
                .is_session_privileged(task.project_id)
                .unwrap_or(false)
        {
            if !disallowed_tools.is_empty() {
                disallowed_tools.push(',');
            }
            disallowed_tools.push_str("web_search,WebFetch");
        }
        let agent_network = if isolated {
            Some(Sandbox::ISOLATED_NETWORK.to_string())
        } else if self.agent_network_available {
            Some(Sandbox::AGENT_NETWORK.to_string())
        } else {
            None
        };

        let chat_context = if !task.chat_thread.is_empty() && task.attempt == 0 {
            self.db
                .get_chat_messages(&task.chat_thread, 20)
                .unwrap_or_default()
                .into_iter()
                .map(|m| {
                    let sender = m.sender_name.unwrap_or_else(|| {
                        if m.is_from_me {
                            "assistant".into()
                        } else {
                            "user".into()
                        }
                    });
                    (sender, m.content)
                })
                .collect()
        } else {
            Vec::new()
        };

        let prior_report = self
            .db
            .get_task_structured_data(task.id)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        let prior_passed = prior_report
            .as_ref()
            .and_then(prior_retrieval_protocol_passed_from_structured_data)
            .unwrap_or(false);
        let clarification_resume_reuses_prior_review =
            should_offer_retrieval_reuse_guidance(task, prior_passed);
        let clarification_resume_question = if clarification_resume_reuses_prior_review {
            clarification_resume_question(prior_report.as_ref(), &task.last_error)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let gh_resolved = self.resolve_gh_token(&task.created_by);

        let system_api_token = std::fs::read_to_string(format!("{}/.api-token", self.config.data_dir))
            .unwrap_or_default()
            .trim()
            .to_string();
        let borg_api_token = self.resolve_agent_token(task, &system_api_token);

        PhaseContext {
            task: task.clone(),
            repo_config: self.repo_config(task),
            data_dir: self.config.data_dir.clone(),
            session_dir,
            work_dir,
            oauth_token: self.config.oauth_token.clone(),
            model: if self.config.model.is_empty() {
                self.config.pipeline.default_docker_model.clone()
            } else {
                self.config.model.clone()
            },
            pending_messages,
            phase_attempt: task.attempt,
            phase_gate_token: format!(
                "gate:{}:{}",
                task.id,
                Utc::now()
                    .timestamp_nanos_opt()
                    .unwrap_or_else(|| Utc::now().timestamp_micros() * 1_000)
            ),
            system_prompt_suffix,
            user_coauthor,
            stream_tx: None,
            setup_script,
            api_keys,
            disallowed_tools,
            knowledge_files,
            knowledge_dir,
            knowledge_repo_paths,
            agent_network,
            prior_research: Vec::new(),
            revision_count: task.revision_count,
            experimental_domains: self.config.pipeline.experimental_domains,
            isolated,
            borg_api_url: format!("http://127.0.0.1:{}", self.config.web.port),
            borg_api_token,
            chat_context,
            github_token: gh_resolved.0.clone(),
            github_token_is_user: gh_resolved.1,
            clarification_resume_reuses_prior_review,
            clarification_resume_question,
            custom_mcp_servers: {
                let mut servers: Vec<crate::types::CustomMcpServer> = self
                    .db
                    .get_enabled_custom_mcp_servers_resolved(task.workspace_id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, command, args, env)| crate::types::CustomMcpServer {
                        name,
                        command,
                        args,
                        env,
                    })
                    .collect();
                if let Some(google) = self.build_google_mcp_server(&task.created_by) {
                    servers.push(google);
                }
                servers
            },
            ms365_token: self.resolve_ms365_token(&task.created_by),
        }
    }

    /// Generate a per-user scoped agent token for user-created tasks,
    /// or fall back to the system admin token for system-created tasks.
    fn resolve_agent_token(&self, task: &Task, system_token: &str) -> String {
        const SYSTEM_CREATORS: &[&str] = &["seed", "proposal", "health-check", "observer"];
        let created_by = task.created_by.trim();
        if created_by.is_empty()
            || SYSTEM_CREATORS.contains(&created_by)
            || created_by.starts_with("cron:")
        {
            return system_token.to_string();
        }
        if self.jwt_secret.is_empty() {
            return system_token.to_string();
        }
        let user = self.db.get_user_by_username(created_by).ok().flatten();
        let Some((user_id, _, _, _, is_admin)) = user else {
            info!(
                task_id = task.id,
                created_by,
                "no DB user found for task creator, using system token"
            );
            return system_token.to_string();
        };
        let workspace_id = if task.workspace_id > 0 {
            task.workspace_id
        } else {
            self.db
                .get_user_default_workspace_id(user_id)
                .ok()
                .flatten()
                .unwrap_or(0)
        };
        let token = crate::token::generate_agent_token(
            &self.jwt_secret,
            user_id,
            created_by,
            workspace_id,
            is_admin,
            86400, // 24h TTL
        );
        if token.is_empty() {
            warn!(task_id = task.id, "agent token generation failed, using system token");
            return system_token.to_string();
        }
        token
    }

    fn resolve_ms365_token(&self, created_by: &str) -> String {
        if created_by.is_empty() {
            return String::new();
        }
        let user_id = self
            .db
            .get_user_by_username(created_by)
            .ok()
            .flatten()
            .map(|(id, _, _, _, _)| id)
            .unwrap_or(0);
        if user_id == 0 {
            return String::new();
        }
        // Synchronous check — if token exists and not expired, return it.
        // Full async refresh happens at agent dispatch time.
        let encrypted = self
            .db
            .get_user_setting(user_id, "ms365_access_token")
            .ok()
            .flatten()
            .unwrap_or_default();
        if encrypted.is_empty() {
            return String::new();
        }
        crate::db::Db::decrypt_secret(&encrypted)
    }

    fn build_google_mcp_server(&self, created_by: &str) -> Option<crate::types::CustomMcpServer> {
        if created_by.is_empty() {
            return None;
        }
        let user_id = self
            .db
            .get_user_by_username(created_by)
            .ok()
            .flatten()
            .map(|(id, _, _, _, _)| id)?;
        let encrypted_refresh = self
            .db
            .get_user_setting(user_id, "google_refresh_token")
            .ok()
            .flatten()?;
        let refresh_token = crate::db::Db::decrypt_secret(&encrypted_refresh);
        if refresh_token.is_empty() {
            return None;
        }
        let client_id = self.db.get_config("google_client_id").ok().flatten().unwrap_or_default();
        let client_secret = self.db.get_config("google_client_secret").ok().flatten().unwrap_or_default();
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        let mut env = std::collections::HashMap::new();
        env.insert("GOOGLE_WORKSPACE_CLIENT_ID".into(), client_id);
        env.insert("GOOGLE_WORKSPACE_CLIENT_SECRET".into(), client_secret);
        env.insert("GOOGLE_WORKSPACE_REFRESH_TOKEN".into(), refresh_token);
        Some(crate::types::CustomMcpServer {
            name: "google-workspace".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@alanxchen/google-workspace-mcp".into()],
            env,
        })
    }

    fn clear_session_provider_credentials(session_dir: &str, provider: &str) {
        let path = match provider {
            PROVIDER_CLAUDE => Path::new(session_dir).join(".claude"),
            PROVIDER_OPENAI => Path::new(session_dir).join(".codex"),
            _ => return,
        };
        let _ = std::fs::remove_dir_all(path);
    }

    async fn prepare_linked_agent_credentials(
        &self,
        task: &Task,
        backend_name: &str,
        ctx: &mut PhaseContext,
    ) -> Result<()> {
        let provider = match backend_name {
            "claude" | "agent-sdk" => PROVIDER_CLAUDE,
            "codex" => PROVIDER_OPENAI,
            _ => return Ok(()),
        };
        let Some((user_id, _, _, _, _)) = self.db.get_user_by_username(&task.created_by)? else {
            return Ok(());
        };
        let Some(secret) = self.db.get_user_linked_credential(user_id, provider)? else {
            return Ok(());
        };
        if secret.entry.status != "connected" {
            return Ok(());
        }
        restore_bundle(&secret.bundle, Path::new(&ctx.session_dir))
            .context("restore linked credential bundle into task session")?;

        if should_revalidate(&secret.entry.last_validated_at, &secret.entry.expires_at) {
            let validation = validate_home(provider, Path::new(&ctx.session_dir)).await?;
            let now = Utc::now().to_rfc3339();
            if validation.ok {
                let refreshed_bundle = capture_bundle(provider, Path::new(&ctx.session_dir))
                    .context("capture refreshed linked credential bundle")?;
                self.db.update_user_linked_credential_state(
                    user_id,
                    provider,
                    &validation.auth_kind,
                    if validation.account_email.is_empty() {
                        &secret.entry.account_email
                    } else {
                        &validation.account_email
                    },
                    if validation.account_label.is_empty() {
                        &secret.entry.account_label
                    } else {
                        &validation.account_label
                    },
                    "connected",
                    &validation.expires_at,
                    &now,
                    "",
                    Some(&refreshed_bundle),
                )?;
            } else {
                self.db.update_user_linked_credential_state(
                    user_id,
                    provider,
                    if validation.auth_kind.is_empty() {
                        &secret.entry.auth_kind
                    } else {
                        &validation.auth_kind
                    },
                    if validation.account_email.is_empty() {
                        &secret.entry.account_email
                    } else {
                        &validation.account_email
                    },
                    if validation.account_label.is_empty() {
                        &secret.entry.account_label
                    } else {
                        &validation.account_label
                    },
                    "expired",
                    &validation.expires_at,
                    &now,
                    &validation.last_error,
                    None,
                )?;
                Self::clear_session_provider_credentials(&ctx.session_dir, provider);
                return Ok(());
            }
        }

        if provider == PROVIDER_CLAUDE {
            if let Some(token) = claude_oauth_token_from_home(Path::new(&ctx.session_dir)) {
                ctx.oauth_token = token;
            }
        }
        self.db
            .touch_user_linked_credential_used(user_id, provider)?;
        Ok(())
    }

    /// Increment attempt and set the retry status, or fail if attempts exhausted.
    /// After 3 failed attempts, clears the session ID to force a fresh start and
    /// builds a summary of previous attempts so the new session has context.
    pub(crate) fn fail_or_retry(&self, task: &Task, retry_status: &str, error: &str) -> Result<()> {
        if classify_retry_error(error) == RetryClass::Authentication {
            let reason = format!(
                "operator action required: backend authentication failed and automatic retry will not recover it: {error}"
            );
            self.db
                .update_task_status(task.id, "blocked", Some(&reason))?;
            let project_id = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let _ = self.db.log_event_full(
                Some(task.id),
                None,
                project_id,
                "pipeline",
                "task.authentication_blocked",
                &serde_json::json!({
                    "phase": retry_status,
                    "error": error,
                }),
            );
            return Ok(());
        }

        let repeat_count = self.note_failure_signature(task.id, retry_status, error);
        let stuck_loop_threshold = failure_repeat_block_threshold(error);
        if repeat_count >= stuck_loop_threshold {
            let reason = format!(
                "stuck loop detected in phase '{retry_status}' (same failure signature repeated {repeat_count}x, threshold {stuck_loop_threshold}): {error}"
            );
            self.db
                .update_task_status(task.id, "blocked", Some(&reason))?;
            let project_id = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let _ = self.db.log_event_full(
                Some(task.id),
                None,
                project_id,
                "pipeline",
                "task.stuck_loop_detected",
                &serde_json::json!({
                    "phase": retry_status,
                    "repeat_count": repeat_count,
                    "threshold": stuck_loop_threshold,
                    "error": error,
                }),
            );
            return Ok(());
        }

        self.db.increment_attempt(task.id)?;
        let current = self.db.get_task(task.id)?.unwrap_or_else(|| {
            // Fallback: use stale snapshot but with incremented attempt so check is correct
            let mut t = task.clone();
            t.attempt += 1;
            t
        });
        if current.attempt >= current.max_attempts {
            self.db.update_task_status(task.id, "failed", Some(error))?;
            let project_id = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let _ = self.db.log_event_full(
                Some(task.id),
                None,
                project_id,
                "pipeline",
                "task.failed_max_attempts",
                &serde_json::json!({
                    "phase": retry_status,
                    "attempt": current.attempt,
                    "max_attempts": current.max_attempts,
                    "error": error,
                }),
            );
        } else {
            if let Some(backoff_s) = self.retry_backoff_secs(task.id, current.attempt, error) {
                info!(
                    "task #{} retry backoff scheduled: {}s (attempt {} phase {})",
                    task.id, backoff_s, current.attempt, retry_status
                );
            }
            // After 3 attempts, force a fresh session with a summary of what was tried
            let error_ctx = if current.attempt >= 3 {
                self.db.update_task_session(task.id, "").ok();
                info!(
                    "task #{} attempt {} — clearing session for fresh start",
                    task.id, current.attempt
                );
                let project_id = if task.project_id > 0 {
                    Some(task.project_id)
                } else {
                    None
                };
                let _ = self.db.log_event_full(
                    Some(task.id),
                    None,
                    project_id,
                    "pipeline",
                    "task.session_reset_for_retry",
                    &serde_json::json!({
                        "phase": retry_status,
                        "attempt": current.attempt,
                    }),
                );
                self.build_retry_summary(task.id, error)
            } else {
                error.to_string()
            };
            self.db
                .update_task_status(task.id, retry_status, Some(&error_ctx))?;
            let project_id = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let _ = self.db.log_event_full(
                Some(task.id),
                None,
                project_id,
                "pipeline",
                "task.retry_scheduled",
                &serde_json::json!({
                    "phase": retry_status,
                    "attempt": current.attempt,
                    "max_attempts": current.max_attempts,
                    "error": error,
                }),
            );
        }
        Ok(())
    }

    fn normalize_error_signature(error: &str) -> String {
        let mut out = String::with_capacity(256);
        let mut prev_space = false;
        for ch in error.chars().flat_map(|c| c.to_lowercase()) {
            let mapped = if ch.is_ascii_digit() {
                '#'
            } else if ch.is_ascii_alphanumeric() {
                ch
            } else {
                ' '
            };
            if mapped == ' ' {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            } else {
                out.push(mapped);
                prev_space = false;
            }
            if out.len() >= 220 {
                break;
            }
        }
        out.trim().to_string()
    }

    fn note_failure_signature(&self, task_id: i64, phase: &str, error: &str) -> u32 {
        let sig = Self::normalize_error_signature(error);
        let mut map = self
            .failure_signatures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let key = (task_id, phase.to_string());
        match map.get_mut(&key) {
            Some((prev_sig, count)) if *prev_sig == sig => {
                *count += 1;
                *count
            },
            Some((prev_sig, count)) => {
                *prev_sig = sig;
                *count = 1;
                1
            },
            None => {
                map.insert(key, (sig, 1));
                1
            },
        }
    }

    pub(crate) fn clear_failure_signatures(&self, task_id: i64) {
        let mut map = self
            .failure_signatures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.retain(|(id, _), _| *id != task_id);
        if let Ok(mut retry_map) = self.retry_not_before.try_lock() {
            retry_map.remove(&task_id);
        }
    }

    /// Build a summary of previous failed attempts for fresh-session retries.
    fn build_retry_summary(&self, task_id: i64, current_error: &str) -> String {
        let outputs = self.db.get_task_outputs(task_id).unwrap_or_default();
        let mut summary =
            String::from("FRESH RETRY — previous approaches failed. Summary of attempts:\n");
        for (i, output) in outputs.iter().rev().take(3).enumerate() {
            let truncated: String = output.output.chars().take(500).collect();
            summary.push_str(&format!(
                "\nAttempt {} ({}): {}\n",
                i + 1,
                output.phase,
                truncated
            ));
        }
        summary.push_str(&format!(
            "\nLatest error:\n{}\n\nTry a fundamentally different approach.",
            current_error.chars().take(2000).collect::<String>()
        ));
        summary
    }

    /// Git author pair from config, or None if not configured.
    fn git_author(&self) -> Option<(&str, &str)> {
        if self.config.git.author_name.is_empty() {
            None
        } else {
            Some((
                self.config.git.author_name.as_str(),
                self.config.git.author_email.as_str(),
            ))
        }
    }
    // ── Phase handlers ────────────────────────────────────────────────────

    /// Setup phase: record branch name, create per-task worktree, and advance.
    pub(crate) async fn setup_branch(&self, task: &Task, mode: &PipelineMode) -> Result<()> {
        let next = mode
            .phases
            .iter()
            .find(|p| p.phase_type != PhaseType::Setup)
            .map(|p| p.name.as_str())
            .unwrap_or("spec");

        let branch = format!("task-{}", task.id);
        self.db.update_task_branch(task.id, &branch)?;

        // Create per-task worktree for concurrent agent isolation
        if !task.repo_path.is_empty() {
            let git = crate::git::Git::new(&task.repo_path);
            let _ = git.fetch_origin();
            let worktree_dir = format!("{}/.worktrees/task-{}", task.repo_path, task.id);
            let start_ref = git
                .resolve_start_ref(&["origin/main", "origin/master", "main", "master", "HEAD"])
                .unwrap_or_else(|_| "HEAD".to_string());
            match git.create_worktree(&worktree_dir, &branch, &start_ref) {
                Ok(()) => {
                    self.db.update_task_repo_path(task.id, &worktree_dir)?;
                    info!(
                        "task #{} created worktree at {} from {}",
                        task.id, worktree_dir, start_ref
                    );
                },
                Err(e) => {
                    warn!("task #{} worktree creation failed: {e}", task.id);
                },
            }
        }

        self.db.update_task_status(task.id, next, None)?;

        self.emit(PipelineEvent::Phase {
            task_id: Some(task.id),
            message: format!("task #{} started branch {}", task.id, branch),
        });

        Ok(())
    }

    pub(crate) async fn run_compliance_check_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        _mode: &PipelineMode,
    ) -> Result<()> {
        let outputs = self.db.get_task_outputs(task.id).unwrap_or_default();
        let latest_text = outputs
            .iter()
            .rev()
            .find(|o| !o.output.trim().is_empty())
            .map(|o| o.output.as_str())
            .unwrap_or("");
        let profile = if phase.compliance_profile.trim().is_empty() {
            "uk_sra"
        } else {
            phase.compliance_profile.trim()
        };
        let enforcement = if phase.compliance_enforcement.trim().is_empty() {
            "warn"
        } else {
            phase.compliance_enforcement.trim()
        };

        let findings = run_compliance_pack(profile, latest_text);
        let mut report = String::new();
        report.push_str("# Compliance Check\n\n");
        report.push_str(&format!(
            "- Profile: `{profile}`\n- Enforcement: `{enforcement}`\n"
        ));
        if findings.is_empty() {
            report.push_str("\nResult: PASS. No compliance findings.\n");
        } else {
            report.push_str("\nResult: FINDINGS\n\n");
            for f in &findings {
                report.push_str(&format!(
                    "- [{}] {} ({})\n",
                    f.severity, f.issue, f.check_id
                ));
            }
            report.push_str("\nRecommended remediation: add a `Regulatory Considerations` section with source links and an as-of date.\n");
        }

        let compliance_json = serde_json::json!({
            "phase": phase.name,
            "profile": profile,
            "enforcement": enforcement,
            "checked_at": chrono::Utc::now().to_rfc3339(),
            "passed": findings.is_empty(),
            "findings": findings.iter().map(|f| serde_json::json!({
                "check_id": f.check_id,
                "severity": f.severity,
                "issue": f.issue,
                "source_url": f.source_url,
                "as_of": f.as_of,
            })).collect::<Vec<_>>(),
        });
        if let Ok(existing_raw) = self.db.get_task_structured_data(task.id) {
            let mut base = serde_json::from_str::<serde_json::Value>(&existing_raw)
                .unwrap_or_else(|_| serde_json::json!({}));
            if !base.is_object() {
                base = serde_json::json!({});
            }
            base["compliance_check"] = compliance_json;
            if let Ok(serialized) = serde_json::to_string(&base) {
                let _ = self.db.update_task_structured_data(task.id, &serialized);
            }
        }

        let blocked = compliance_should_block(enforcement, &findings);
        let success = !blocked;
        let exit_code = if success { 0 } else { 1 };
        if let Err(e) = self
            .db
            .insert_task_output(task.id, &phase.name, &report, "", exit_code)
        {
            warn!("task #{}: insert_task_output({}): {e}", task.id, phase.name);
        }

        if findings.is_empty() {
            self.db.update_task_status(task.id, &phase.next, None)?;
            return Ok(());
        }

        if blocked {
            self.db
                .update_task_status(task.id, "pending_review", Some(&report))?;
            self.emit(PipelineEvent::Phase {
                task_id: Some(task.id),
                message: format!(
                    "task #{} blocked by compliance check ({profile}) — moved to pending_review",
                    task.id
                ),
            });
            return Ok(());
        }

        self.db.update_task_status(task.id, &phase.next, None)?;
        Ok(())
    }

    /// Run an agent phase.
    pub(crate) async fn run_agent_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
    ) -> Result<()> {
        let session_dir_rel = Self::task_session_dir_rel(task);
        tokio::fs::create_dir_all(&session_dir_rel).await.ok();
        let session_dir = Self::task_session_dir(task);

        // Use the task worktree as work_dir when available (created in setup_branch).
        // This ensures Docker containers bind-mount the actual repo, not the session dir.
        let work_dir = if !task.repo_path.is_empty()
            && std::path::Path::new(&task.repo_path).join(".git").exists()
        {
            task.repo_path.clone()
        } else {
            session_dir.clone()
        };

        let pending_messages = self
            .db
            .get_pending_task_messages(task.id)
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.role, m.content))
            .collect::<Vec<_>>();

        let backend_name = self.selected_backend_name(task);
        let mut ctx = self.make_context(task, work_dir.clone(), session_dir, pending_messages);
        self.prepare_linked_agent_credentials(task, &backend_name, &mut ctx)
            .await
            .unwrap_or_else(|err| {
                warn!(
                    task_id = task.id,
                    backend = backend_name.as_str(),
                    "failed to prepare linked credentials: {err}"
                );
            });
        let had_pending = !ctx.pending_messages.is_empty();
        let phase_gate_token = Self::build_phase_gate_token(task, phase);
        ctx.phase_attempt = task.attempt;
        ctx.phase_gate_token = phase_gate_token.clone();
        let test_cmd = ctx.repo_config.test_cmd.clone();

        // Inject prior research from knowledge graph for lawborg tasks
        if task.mode == "lawborg" || task.mode == "legal" {
            let pid = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let query = format!("{} {}", task.title, task.description);
            let results = crate::knowledge::get_prior_research(
                &self.db,
                self.embed_registry.client_for_mode(&task.mode),
                &query,
                pid,
                5,
            )
            .await;
            ctx.prior_research = results
                .into_iter()
                .map(|r| format!("[{}] {}", r.file_path, r.chunk_text))
                .collect();
        }

        // Wire live NDJSON stream for the dashboard LiveTerminal.
        let (stream_tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        ctx.stream_tx = Some(stream_tx);
        self.stream_manager.start(task.id).await;
        let sm = Arc::clone(&self.stream_manager);
        let stream_task_id = task.id;
        let chat_tx = if task.project_id > 0 {
            self.chat_event_tx.clone().map(|tx| (tx, task.project_id))
        } else {
            None
        };
        tokio::spawn(async move {
            while let Some(line) = stream_rx.recv().await {
                if let Some((ref tx, pid)) = chat_tx {
                    let evt = serde_json::json!({
                        "type": "task_stream",
                        "thread": format!("project:{pid}"),
                        "task_id": stream_task_id,
                        "data": &line,
                    })
                    .to_string();
                    let _ = tx.send(evt);
                }
                sm.push_line(&stream_task_id, line).await;
            }
            sm.end_task(stream_task_id).await;
        });

        info!("running {} phase for task #{}", phase.name, task.id);
        self.log_pipeline_event(
            task,
            "phase.started",
            &serde_json::json!({
                "phase": phase.name,
                "attempt": task.attempt,
                "revision_count": task.revision_count,
                "has_pending_messages": had_pending,
            }),
        );

        let backend = match self.resolve_backend(task) {
            Some(b) => b,
            None => {
                warn!("task #{}: no backend configured, failing task", task.id);
                self.fail_or_retry(task, &phase.name, "no agent backend configured")?;
                return Ok(());
            },
        };
        Self::clear_phase_control_files(&work_dir);
        if let Err(e) = self
            .write_pipeline_state_snapshot(task, &phase.name, &work_dir)
            .await
        {
            warn!("task #{}: write_pipeline_state_snapshot: {e}", task.id);
        }
        let result = self
            .run_backend_phase(&backend, task, phase, ctx)
            .await
            .unwrap_or_else(|e| {
                error!("backend.run_phase for task #{}: {e}", task.id);
                PhaseOutput::failed(String::new())
            });

        if let Some(ref sid) = result.new_session_id {
            if let Err(e) = self.db.update_task_session(task.id, sid) {
                warn!("task #{}: update_task_session: {e}", task.id);
            }
        }

        let exit_code: i64 = if result.success { 0 } else { 1 };
        if let Err(e) = self.db.insert_task_output(
            task.id,
            &phase.name,
            &result.output,
            &result.raw_stream,
            exit_code,
        ) {
            warn!("task #{}: insert_task_output: {e}", task.id);
        }

        self.log_pipeline_event(
            task,
            "phase.agent_finished",
            &serde_json::json!({
                "phase": phase.name,
                "success": result.success,
                "exit_code": exit_code,
                "output_len": result.output.len(),
                "raw_stream_len": result.raw_stream.len(),
            }),
        );
        self.emit(PipelineEvent::Output {
            task_id: Some(task.id),
            message: format!(
                "task #{} phase {} completed (success={})",
                task.id, phase.name, result.success
            ),
        });

        // Read agent signal from .borg/signal.json (if present), or from stdout.
        let signal = Self::read_agent_signal(&work_dir, result.signal_json.as_deref());
        if !signal.reason.is_empty() {
            info!(
                "task #{} signal: status={} reason={}",
                task.id, signal.status, signal.reason
            );
            self.log_pipeline_event(
                task,
                "agent.signal",
                &serde_json::json!({
                    "phase": phase.name,
                    "status": signal.status,
                    "reason": signal.reason,
                    "question": signal.question,
                }),
            );
        }

        // Handle abandon signal: mark failed immediately, don't burn retry budget.
        if signal.is_abandon() {
            let reason = if signal.reason.is_empty() {
                "agent abandoned task".to_string()
            } else {
                format!("agent abandoned: {}", signal.reason)
            };
            self.log_pipeline_event(
                task,
                "agent.abandoned",
                &serde_json::json!({
                    "phase": phase.name,
                    "reason": &reason,
                }),
            );
            self.db
                .update_task_status(task.id, "failed", Some(&reason))?;
            return Ok(());
        }

        let benchmark_state = if Self::requires_legal_benchmark_state(task, phase)
            && (signal.is_blocked() || result.success)
        {
            let state = match Self::read_benchmark_phase_state(&work_dir) {
                Some(state) => state,
                None => {
                    self.log_pipeline_event(
                        task,
                        "guard.benchmark_state_missing",
                        &serde_json::json!({
                            "phase": phase.name,
                            "attempt": task.attempt,
                        }),
                    );
                    self.fail_or_retry(
                        task,
                        &phase.name,
                        "missing or invalid .borg/benchmark-state.json; legal benchmark phases must write structured benchmark state before exiting",
                    )?;
                    return Ok(());
                },
            };
            if let Err(error) = Self::validate_benchmark_phase_state(
                &state,
                task,
                phase,
                &phase_gate_token,
                &signal,
            ) {
                self.fail_or_retry(task, &phase.name, &error)?;
                return Ok(());
            }
            self.persist_benchmark_phase_state(task.id, &state);
            Some(state)
        } else {
            None
        };

        // Handle blocked signal: pause task, don't retry.
        if signal.is_blocked() {
            // Persist any completed retrieval pass before returning blocked so
            // clarification resumes can inherit it as execution context.
            let _ = self.enforce_legal_retrieval_protocol(task, phase, &result.raw_stream);
            let reason = if signal.reason.is_empty() {
                "agent blocked (no reason given)".to_string()
            } else {
                signal.reason.clone()
            };
            let block_detail = if signal.question.is_empty() {
                reason.clone()
            } else {
                format!("{}\n\nQuestion: {}", reason, signal.question)
            };
            self.log_pipeline_event(
                task,
                "agent.blocked",
                &serde_json::json!({
                    "phase": phase.name,
                    "reason": &reason,
                    "question": signal.question,
                    "attempt": task.attempt,
                }),
            );
            self.db
                .update_task_status(task.id, "blocked", Some(&block_detail))?;
            self.emit(PipelineEvent::Phase {
                task_id: Some(task.id),
                message: format!("task #{} blocked: {}", task.id, reason),
            });
            return Ok(());
        }

        // Never advance on a failed agent run; retry the same logical phase path.
        if !result.success {
            let error_msg = if result.output.trim().is_empty() {
                format!("{} phase failed", phase.name)
            } else {
                result.output.clone()
            };
            let retry_status = if phase.name == "impl" || phase.name == "retry" {
                "retry"
            } else {
                phase.name.as_str()
            };
            self.fail_or_retry(task, retry_status, error_msg.trim())?;
            return Ok(());
        }

        if let Some(ref artifact) = phase.check_artifact {
            if !crate::ipc::check_artifact(&work_dir, artifact) {
                self.fail_or_retry(task, &phase.name, &format!("missing artifact: {artifact}"))?;
                return Ok(());
            }
        }

        // For Docker phases, commit agent changes from the host (the container
        // bind-mounts the worktree but cannot push).
        if result.ran_in_docker && !task.repo_path.is_empty() {
            let git = crate::git::Git::new(&task.repo_path);
            let (_, user_coauthor) = self.git_coauthor_settings();
            let msg = Self::with_user_coauthor("feat: borg agent changes", &user_coauthor);
            match git.commit_all(&work_dir, &msg, self.git_author()) {
                Ok(true) => info!("task #{} committed Docker agent changes", task.id),
                Ok(false) => info!("task #{} Docker phase: no changes to commit", task.id),
                Err(e) => warn!("task #{} post-Docker commit failed: {e}", task.id),
            }
        }

        if phase.compile_check && !test_cmd.is_empty() {
            if let Some(check_cmd) = derive_compile_check(&test_cmd) {
                let out = if result.ran_in_docker {
                    container_result_as_test_output(&result.container_test_results, "compileCheck")
                } else {
                    match self
                        .run_test_command_for_task(task, &work_dir, &check_cmd)
                        .await
                    {
                        Ok(o) => Some(o),
                        Err(e) => {
                            warn!("compile check error for task #{}: {e}", task.id);
                            None
                        },
                    }
                };
                if let Some(ref o) = out {
                    if o.exit_code != 0 {
                        let compile_err = format!("{}\n{}", o.stdout, o.stderr);
                        info!("task #{} compile check failed, running fix agent", task.id);
                        if !self
                            .run_compile_fix(task, &work_dir, &check_cmd, &compile_err)
                            .await?
                        {
                            let msg = format!(
                                "Compile fix failed after 2 attempts\n\n{}",
                                compile_err.chars().take(2000).collect::<String>()
                            );
                            self.fail_or_retry(task, &phase.name, &msg)?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        if let Some(protocol_error) =
            self.enforce_legal_retrieval_protocol(task, phase, &result.raw_stream)
        {
            self.log_pipeline_event(
                task,
                "guard.retrieval_protocol_rejected",
                &serde_json::json!({
                    "phase": phase.name,
                    "attempt": task.attempt,
                    "error": protocol_error.chars().take(1000).collect::<String>(),
                }),
            );
            self.fail_or_retry(task, &phase.name, &protocol_error)?;
            return Ok(());
        }
        if let Some(ref state) = benchmark_state {
            if let Some(state_error) = Self::enforce_legal_benchmark_state_guard(task, phase, state)
            {
                self.log_pipeline_event(
                    task,
                    "guard.state_guard_rejected",
                    &serde_json::json!({
                        "phase": phase.name,
                        "attempt": task.attempt,
                        "error": state_error.chars().take(1000).collect::<String>(),
                    }),
                );
                self.fail_or_retry(task, &phase.name, &state_error)?;
                return Ok(());
            }
        }
        if let Some(clarification_error) = Self::enforce_legal_benchmark_clarification_guard(
            task,
            phase,
            &work_dir,
            &result.output,
        ) {
            self.log_pipeline_event(
                task,
                "guard.clarification_guard_rejected",
                &serde_json::json!({
                    "phase": phase.name,
                    "attempt": task.attempt,
                    "error": clarification_error.chars().take(1000).collect::<String>(),
                }),
            );
            self.fail_or_retry(task, &phase.name, &clarification_error)?;
            return Ok(());
        }

        if phase.runs_tests && mode.uses_test_cmd && !test_cmd.is_empty() {
            let out = if result.ran_in_docker {
                container_result_as_test_output(&result.container_test_results, "test")
            } else {
                match self
                    .run_test_command_for_task(task, &work_dir, &test_cmd)
                    .await
                {
                    Ok(o) => Some(o),
                    Err(e) => {
                        warn!("test command error for task #{}: {}", task.id, e);
                        return Ok(());
                    },
                }
            };
            if let Some(o) = out {
                if o.exit_code != 0 {
                    let error_msg = format!("{}\n{}", o.stdout, o.stderr);
                    self.fail_or_retry(task, "retry", &error_msg)?;
                    return Ok(());
                }
            }
        }

        let verdict = match Self::read_phase_completion_verdict(&work_dir) {
            Some(verdict) => verdict,
            None => {
                self.fail_or_retry(
                    task,
                    &phase.name,
                    "missing or invalid .borg/phase-verdict.json; phase may not advance without an explicit completion verdict",
                )?;
                return Ok(());
            },
        };
        if let Err(error) =
            Self::validate_phase_completion_verdict(&verdict, task, phase, &phase_gate_token)
        {
            self.fail_or_retry(task, &phase.name, &error)?;
            return Ok(());
        }
        if !verdict.ready_to_advance {
            let mut msg = if verdict.rationale.trim().is_empty() {
                "agent reported that the phase is not ready to advance".to_string()
            } else {
                format!(
                    "agent self-check did not approve phase advancement: {}",
                    verdict.rationale.trim()
                )
            };
            if !verdict.missing_requirements.is_empty() {
                msg.push_str("\n\nMissing requirements:\n");
                for item in &verdict.missing_requirements {
                    msg.push_str("- ");
                    msg.push_str(item.trim());
                    msg.push('\n');
                }
            }
            self.fail_or_retry(task, &phase.name, msg.trim())?;
            return Ok(());
        }

        self.log_pipeline_event(
            task,
            "phase.advanced",
            &serde_json::json!({
                "phase": phase.name,
                "attempt": task.attempt,
                "verdict_rationale": verdict.rationale.chars().take(500).collect::<String>(),
            }),
        );
        self.advance_phase(task, phase, mode)?;
        if had_pending {
            if let Err(e) = self.db.mark_messages_delivered(task.id, &phase.name) {
                warn!("task #{}: mark_messages_delivered: {e}", task.id);
            }
        }
        Ok(())
    }

    /// Read `.borg/signal.json` from the work dir. Returns default (done) if missing or malformed.
    fn read_agent_signal(
        work_dir: &str,
        phase_output_signal: Option<&str>,
    ) -> crate::types::AgentSignal {
        // Try direct path first, then Docker container path
        let paths = [
            format!("{work_dir}/.borg/signal.json"),
            format!("{work_dir}/repo/.borg/signal.json"),
        ];
        for path in &paths {
            if let Ok(contents) = std::fs::read_to_string(path) {
                std::fs::remove_file(path).ok();
                if let Ok(sig) = serde_json::from_str(&contents) {
                    return sig;
                }
            }
        }
        // Fall back to signal from agent stdout
        if let Some(json_str) = phase_output_signal {
            if let Ok(sig) = serde_json::from_str(json_str) {
                return sig;
            }
        }
        crate::types::AgentSignal::default()
    }

    fn build_phase_gate_token(task: &Task, phase: &PhaseConfig) -> String {
        format!(
            "{}:{}:{}:{}",
            task.id,
            phase.name,
            task.attempt,
            Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or_else(|| Utc::now().timestamp_micros() * 1_000)
        )
    }

    fn clear_phase_control_files(work_dir: &str) {
        for path in Self::phase_control_paths(work_dir, "signal.json")
            .into_iter()
            .chain(Self::phase_control_paths(work_dir, "phase-verdict.json"))
            .chain(Self::phase_control_paths(work_dir, "benchmark-state.json"))
        {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("failed to remove stale phase control file {}: {}", path, e);
                }
            }
        }
    }

    fn phase_control_paths(work_dir: &str, file_name: &str) -> [String; 2] {
        [
            format!("{work_dir}/.borg/{file_name}"),
            format!("{work_dir}/repo/.borg/{file_name}"),
        ]
    }

    /// Read `.borg/phase-verdict.json` from the work dir. Returns `None` if missing or malformed.
    pub(crate) fn read_phase_completion_verdict(work_dir: &str) -> Option<PhaseCompletionVerdict> {
        for path in Self::phase_control_paths(work_dir, "phase-verdict.json") {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                std::fs::remove_file(&path).ok();
                match serde_json::from_str::<PhaseCompletionVerdict>(&raw) {
                    Ok(verdict) => return Some(verdict),
                    Err(e) => {
                        warn!("invalid phase-verdict.json at {}: {}", path, e);
                        return None;
                    },
                }
            }
        }
        None
    }

    pub(crate) fn read_benchmark_phase_state(work_dir: &str) -> Option<BenchmarkPhaseState> {
        for path in Self::phase_control_paths(work_dir, "benchmark-state.json") {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                std::fs::remove_file(&path).ok();
                match serde_json::from_str::<BenchmarkPhaseState>(&raw) {
                    Ok(state) => return Some(state),
                    Err(e) => {
                        warn!("invalid benchmark-state.json at {}: {}", path, e);
                        return None;
                    },
                }
            }
        }
        None
    }

    pub(crate) fn validate_phase_completion_verdict(
        verdict: &PhaseCompletionVerdict,
        task: &Task,
        phase: &PhaseConfig,
        gate_token: &str,
    ) -> std::result::Result<(), String> {
        let mut problems = Vec::new();

        if verdict.task_id != task.id {
            problems.push(format!(
                "task_id mismatch (expected {}, got {})",
                task.id, verdict.task_id
            ));
        }
        if verdict.phase.trim() != phase.name {
            problems.push(format!(
                "phase mismatch (expected {}, got {})",
                phase.name,
                verdict.phase.trim()
            ));
        }
        if verdict.attempt != task.attempt {
            problems.push(format!(
                "attempt mismatch (expected {}, got {})",
                task.attempt, verdict.attempt
            ));
        }
        if verdict.gate_token != gate_token {
            problems.push("gate token mismatch (verdict is stale or from another run)".to_string());
        }
        if verdict.rationale.trim().is_empty() {
            problems.push("rationale must not be empty".to_string());
        }
        if verdict
            .missing_requirements
            .iter()
            .any(|item| item.trim().is_empty())
        {
            problems.push("missing_requirements must not contain blank items".to_string());
        }
        if verdict.ready_to_advance && !verdict.missing_requirements.is_empty() {
            problems.push(
                "ready_to_advance cannot be true when missing_requirements is non-empty"
                    .to_string(),
            );
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "invalid or stale .borg/phase-verdict.json: {}",
                problems.join("; ")
            ))
        }
    }

    pub(crate) fn validate_benchmark_phase_state(
        state: &BenchmarkPhaseState,
        task: &Task,
        phase: &PhaseConfig,
        gate_token: &str,
        signal: &crate::types::AgentSignal,
    ) -> std::result::Result<(), String> {
        let mut problems = Vec::new();

        if state.task_id != task.id {
            problems.push(format!(
                "task_id mismatch (expected {}, got {})",
                task.id, state.task_id
            ));
        }
        if state.phase.trim() != phase.name {
            problems.push(format!(
                "phase mismatch (expected {}, got {})",
                phase.name,
                state.phase.trim()
            ));
        }
        if state.attempt != task.attempt {
            problems.push(format!(
                "attempt mismatch (expected {}, got {})",
                task.attempt, state.attempt
            ));
        }
        if state.gate_token != gate_token {
            problems.push(
                "gate token mismatch (benchmark-state is stale or from another run)".to_string(),
            );
        }
        if state.rationale.trim().is_empty() {
            problems.push("rationale must not be empty".to_string());
        }
        if !matches!(state.status.as_str(), "ready" | "blocked_for_clarification") {
            problems.push("status must be `ready` or `blocked_for_clarification`".to_string());
        }
        for (index, uncertainty) in state.uncertainties.iter().enumerate() {
            if uncertainty.issue.trim().is_empty() {
                problems.push(format!("uncertainties[{index}].issue must not be empty"));
            }
            if uncertainty.missing_fact.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].missing_fact must not be empty"
                ));
            }
            if uncertainty.uncertainty_type.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].uncertainty_type must not be empty"
                ));
            }
            if uncertainty.support_status.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].support_status must not be empty"
                ));
            }
            if uncertainty.operative_status.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].operative_status must not be empty"
                ));
            }
            if uncertainty.recommended_treatment.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].recommended_treatment must not be empty"
                ));
            }
            if uncertainty.justification.trim().is_empty() {
                problems.push(format!(
                    "uncertainties[{index}].justification must not be empty"
                ));
            }
        }
        for (index, claim) in state.claims.iter().enumerate() {
            if claim.claim.trim().is_empty() {
                problems.push(format!("claims[{index}].claim must not be empty"));
            }
            if claim.claim_type.trim().is_empty() {
                problems.push(format!("claims[{index}].claim_type must not be empty"));
            }
            if claim.support_status.trim().is_empty() {
                problems.push(format!("claims[{index}].support_status must not be empty"));
            }
            if claim
                .supporting_artifacts
                .iter()
                .any(|artifact| artifact.trim().is_empty())
            {
                problems.push(format!(
                    "claims[{index}].supporting_artifacts must not contain blank items"
                ));
            }
        }

        if state.is_blocked_for_clarification() {
            if state.clarification_type.trim().is_empty() {
                problems.push(
                    "blocked_for_clarification state must include clarification_type".to_string(),
                );
            }
            if state.material_fact.trim().is_empty() {
                problems
                    .push("blocked_for_clarification state must include material_fact".to_string());
            }
            if state.question.trim().is_empty() {
                problems.push("blocked_for_clarification state must include question".to_string());
            }
            if !signal.is_blocked() {
                problems.push(
                    "benchmark-state says blocked_for_clarification but .borg/signal.json did not block"
                        .to_string(),
                );
            }
            if signal.reason_code.trim().is_empty() {
                problems.push(
                    "blocked benchmark signals must include a machine-readable reason_code"
                        .to_string(),
                );
            } else if signal.reason_code.trim() != state.clarification_type.trim() {
                problems.push(format!(
                    "blocked reason_code mismatch (signal {}, benchmark-state {})",
                    signal.reason_code.trim(),
                    state.clarification_type.trim()
                ));
            }
            // Allow signal.json question to differ from benchmark-state question —
            // the model may batch multiple questions in signal.json as a numbered list
            // while benchmark-state.question summarises the first or primary question.
        } else {
            if !state.question.trim().is_empty() {
                problems.push("ready benchmark-state must leave question empty".to_string());
            }
            if signal.is_blocked() {
                problems.push(
                    "benchmark-state says ready but .borg/signal.json blocked the phase"
                        .to_string(),
                );
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "invalid or inconsistent .borg/benchmark-state.json: {}",
                problems.join("; ")
            ))
        }
    }

    fn enforce_legal_retrieval_protocol(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        raw_stream: &str,
    ) -> Option<String> {
        if !self.config.pipeline.enforce_retrieval_protocol {
            return None;
        }
        if raw_stream.trim().is_empty() {
            return None;
        }
        let stats = self.db.get_project_file_stats(task.project_id).ok()?;
        let trigger_source = legal_retrieval_protocol_trigger(task, phase, &stats)?;
        let prior_report = self
            .db
            .get_task_structured_data(task.id)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        let prior_passed = prior_report
            .as_ref()
            .and_then(prior_retrieval_protocol_passed_from_structured_data)
            .unwrap_or(false);

        let mut report = inspect_legal_retrieval_trace(raw_stream);
        report.enforced = true;
        report.trigger_source = trigger_source.to_string();

        let mut missing = Vec::new();
        if report.inventory_calls == 0 {
            missing.push("call `list_documents` to inventory the full corpus".to_string());
        }
        if report.search_calls < 2 || report.distinct_search_queries.len() < 2 {
            missing.push(
                "run at least 2 `search_documents` / BorgSearch query passes with distinct queries"
                    .to_string(),
            );
        }
        if report.coverage_calls == 0 {
            missing.push(
                "call `check_coverage` (or `/api/borgsearch/coverage`) to find unmatched documents"
                    .to_string(),
            );
        }
        if report.full_document_reads == 0 {
            missing.push(
                "inspect at least 1 full document via `read_document`, BorgSearch file fetch, or a staged `project_files/` read".to_string(),
            );
        }

        let reused_prior_pass =
            should_reuse_prior_retrieval_pass(task, prior_passed, missing.is_empty());
        if reused_prior_pass {
            missing.clear();
        }

        report.missing_steps = missing.clone();
        report.passed = missing.is_empty();
        self.persist_retrieval_protocol_report(task.id, &report, reused_prior_pass);

        if report.passed {
            return None;
        }

        let seen_queries = if report.search_queries.is_empty() {
            "none".to_string()
        } else {
            report.search_queries.join(" | ")
        };
        Some(format!(
            "Exhaustive legal retrieval protocol was not satisfied.\n\
             Required for this task: inventory the corpus, iterate search with distinct queries, run coverage, and inspect full documents. Trigger source: {}.\n\
             Observed: list_documents={}, get_document_categories={}, search_documents={}, check_coverage={}, full_document_reads={}; search queries={}\n\
             Missing: {}\n\
             Retry and complete the retrieval protocol before drafting conclusions about the corpus.",
            trigger_source,
            report.inventory_calls,
            report.category_calls,
            report.search_calls,
            report.coverage_calls,
            report.full_document_reads,
            seen_queries,
            missing.join("; "),
        ))
    }

    fn persist_retrieval_protocol_report(
        &self,
        task_id: i64,
        report: &LegalRetrievalTrace,
        reused_prior_pass: bool,
    ) {
        let payload = serde_json::json!({
            "checked_at": chrono::Utc::now().to_rfc3339(),
            "enforced": report.enforced,
            "passed": report.passed,
            "reused_prior_pass": reused_prior_pass,
            "trigger_source": report.trigger_source,
            "tool_counts": {
                "list_documents": report.inventory_calls,
                "get_document_categories": report.category_calls,
                "search_documents": report.search_calls,
                "check_coverage": report.coverage_calls,
                "full_document_reads": report.full_document_reads,
            },
            "search_queries": report.search_queries,
            "distinct_search_queries": report.distinct_search_queries,
            "coverage_queries": report.coverage_queries,
            "mcp_servers": report.mcp_servers,
            "missing_steps": report.missing_steps,
        });

        let mut base = self
            .db
            .get_task_structured_data(task_id)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| serde_json::json!({}));
        base["retrieval_protocol"] = payload;
        if let Ok(serialized) = serde_json::to_string(&base) {
            let _ = self.db.update_task_structured_data(task_id, &serialized);
        }
    }

    fn persist_benchmark_phase_state(&self, task_id: i64, state: &BenchmarkPhaseState) {
        let payload = serde_json::to_value(state).unwrap_or_else(|_| serde_json::json!({}));
        let mut base = self
            .db
            .get_task_structured_data(task_id)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| serde_json::json!({}));
        base["benchmark_state"] = payload;
        if let Ok(serialized) = serde_json::to_string(&base) {
            let _ = self.db.update_task_structured_data(task_id, &serialized);
        }
    }

    pub(crate) fn enforce_legal_benchmark_clarification_guard(
        task: &Task,
        phase: &PhaseConfig,
        work_dir: &str,
        phase_output: &str,
    ) -> Option<String> {
        if !Self::is_legal_benchmark_task(task) {
            return None;
        }
        if !matches!(
            phase.name.as_str(),
            "implement" | "impl" | "retry" | "review"
        ) {
            return None;
        }

        let sources = Self::benchmark_guard_sources(work_dir, phase_output);

        // If any source in the bundle contains a definitive negative
        // recommendation, the overall deliverable set is refusing to
        // give a green light. Companion files (intake note, DD report)
        // naturally reference pre-sign timing and unresolved facts
        // without being escape hatches themselves.
        let bundle_has_negative_recommendation = sources
            .iter()
            .any(|(_, text)| is_negative_sign_recommendation(&text.to_ascii_lowercase()));
        if bundle_has_negative_recommendation {
            return None;
        }

        // Only recommendation-bearing documents should be checked for
        // clarification escapes. Supporting documents (intake note, DD
        // report, action plan) inventory facts, timing, and open items
        // without making sign/close recommendations themselves.
        let recommendation_sources: &[&str] = &["advice_memo.md", "review_notes.md"];
        let has_recommendation_file = sources
            .iter()
            .any(|(source, _)| recommendation_sources.contains(&source.as_str()));

        for (source, text) in &sources {
            // Only check recommendation documents. Phase output is a
            // fallback when no recommendation files were produced.
            if source == "phase output" && has_recommendation_file {
                continue;
            }
            if source != "phase output" && !recommendation_sources.contains(&source.as_str()) {
                continue;
            }
            if let Some(excerpt) = detect_benchmark_clarification_escape(text) {
                return Some(format!(
                    "Benchmark clarification guard failed.\n\
                     The task output still treats an unresolved pre-sign/pre-close fact as a caveat instead of blocking for clarification.\n\
                     Source: {}\n\
                     Evidence: {}\n\
                     If signing or closing depends on confirming a material fact that is not answerable from the corpus, write `.borg/signal.json` with `{{\"status\":\"blocked\",\"reason\":\"Material fact missing\",\"question\":\"...\"}}` before finalising deliverables.",
                    source, excerpt
                ));
            }
        }

        None
    }

    pub(crate) fn enforce_legal_benchmark_state_guard(
        task: &Task,
        phase: &PhaseConfig,
        state: &BenchmarkPhaseState,
    ) -> Option<String> {
        if !Self::is_legal_benchmark_task(task) {
            return None;
        }
        if !matches!(
            phase.name.as_str(),
            "implement" | "impl" | "retry" | "review"
        ) {
            return None;
        }
        if state.is_blocked_for_clarification() {
            return None;
        }

        for uncertainty in &state.uncertainties {
            if uncertainty.recommended_treatment.trim() == "blocked_clarification" {
                return Some(format!(
                    "Benchmark structured-state guard failed.\n\
                     The benchmark-state file says an unresolved issue requires blocked clarification, but the run exited as ready.\n\
                     Issue: {}\n\
                     Missing fact: {}\n\
                     Justification: {}",
                    uncertainty.issue.trim(),
                    uncertainty.missing_fact.trim(),
                    uncertainty.justification.trim()
                ));
            }

            let changes_recommendation = uncertainty.changes_sign || uncertainty.changes_close_only;
            let support_confirmed = matches!(
                uncertainty.support_status.trim(),
                "record_confirmed"
                    | "confirmed_via_clarification"
                    | "confirmed"
                    | "asked_but_unavailable"
                    | "clarification_attempted"
                    | "asked_unavailable"
            );
            let support_missing = !support_confirmed;
            let not_blocked = uncertainty.recommended_treatment.trim() != "blocked_clarification";
            if changes_recommendation && support_missing && not_blocked {
                return Some(format!(
                    "Benchmark structured-state guard failed.\n\
                     An uncertainty that changes the sign/close recommendation has no confirmed \
                     support but was not routed to blocked clarification.\n\
                     Issue: {}\n\
                     Missing fact: {}\n\
                     Support status: {}\n\
                     Recommended treatment: {}\n\
                     If an adverse answer would change your blocker / condition / post-close \
                     classification, use the clarification channel before finalising.",
                    uncertainty.issue.trim(),
                    uncertainty.missing_fact.trim(),
                    uncertainty.support_status.trim(),
                    uncertainty.recommended_treatment.trim()
                ));
            }
        }

        // Per-uncertainty check: any uncertainty whose support is NOT fully confirmed
        // must be routed to blocked_clarification. Only "record_confirmed" and
        // "confirmed_via_clarification" pass — everything else (unavailable, partial_record,
        // inferred, conflicting, stale, intended_only, etc.) must go through the
        // clarification channel. If a fact is truly immaterial, don't list it as an
        // uncertainty at all.
        // Collect ALL failing uncertainties so the model can fix them in one pass.
        // Skip in revision stages where clarification budget may be exhausted.
        // Also skip after attempt >= 4 — the model has had enough chances to ask
        // questions and further rejections just burn attempts without value.
        if task.revision_count == 0 && task.attempt < 4 {
            let mut failing_issues: Vec<String> = Vec::new();
            for uncertainty in &state.uncertainties {
                let confirmed = matches!(
                    uncertainty.support_status.trim(),
                    "record_confirmed"
                    | "confirmed_via_clarification"
                    | "confirmed"
                    | "asked_but_unavailable"
                    | "clarification_attempted"
                    | "asked_unavailable"
                );
                if confirmed {
                    continue;
                }
                if uncertainty.recommended_treatment.trim() == "blocked_clarification" {
                    continue;
                }
                failing_issues.push(format!(
                    "  - Issue: {} | Missing fact: {} | Support: {} | Treatment: {}",
                    uncertainty.issue.trim(),
                    uncertainty.missing_fact.trim(),
                    uncertainty.support_status.trim(),
                    uncertainty.recommended_treatment.trim()
                ));
            }
            if !failing_issues.is_empty() {
                return Some(format!(
                    "Benchmark structured-state guard failed.\n\
                     {} uncertainties have unconfirmed support but were not routed to \
                     blocked_clarification.\n\
                     Failing uncertainties:\n{}\n\
                     In the initial review stage, every uncertainty whose support is not \
                     \"record_confirmed\", \"confirmed_via_clarification\", or \"asked_but_unavailable\" \
                     MUST use the clarification channel. If a fact is truly immaterial, don't list \
                     it as an uncertainty. For each failing uncertainty: set recommended_treatment \
                     to \"blocked_clarification\" and write .borg/signal.json to ask about it. \
                     If clarification returned no answer, set support_status to \"asked_but_unavailable\".",
                    failing_issues.len(),
                    failing_issues.join("\n")
                ));
            }
        }

        for claim in &state.claims {
            if claim.safe_to_state_definitively && claim.depends_on_unresolved_fact {
                return Some(format!(
                    "Benchmark structured-state guard failed.\n\
                     A material claim was marked as definitive even though it still depends on an unresolved fact.\n\
                     Claim type: {}\n\
                     Claim: {}",
                    claim.claim_type.trim(),
                    claim.claim.trim()
                ));
            }
            if claim.safe_to_state_definitively
                && matches!(
                    claim.claim_type.trim(),
                    "corpus_exhaustiveness" | "record_completeness"
                )
                && !matches!(
                    claim.support_status.trim(),
                    "record_confirmed"
                        | "supported"
                        | "coverage_verified"
                        | "confirmed_via_clarification"
                        | "confirmed"
                        | "asked_but_unavailable"
                )
            {
                return Some(format!(
                    "Benchmark structured-state guard failed.\n\
                     A completeness-style claim was stated definitively without confirmed support.\n\
                     Claim type: {}\n\
                     Support status: {}\n\
                     Claim: {}",
                    claim.claim_type.trim(),
                    claim.support_status.trim(),
                    claim.claim.trim()
                ));
            }
        }

        None
    }

    fn is_legal_benchmark_task(task: &Task) -> bool {
        matches!(task.mode.as_str(), "lawborg" | "legal")
            && task.task_type.trim() == "benchmark_analysis"
    }

    pub(crate) fn log_pipeline_event(&self, task: &Task, kind: &str, payload: &serde_json::Value) {
        let pid = if task.project_id > 0 {
            Some(task.project_id)
        } else {
            None
        };
        let _ = self
            .db
            .log_event_full(Some(task.id), None, pid, "pipeline", kind, payload);
    }

    fn requires_legal_benchmark_state(task: &Task, phase: &PhaseConfig) -> bool {
        Self::is_legal_benchmark_task(task)
            && matches!(
                phase.name.as_str(),
                "implement" | "impl" | "retry" | "review"
            )
    }

    fn benchmark_guard_sources(work_dir: &str, phase_output: &str) -> Vec<(String, String)> {
        let mut sources = Vec::new();
        if !phase_output.trim().is_empty() {
            sources.push(("phase output".to_string(), phase_output.to_string()));
        }

        let candidate_names = [
            "intake_note.md",
            "advice_memo.md",
            "dd_report.md",
            "action_plan.json",
            "review_notes.md",
        ];
        for name in candidate_names {
            let path = std::path::Path::new(work_dir).join(name);
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if !raw.trim().is_empty() {
                    sources.push((name.to_string(), raw));
                }
            }
        }
        sources
    }

    /// Run a purge phase: delete vectors, messages, and raw files for a task.
    pub(crate) async fn run_purge_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
    ) -> Result<()> {
        info!("task #{} [{}] executing purge phase", task.id, task.status);

        // Delete DB vectors and messages
        self.db.purge_task_data(task.id)?;

        // Delete session directory
        let session_dir = format!("{}/sessions/task-{}", self.config.data_dir, task.id);
        if let Err(e) = std::fs::remove_dir_all(&session_dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "task #{} failed to remove session dir {}: {}",
                    task.id, session_dir, e
                );
            }
        }

        // Delete worktree directory if it's outside the main repo
        if task.repo_path.contains(".worktrees") {
            if let Err(e) = std::fs::remove_dir_all(&task.repo_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(
                        "task #{} failed to remove worktree {}: {}",
                        task.id, task.repo_path, e
                    );
                }
            }
        }

        // We do NOT delete the task record itself, or task_outputs, so the status and final draft survive
        self.advance_phase(task, phase, mode)?;
        Ok(())
    }

    /// Run a validate phase: execute test/compile commands independently, loop back on failure.
    pub(crate) async fn run_validate_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
    ) -> Result<()> {
        let work_dir = task.repo_path.clone();

        let test_cmd = self.repo_config(task).test_cmd;
        if test_cmd.is_empty() {
            self.advance_phase(task, phase, mode)?;
            info!("task #{} validate: no test command, skipping", task.id);
            return Ok(());
        }

        let use_docker = self.sandbox_mode == SandboxMode::Docker;

        // Compile check first (if derivable from test command)
        if let Some(check_cmd) = derive_compile_check(&test_cmd) {
            let out = if use_docker {
                self.run_test_in_container(task, &check_cmd).await?
            } else {
                self.run_test_command_for_task(task, &work_dir, &check_cmd)
                    .await?
            };
            if out.exit_code != 0 {
                let error_msg = format!("{}\n{}", out.stdout, out.stderr);
                info!("task #{} validate: compile check failed", task.id);
                if let Err(e) = self.db.insert_task_output(
                    task.id,
                    "validate",
                    error_msg.trim(),
                    "",
                    out.exit_code as i64,
                ) {
                    warn!("task #{}: insert_task_output(validate): {e}", task.id);
                }
                let retry_status = if phase.retry_phase.is_empty() {
                    &phase.name
                } else {
                    &phase.retry_phase
                };
                self.fail_or_retry(task, retry_status, error_msg.trim())?;
                return Ok(());
            }
        }

        // Run the full test suite
        let out = if use_docker {
            self.run_test_in_container(task, &test_cmd).await?
        } else {
            match self
                .run_test_command_for_task(task, &work_dir, &test_cmd)
                .await
            {
                Ok(o) => o,
                Err(e) => {
                    warn!("task #{} validate: test command error: {e}", task.id);
                    self.fail_or_retry(task, "validate", &format!("test command error: {e}"))?;
                    return Ok(());
                },
            }
        };
        let full_output = format!("{}\n{}", out.stdout, out.stderr);
        if let Err(e) = self.db.insert_task_output(
            task.id,
            "validate",
            full_output.trim(),
            "",
            out.exit_code as i64,
        ) {
            warn!("task #{}: insert_task_output(validate): {e}", task.id);
        }
        if out.exit_code == 0 {
            info!("task #{} validate: all tests pass", task.id);
            self.advance_phase(task, phase, mode)?;
        } else {
            info!("task #{} validate: tests failed", task.id);
            let retry_status = if phase.retry_phase.is_empty() {
                &phase.name
            } else {
                &phase.retry_phase
            };
            self.fail_or_retry(task, retry_status, full_output.trim())?;
        }

        Ok(())
    }

    /// Rebase: try GitHub update-branch API first; on conflict spawn a Docker agent.
    pub(crate) async fn run_rebase_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
    ) -> Result<()> {
        let repo = self.repo_config(task);
        if repo.repo_slug.is_empty() {
            warn!("task #{} rebase: no repo_slug, skipping", task.id);
            self.advance_phase(task, phase, mode)?;
            return Ok(());
        }

        let branch = format!("task-{}", task.id);
        let slug = &repo.repo_slug;

        // Find the PR number for this branch
        let pr_num_out = self
            .gh(&[
                "pr", "view", &branch, "--repo", slug, "--json", "number", "--jq", ".number",
            ])
            .await;
        let pr_num = pr_num_out
            .ok()
            .filter(|o| o.exit_code == 0)
            .and_then(|o| o.stdout.trim().parse::<u64>().ok());

        if let Some(num) = pr_num {
            let update_out = self
                .gh(&[
                    "api",
                    "-X",
                    "PUT",
                    &format!("repos/{slug}/pulls/{num}/update-branch"),
                ])
                .await;
            match update_out {
                Ok(o) if o.exit_code == 0 => {
                    info!("task #{} rebase: update-branch succeeded", task.id);
                    self.advance_phase(task, phase, mode)?;
                    return Ok(());
                },
                Ok(o) => {
                    let err = o.stderr.trim().chars().take(300).collect::<String>();
                    let err_lc = err.to_ascii_lowercase();
                    if err_lc.contains("expected head sha") || err_lc.contains("head ref") {
                        // GitHub branch-tip race; retry on next tick instead of spawning an agent.
                        info!(
                            "task #{} rebase: head SHA race, will retry update-branch on next tick",
                            task.id
                        );
                        return Ok(());
                    }
                    if err_lc.contains("could not resolve host")
                        || err_lc.contains("temporary failure in name resolution")
                        || err_lc.contains("network is unreachable")
                    {
                        warn!(
                            "task #{} rebase: GitHub DNS/network unavailable; skipping agent spawn",
                            task.id
                        );
                        self.fail_or_retry(task, "rebase", &err)?;
                        return Ok(());
                    }
                    warn!(
                        "task #{} rebase: update-branch failed, spawning agent: {err}",
                        task.id
                    );
                },
                Err(e) => {
                    let es = e.to_string();
                    let err_lc = es.to_ascii_lowercase();
                    if err_lc.contains("could not resolve host")
                        || err_lc.contains("temporary failure in name resolution")
                        || err_lc.contains("network is unreachable")
                    {
                        warn!(
                            "task #{} rebase: GitHub DNS/network unavailable; skipping agent spawn",
                            task.id
                        );
                        self.fail_or_retry(task, "rebase", &es)?;
                        return Ok(());
                    }
                    warn!(
                        "task #{} rebase: update-branch error, spawning agent: {e}",
                        task.id
                    );
                },
            }
        } else {
            info!("task #{} rebase: no PR found, advancing", task.id);
            self.advance_phase(task, phase, mode)?;
            return Ok(());
        }

        // Codex backend runs directly on host work_dir; rebase phases use session dirs.
        // Use deterministic local git rebase path to avoid "not a repo" / sandbox loops.
        if self.selected_backend_name(task) == "codex" {
            return self
                .run_rebase_non_interactive(task, phase, mode, slug, &branch)
                .await;
        }

        // GitHub API couldn't auto-merge — spawn an agent to resolve conflicts
        self.run_rebase_agent(task, phase, mode, &branch).await
    }

    async fn verify_rebased_branch(&self, _task: &Task, slug: &str, branch: &str) -> Result<()> {
        let compare = self
            .gh(&[
                "api",
                &format!("repos/{slug}/compare/main...{branch}"),
                "--jq",
                ".behind_by",
            ])
            .await?;
        let behind_by = compare.stdout.trim().parse::<u64>().unwrap_or(1);
        if behind_by > 0 {
            anyhow::bail!("branch {branch} is still behind main by {behind_by}");
        }

        let state_out = self
            .gh(&[
                "pr",
                "view",
                branch,
                "--repo",
                slug,
                "--json",
                "state,number",
                "--jq",
                ".state + \" \" + (.number|tostring)",
            ])
            .await;
        if let Ok(o) = state_out {
            if o.exit_code == 0 {
                let mut parts = o.stdout.split_whitespace();
                let state = parts.next().unwrap_or_default();
                let num = parts.next().unwrap_or_default();
                if state == "CLOSED" {
                    let reopen = self
                        .gh(&["pr", "reopen", num, "--repo", slug])
                        .await
                        .ok()
                        .filter(|x| x.exit_code == 0);
                    if reopen.is_none() {
                        anyhow::bail!("PR #{num} is closed and could not be reopened");
                    }
                }
            }
        }
        Ok(())
    }

    async fn run_rebase_non_interactive(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
        slug: &str,
        branch: &str,
    ) -> Result<()> {
        if let Err(e) = self.ensure_tmp_capacity(task.id, "rebase_non_interactive") {
            self.fail_or_retry(task, "rebase", &format!("tmp capacity check failed: {e}"))?;
            return Ok(());
        }

        let ts = Utc::now().timestamp_millis();
        let tmp_root = self.pipeline_tmp_dir();
        std::fs::create_dir_all(&tmp_root).ok();
        let temp_root = tmp_root.join(format!("borg-rebase-task-{}-{ts}", task.id));
        std::fs::create_dir_all(&temp_root)
            .with_context(|| format!("create temp rebase dir {}", temp_root.display()))?;
        struct TempDirGuard(PathBuf);
        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _temp_guard = TempDirGuard(temp_root.clone());

        let work_dir = temp_root.join("repo");
        let work_dir_s = work_dir.to_string_lossy().to_string();
        let tmp_env = self.pipeline_tmp_dir().to_string_lossy().to_string();

        let clone = tokio::process::Command::new("git")
            .args(["clone", "--no-tags", &task.repo_path, &work_dir_s])
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git clone for non-interactive rebase")?;
        if !clone.status.success() {
            let err = String::from_utf8_lossy(&clone.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("clone failed: {err}"))?;
            return Ok(());
        }

        let fetch = tokio::process::Command::new("git")
            .args([
                "fetch",
                "origin",
                "main:refs/remotes/origin/main",
                &format!("{branch}:refs/remotes/origin/{branch}"),
            ])
            .current_dir(&work_dir_s)
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git fetch origin main")?;
        if !fetch.status.success() {
            let err = String::from_utf8_lossy(&fetch.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("fetch failed: {err}"))?;
            return Ok(());
        }

        let checkout = tokio::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(&work_dir_s)
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git checkout branch for rebase")?;
        if !checkout.status.success() {
            let err = String::from_utf8_lossy(&checkout.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("checkout failed: {err}"))?;
            return Ok(());
        }

        let rebase = tokio::process::Command::new("git")
            .args(["rebase", "-X", "theirs", "origin/main"])
            .current_dir(&work_dir_s)
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git rebase origin/main")?;
        if !rebase.status.success() {
            let err = String::from_utf8_lossy(&rebase.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("rebase failed: {err}"))?;
            return Ok(());
        }

        let test_cmd = self.repo_config(task).test_cmd;
        if let Some(check_cmd) = derive_compile_check(&test_cmd) {
            let out = self
                .run_test_command_for_task(task, &work_dir_s, &check_cmd)
                .await?;
            if out.exit_code != 0 {
                let err = format!("{}\n{}", out.stdout, out.stderr);
                self.fail_or_retry(task, "rebase", &format!("compile check failed: {err}"))?;
                return Ok(());
            }
        }

        let (gh_token, _) = self.resolve_gh_token(&task.created_by);
        let origin_url = if !gh_token.is_empty() {
            format!("https://x-access-token:{gh_token}@github.com/{slug}.git")
        } else {
            format!("https://github.com/{slug}.git")
        };
        let set_url = tokio::process::Command::new("git")
            .args(["remote", "set-url", "origin", &origin_url])
            .current_dir(&work_dir_s)
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git remote set-url origin")?;
        if !set_url.status.success() {
            let err = String::from_utf8_lossy(&set_url.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("set-url failed: {err}"))?;
            return Ok(());
        }

        let push = tokio::process::Command::new("git")
            .args(["push", "--force-with-lease", "origin", branch])
            .current_dir(&work_dir_s)
            .env("TMPDIR", &tmp_env)
            .output()
            .await
            .context("git push --force-with-lease")?;
        if !push.status.success() {
            let err = String::from_utf8_lossy(&push.stderr).to_string();
            self.fail_or_retry(task, "rebase", &format!("push failed: {err}"))?;
            return Ok(());
        }

        if let Err(e) = self.verify_rebased_branch(task, slug, branch).await {
            self.fail_or_retry(
                task,
                "rebase",
                &format!("post-rebase verification failed: {e}"),
            )?;
            return Ok(());
        }

        self.advance_phase(task, phase, mode)?;
        Ok(())
    }

    /// Spawn a Docker agent to rebase the branch onto main and resolve conflicts.
    async fn run_rebase_agent(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
        branch: &str,
    ) -> Result<()> {
        let rebase_phase = PhaseConfig {
            name: "rebase_fix".into(),
            label: "Rebase Fix".into(),
            system_prompt: "You are a rebase agent. Your job is to rebase the current branch \
onto origin/main and resolve any merge conflicts. Preserve the intent of the branch's changes \
while incorporating upstream updates. After resolving conflicts, ensure the code compiles and \
tests pass if a test command is available. Push the result.".into(),
            instruction: format!(
                "Rebase branch `{branch}` onto `origin/main`. Steps:\n\
1. `git fetch origin`\n\
2. `git rebase origin/main`\n\
3. If conflicts arise, resolve them preserving the branch's intent\n\
4. `git rebase --continue` after resolving each conflict\n\
5. After rebase, run the project's compile check (e.g. `cargo check`) to verify the result compiles\n\
6. Fix any compile errors introduced by the rebase before pushing\n\
7. `git push --force-with-lease origin {branch}`\n\n\
If the rebase is too complex or the conflicts are unclear, abort with `git rebase --abort` \
and report what went wrong.",
            ),
            allowed_tools: "Read,Glob,Grep,Write,Edit,Bash".into(),
            use_docker: true,
            fresh_session: true,
            error_instruction: "\n\n---\n## Previous Attempt Failed\n{ERROR}\n\n\
                Analyze what went wrong and take a different approach. \
                Pay close attention to any compile errors — fix them before pushing.".into(),
            ..PhaseConfig::default()
        };

        let session_dir_rel = Self::task_session_dir_rel(task);
        tokio::fs::create_dir_all(&session_dir_rel).await.ok();
        let session_dir = std::fs::canonicalize(&session_dir_rel)
            .unwrap_or_else(|_| std::path::PathBuf::from(&session_dir_rel))
            .to_string_lossy()
            .to_string();

        let ctx = self.make_context(task, session_dir.clone(), session_dir, Vec::new());

        let backend = match self.resolve_backend(task) {
            Some(b) => b,
            None => {
                warn!("task #{} rebase: no backend available", task.id);
                self.fail_or_retry(task, "rebase", "no agent backend")?;
                return Ok(());
            },
        };

        let result = self
            .run_backend_phase(&backend, task, &rebase_phase, ctx)
            .await
            .unwrap_or_else(|e| {
                error!("rebase agent for task #{}: {e}", task.id);
                PhaseOutput::failed(String::new())
            });

        if let Some(ref sid) = result.new_session_id {
            self.db.update_task_session(task.id, sid).ok();
        }

        self.db
            .insert_task_output(
                task.id,
                "rebase_fix",
                &result.output,
                &result.raw_stream,
                if result.success { 0 } else { 1 },
            )
            .ok();

        if result.success {
            // If the container ran a compile check, enforce it before advancing.
            // A bad conflict resolution often compiles fine locally but fails here.
            let compile_result = result
                .container_test_results
                .iter()
                .find(|r| r.phase == "compileCheck");
            if let Some(cr) = compile_result {
                if !cr.passed {
                    let errors = cr.output.chars().take(3000).collect::<String>();
                    warn!(
                        "task #{} rebase: compile check failed after rebase, retrying",
                        task.id
                    );
                    self.db
                        .insert_task_output(task.id, "rebase_compile_fail", &errors, "", 1)
                        .ok();
                    self.fail_or_retry(
                        task,
                        "rebase",
                        &format!("Compile failed after rebase:\n{errors}"),
                    )?;
                    return Ok(());
                }
            }
            let repo = self.repo_config(task);
            if let Err(e) = self
                .verify_rebased_branch(task, &repo.repo_slug, branch)
                .await
            {
                self.fail_or_retry(
                    task,
                    "rebase",
                    &format!("post-rebase verification failed: {e}"),
                )?;
                return Ok(());
            }
            info!("task #{} rebase: agent resolved conflicts", task.id);
            self.advance_phase(task, phase, mode)?;
        } else {
            warn!(
                "task #{} rebase: agent failed to resolve conflicts",
                task.id
            );
            self.fail_or_retry(task, "rebase", &result.output)?;
        }

        Ok(())
    }

    /// Lint is handled inside the Docker container by the entrypoint.
    pub(crate) async fn run_lint_fix_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        mode: &PipelineMode,
    ) -> Result<()> {
        // In Docker mode, lint is handled inside the container by the entrypoint.
        if self.sandbox_mode == SandboxMode::Docker {
            self.advance_phase(task, phase, mode)?;
            return Ok(());
        }

        let wt_path = task.repo_path.clone();

        let lint_cmd = match self.repo_lint_cmd(&task.repo_path, &wt_path) {
            Some(cmd) => cmd,
            None => {
                self.advance_phase(task, phase, mode)?;
                info!("task #{} lint_fix: no lint command, skipping", task.id);
                return Ok(());
            },
        };

        const LINT_FIX_SYSTEM: &str = "You are a lint-fix agent. Your only job is to make the \
codebase pass the project's linter with zero warnings or errors. Do not refactor, rename, or \
change logic — only fix what the linter reports. Read the lint output carefully and make the \
minimal changes needed. After editing, do not run the linter yourself — the pipeline will verify.";

        let mut lint_out = self
            .run_test_command_for_task(task, &wt_path, &lint_cmd)
            .await?;
        if lint_out.exit_code == 0 {
            self.advance_phase(task, phase, mode)?;
            info!("task #{} lint_fix: already clean", task.id);
            return Ok(());
        }

        let session_dir = Self::task_session_dir(task);

        for fix_attempt in 0..2u32 {
            let lint_output_text = format!("{}\n{}", lint_out.stdout, lint_out.stderr)
                .trim()
                .to_string();

            info!(
                "task #{} lint_fix: running fix agent (attempt {})",
                task.id,
                fix_attempt + 1
            );

            let fix_phase = PhaseConfig {
                name: format!("lint_fix_{fix_attempt}"),
                label: "Lint Fix".into(),
                system_prompt: LINT_FIX_SYSTEM.into(),
                instruction: format!(
                    "Fix all lint errors. Lint output:\n\n```\n{lint_output_text}\n```\n\n\
Make only the minimal changes the linter requires. Do not refactor or change logic.",
                ),
                allowed_tools: "Read,Glob,Grep,Write,Edit,Bash".into(),
                use_docker: true,
                allow_no_changes: true,
                fresh_session: true,
                ..PhaseConfig::default()
            };

            let ctx = self.make_context(task, wt_path.clone(), session_dir.clone(), Vec::new());

            let agent_result = match self.resolve_backend(task) {
                Some(b) => {
                    if let Err(e) = self
                        .write_pipeline_state_snapshot(task, &fix_phase.name, &wt_path)
                        .await
                    {
                        warn!("task #{}: write_pipeline_state_snapshot: {e}", task.id);
                    }
                    self.run_backend_phase(&b, task, &fix_phase, ctx)
                        .await
                        .unwrap_or_else(|e| {
                            error!("lint-fix agent for task #{}: {e}", task.id);
                            PhaseOutput::failed(String::new())
                        })
                },
                None => {
                    warn!(
                        "task #{}: no backend, skipping lint fix attempt {}",
                        task.id, fix_attempt
                    );
                    self.advance_phase(task, phase, mode)?;
                    return Ok(());
                },
            };

            if let Some(ref sid) = agent_result.new_session_id {
                self.db.update_task_session(task.id, sid).ok();
            }
            self.db
                .insert_task_output(
                    task.id,
                    &fix_phase.name,
                    &agent_result.output,
                    &agent_result.raw_stream,
                    if agent_result.success { 0 } else { 1 },
                )
                .ok();

            let git = Git::new(&task.repo_path);
            let (_, user_coauthor) = self.git_coauthor_settings();
            let lint_commit_msg = Self::with_user_coauthor("fix: lint errors", &user_coauthor);
            let _ = git.commit_all(&wt_path, &lint_commit_msg, self.git_author());

            lint_out = self
                .run_test_command_for_task(task, &wt_path, &lint_cmd)
                .await?;
            if lint_out.exit_code == 0 {
                self.advance_phase(task, phase, mode)?;
                info!(
                    "task #{} lint_fix: clean after {} fix attempt(s)",
                    task.id,
                    fix_attempt + 1
                );
                return Ok(());
            }
        }

        let error_msg = format!("{}\n{}", lint_out.stdout, lint_out.stderr);
        self.fail_or_retry(task, "lint_fix", error_msg.trim())?;
        Ok(())
    }

    /// Inline compile-fix agent: tries up to 2 times to fix compile errors.
    /// Returns true if the compile check passes after fixing.
    async fn run_compile_fix(
        &self,
        task: &Task,
        work_dir: &str,
        check_cmd: &str,
        initial_errors: &str,
    ) -> Result<bool> {
        let session_dir = Self::task_session_dir(task);

        let mut errors = initial_errors.to_string();

        for attempt in 0..2u32 {
            info!("task #{} compile_fix: attempt {}", task.id, attempt + 1);

            let fix_phase = PhaseConfig {
                name: format!("compile_fix_{attempt}"),
                label: "Compile Fix".into(),
                system_prompt:
                    "You are a compile-error fix agent. Fix compile errors with minimal changes."
                        .into(),
                instruction: format!(
                    "The code does not compile. Fix the compile errors below.\n\
                     Make only the minimal changes needed to fix the errors.\n\
                     Do not refactor, rename, or change logic.\n\n\
                     ```\n{}\n```",
                    errors.chars().take(4000).collect::<String>()
                ),
                allowed_tools: "Read,Glob,Grep,Write,Edit,Bash".into(),
                use_docker: true,
                allow_no_changes: true,
                fresh_session: true,
                ..PhaseConfig::default()
            };

            let ctx =
                self.make_context(task, work_dir.to_string(), session_dir.clone(), Vec::new());

            let result = match self.resolve_backend(task) {
                Some(b) => self
                    .run_backend_phase(&b, task, &fix_phase, ctx)
                    .await
                    .unwrap_or_else(|e| {
                        error!("compile-fix agent for task #{}: {e}", task.id);
                        PhaseOutput::failed(String::new())
                    }),
                None => return Ok(false),
            };

            if let Some(ref sid) = result.new_session_id {
                self.db.update_task_session(task.id, sid).ok();
            }
            self.db
                .insert_task_output(
                    task.id,
                    &fix_phase.name,
                    &result.output,
                    &result.raw_stream,
                    if result.success { 0 } else { 1 },
                )
                .ok();

            let git = Git::new(&task.repo_path);
            let (_, user_coauthor) = self.git_coauthor_settings();
            let msg = Self::with_user_coauthor("fix: compile errors", &user_coauthor);
            let _ = git.commit_all(work_dir, &msg, self.git_author());

            match self
                .run_test_command_for_task(task, work_dir, check_cmd)
                .await
            {
                Ok(ref out) if out.exit_code == 0 => {
                    info!(
                        "task #{} compile_fix: resolved after {} attempt(s)",
                        task.id,
                        attempt + 1
                    );
                    return Ok(true);
                },
                Ok(ref out) => {
                    errors = format!("{}\n{}", out.stdout, out.stderr);
                },
                Err(e) => {
                    warn!("task #{} compile_fix: check command error: {e}", task.id);
                    return Ok(false);
                },
            }
        }

        Ok(false)
    }

    // ── Phase transition ──────────────────────────────────────────────────

    /// Advance a task to the next phase, or enqueue for integration when done.
    pub(crate) fn advance_phase(&self, task: &Task, phase: &PhaseConfig, mode: &PipelineMode) -> Result<()> {
        let next = phase.next.as_str();
        self.promote_session_privilege_on_phase2_transition(task, mode, next);
        if next == "done" || next == "human_review" {
            self.read_structured_output(task);
        }
        if next == "done" {
            self.index_task_documents(task);

            self.db.update_task_status(task.id, "done", Some(""))?;
            let _ = self.db.mark_task_completed(task.id);
            let pid = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            let _ = self.db.log_event_full(
                Some(task.id),
                None,
                pid,
                "pipeline",
                "task.completed",
                &serde_json::json!({ "title": task.title }),
            );

            let duration_str = self
                .db
                .get_task(task.id)
                .ok()
                .flatten()
                .and_then(|t| t.duration_secs)
                .map(|s| {
                    if s >= 3600 {
                        format!("{}h{}m", s / 3600, (s % 3600) / 60)
                    } else if s >= 60 {
                        format!("{}m{}s", s / 60, s % 60)
                    } else {
                        format!("{}s", s)
                    }
                })
                .unwrap_or_default();

            match mode.integration {
                IntegrationType::GitPr => {
                    let branch = format!("task-{}", task.id);
                    if let Err(e) = self
                        .db
                        .enqueue_or_requeue(task.id, &branch, &task.repo_path, 0)
                    {
                        warn!("enqueue for task #{}: {}", task.id, e);
                    } else {
                        info!("task #{} done, queued for integration", task.id);
                    }
                    if !task.notify_chat.is_empty() {
                        let msg = format!(
                            "Task #{} \"{}\" completed{}, queued for merge.",
                            task.id,
                            task.title,
                            if duration_str.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", duration_str)
                            },
                        );
                        self.notify(&task.notify_chat, &msg);
                    }
                },
                IntegrationType::GitBranch => {
                    info!("task #{} done, branch preserved", task.id);
                },
                IntegrationType::None => {
                    if !task.notify_chat.is_empty() {
                        let summary = self
                            .db
                            .get_task_structured_data(task.id)
                            .ok()
                            .filter(|s| !s.is_empty())
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                            .and_then(|v| {
                                v.get("summary").and_then(|s| s.as_str()).map(String::from)
                            });
                        let mut msg = format!(
                            "Task #{} \"{}\" completed{}.",
                            task.id,
                            task.title,
                            if duration_str.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", duration_str)
                            },
                        );
                        if let Some(ref sum) = summary {
                            msg.push_str(&format!("\n\n{}", sum));
                        }
                        self.notify(&task.notify_chat, &msg);
                    }
                },
            }
        } else {
            self.db.update_task_status(task.id, next, Some(""))?;
        }
        self.emit(PipelineEvent::Phase {
            task_id: Some(task.id),
            message: format!("task #{} advanced to '{}'", task.id, next),
        });
        Ok(())
    }

    fn promote_session_privilege_on_phase2_transition(
        &self,
        task: &Task,
        mode: &PipelineMode,
        next_status: &str,
    ) {
        if task.project_id <= 0 {
            return;
        }
        let is_legal_mode = matches!(task.mode.as_str(), "lawborg" | "legal")
            || matches!(mode.name.as_str(), "lawborg" | "legal");
        if !is_legal_mode {
            return;
        }
        if !Self::is_phase2_or_later(mode, next_status) {
            return;
        }
        if let Err(e) = self.db.set_session_privileged(task.project_id) {
            warn!(
                "task #{} failed to mark project {} as session_privileged on phase transition to '{}': {}",
                task.id, task.project_id, next_status, e
            );
        }
    }

    fn is_phase2_or_later(mode: &PipelineMode, status: &str) -> bool {
        let mut agent_phase_count = 0usize;
        for phase in &mode.phases {
            if phase.phase_type == PhaseType::Agent {
                agent_phase_count += 1;
            }
            if phase.name == status {
                return agent_phase_count >= 2;
            }
        }
        false
    }

    fn read_structured_output(&self, task: &Task) {
        if task.repo_path.is_empty() {
            return;
        }
        let branch = format!("task-{}", task.id);
        let path = std::path::Path::new(&task.repo_path);
        if !path.join(".git").exists() {
            return;
        }
        let out = std::process::Command::new("git")
            .args([
                "-C",
                &task.repo_path,
                "show",
                &format!("{branch}:structured.json"),
            ])
            .stderr(std::process::Stdio::null())
            .output();
        if let Ok(output) = out {
            if output.status.success() {
                let data = String::from_utf8_lossy(&output.stdout);
                let trimmed = data.trim();
                if !trimmed.is_empty() {
                    let merged = match self.db.get_task_structured_data(task.id) {
                        Ok(existing_raw) => {
                            let mut existing =
                                serde_json::from_str::<serde_json::Value>(&existing_raw)
                                    .unwrap_or_else(|_| serde_json::json!({}));
                            let fresh = serde_json::from_str::<serde_json::Value>(trimmed)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            if existing.is_object() && fresh.is_object() {
                                if let (Some(existing_obj), Some(fresh_obj)) =
                                    (existing.as_object_mut(), fresh.as_object())
                                {
                                    for (k, v) in fresh_obj {
                                        existing_obj.insert(k.clone(), v.clone());
                                    }
                                    serde_json::to_string(&existing)
                                        .unwrap_or_else(|_| trimmed.to_string())
                                } else {
                                    trimmed.to_string()
                                }
                            } else {
                                trimmed.to_string()
                            }
                        },
                        Err(_) => trimmed.to_string(),
                    };
                    if let Err(e) = self.db.update_task_structured_data(task.id, &merged) {
                        tracing::warn!("task #{}: failed to save structured data: {e}", task.id);
                    } else {
                        tracing::info!(
                            "task #{}: saved structured output ({} bytes)",
                            task.id,
                            trimmed.len()
                        );
                    }
                }
            }
        }
    }

    fn index_task_documents(&self, task: &Task) {
        if task.repo_path.is_empty() || task.project_id == 0 {
            return;
        }
        let branch = format!("task-{}", task.id);
        let path = std::path::Path::new(&task.repo_path);
        if !path.join(".git").exists() {
            return;
        }
        // List .md files on the task branch
        let out = std::process::Command::new("git")
            .args([
                "-C",
                &task.repo_path,
                "ls-tree",
                "-r",
                "--name-only",
                &branch,
            ])
            .stderr(std::process::Stdio::null())
            .output();
        let files = match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return,
        };
        // Clear old index for this task
        let _ = self.db.fts_remove_task(task.id);
        let mut count = 0;
        for file in files.lines() {
            if !file.ends_with(".md") {
                continue;
            }
            let show = std::process::Command::new("git")
                .args(["-C", &task.repo_path, "show", &format!("{branch}:{file}")])
                .stderr(std::process::Stdio::null())
                .output();
            if let Ok(o) = show {
                if o.status.success() {
                    let content = String::from_utf8_lossy(&o.stdout);
                    let title = content
                        .lines()
                        .next()
                        .unwrap_or(file)
                        .trim_start_matches('#')
                        .trim();
                    if let Err(e) =
                        self.db
                            .fts_index_document(task.project_id, task.id, file, title, &content)
                    {
                        tracing::warn!("task #{}: FTS index failed for {file}: {e}", task.id);
                    } else {
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            tracing::info!("task #{}: indexed {count} documents for FTS", task.id);
        }
    }

    // ── Pipeline state snapshot ───────────────────────────────────────────

    /// Write `.borg/pipeline-state.json` into the working directory before agent launch.
    /// Logs a warning and returns Ok(()) on any error so phase execution is
    /// never aborted by snapshot failures.
    async fn write_pipeline_state_snapshot(
        &self,
        task: &Task,
        phase_name: &str,
        work_dir: &str,
    ) -> Result<()> {
        // Build phase_history from last 5 task outputs, truncating output to 2 000 chars.
        let phase_history: Vec<PhaseHistoryEntry> = self
            .db
            .get_task_outputs(task.id)
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(5)
            .rev()
            .map(|o| PhaseHistoryEntry {
                phase: o.phase,
                success: o.exit_code == 0,
                output: o.output.chars().take(2_000).collect(),
                timestamp: o.created_at,
            })
            .collect();

        // Look up queue entries for this task to populate pending_approvals and pr_url.
        let queue_entries = self
            .db
            .get_queue_entries_for_task(task.id)
            .unwrap_or_default();

        let pending_approvals: Vec<String> = queue_entries
            .iter()
            .filter(|e| e.status == "pending_review")
            .map(|e| e.branch.clone())
            .collect();

        // Derive PR URL by calling `gh pr view` if any queue entry exists.
        let pr_url: Option<String> = if let Some(entry) = queue_entries.first() {
            let out = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                tokio::process::Command::new("gh")
                    .args(["pr", "view", &entry.branch, "--json", "url", "--jq", ".url"])
                    .stderr(std::process::Stdio::null())
                    .output(),
            )
            .await
            .ok()
            .and_then(|r| r.ok());
            out.and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
        } else {
            None
        };

        let snapshot = PipelineStateSnapshot {
            task_id: task.id,
            task_title: task.title.clone(),
            phase: phase_name.to_string(),
            worktree_path: work_dir.to_string(),
            pr_url,
            pending_approvals,
            phase_history,
            generated_at: Utc::now(),
        };

        let borg_dir = format!("{work_dir}/.borg");
        tokio::fs::create_dir_all(&borg_dir).await?;
        let json = serde_json::to_string_pretty(&snapshot)?;
        tokio::fs::write(format!("{borg_dir}/pipeline-state.json"), json).await?;

        Ok(())
    }

}
