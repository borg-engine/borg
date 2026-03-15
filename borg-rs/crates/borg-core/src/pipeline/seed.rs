use anyhow::{Context, Result};
use tracing::{info, warn};

use super::*;

impl Pipeline {
    // ── Seed ─────────────────────────────────────────────────────────────

    pub(crate) async fn seed_if_idle(&self) -> Result<()> {
        if !self.config.continuous_mode {
            return Ok(());
        }

        let active = self.db.list_active_tasks()?.len() as u32;
        if active >= self.config.pipeline_max_backlog {
            return Ok(());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cooldown = self.config.pipeline_seed_cooldown_s;

        for repo in &self.config.watched_repos {
            if repo.is_self {
                let key = (repo.path.clone(), "github_open_issues".to_string());
                {
                    let mut cooldowns = self.seed_cooldowns.lock().await;
                    if now - cooldowns.get(&key).copied().unwrap_or(0) >= cooldown {
                        cooldowns.insert(key.clone(), now);
                        drop(cooldowns);
                        let _ = self.db.set_seed_cooldown(&key.0, &key.1, now);
                        info!("seed scan: 'github_open_issues' for {}", repo.path);
                        if let Err(e) = self.seed_from_open_issues(repo) {
                            warn!("seed github_open_issues for {}: {e}", repo.path);
                        }
                    }
                }
            }

            let mode = match self.resolve_mode(&repo.mode) {
                Some(m) => m,
                None => continue,
            };
            for seed_cfg in mode.seed_modes.clone() {
                // Non-primary repos only get proposal seeds — skip task seeds to avoid
                // creating automated pipeline tasks for repos we don't auto-merge.
                if !repo.is_self && seed_cfg.output_type == SeedOutputType::Task {
                    continue;
                }
                // Re-check backlog limit between seeds to avoid blocking for ages
                if let Ok(active) = self.db.list_active_tasks() {
                    if active.len() as u32 >= self.config.pipeline_max_backlog {
                        info!("seed: backlog full, stopping seed scan early");
                        return Ok(());
                    }
                }
                let key = (repo.path.clone(), seed_cfg.name.clone());
                {
                    let mut cooldowns = self.seed_cooldowns.lock().await;
                    if now - cooldowns.get(&key).copied().unwrap_or(0) < cooldown {
                        continue;
                    }
                    cooldowns.insert(key.clone(), now);
                }
                let _ = self.db.set_seed_cooldown(&key.0, &key.1, now);
                info!("seed scan: '{}' for {}", seed_cfg.name, repo.path);
                if let Err(e) = self.run_seed(repo, &mode.name, &seed_cfg).await {
                    warn!("seed {} for {}: {e}", seed_cfg.name, repo.path);
                }
            }
        }

        Ok(())
    }

    fn seed_from_open_issues(&self, repo: &RepoConfig) -> Result<()> {
        let gh_available = Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !gh_available {
            info!(
                "seed github_open_issues skipped for {}: gh CLI not available",
                repo.path
            );
            return Ok(());
        }

        let mode_name = match self.resolve_mode(&repo.mode) {
            Some(m) => m.name,
            None => {
                warn!(
                    "seed_from_open_issues: unknown pipeline mode {:?}, skipping",
                    repo.mode
                );
                return Ok(());
            },
        };

        let active = self.db.list_active_tasks()?.len() as i64;
        let available_slots = (self.config.pipeline_max_backlog as i64 - active).max(0) as usize;
        if available_slots == 0 {
            return Ok(());
        }

        let issues = self.fetch_open_issues(repo)?;
        if issues.is_empty() {
            return Ok(());
        }

        let existing_tasks = self.db.list_all_tasks(Some(&repo.path))?;
        let existing_proposals = self.db.list_all_proposals(Some(&repo.path))?;
        let mut created = 0usize;
        let mut skipped_existing = 0usize;

        for issue in issues {
            if created >= available_slots {
                break;
            }
            let marker = issue_seed_marker(&issue.url);
            let already_exists = existing_tasks
                .iter()
                .any(|t| t.description.contains(&marker))
                || existing_proposals
                    .iter()
                    .any(|p| p.rationale.contains(&marker));
            if already_exists {
                skipped_existing += 1;
                continue;
            }

            let labels = issue
                .labels
                .iter()
                .map(|l| l.name.trim())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            let label_line = if labels.is_empty() {
                String::new()
            } else {
                format!("Labels: {labels}\n\n")
            };

            let mut description = format!(
                "Imported from GitHub issue #{}.\n\n{}{}",
                issue.number,
                label_line,
                trim_issue_body(&issue.body)
            );
            description.push_str("\n\n");
            description.push_str(&marker);

            let task = Task {
                id: 0,
                title: format!("Issue #{}: {}", issue.number, issue.title.trim()),
                description,
                repo_path: repo.path.clone(),
                branch: String::new(),
                status: "backlog".to_string(),
                attempt: 0,
                max_attempts: 5,
                last_error: String::new(),
                created_by: "issue_seed".to_string(),
                notify_chat: String::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                session_id: String::new(),
                mode: mode_name.clone(),
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
            };
            match self.db.insert_task(&task) {
                Ok(id) => {
                    created += 1;
                    info!("seed github_open_issues created task #{id}: {}", task.title);
                },
                Err(e) => warn!("seed github_open_issues insert_task: {e}"),
            }
        }

        if created > 0 || skipped_existing > 0 {
            info!(
                "seed github_open_issues for {}: created={}, skipped_existing={}",
                repo.path, created, skipped_existing
            );
        }
        Ok(())
    }

    fn fetch_open_issues(&self, repo: &RepoConfig) -> Result<Vec<GithubIssue>> {
        let mut cmd = Command::new("gh");
        cmd.args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "100",
            "--json",
            "number,title,body,url,labels",
        ]);
        if !repo.repo_slug.is_empty() {
            cmd.args(["--repo", &repo.repo_slug]);
        } else if std::path::Path::new(&repo.path).exists() {
            cmd.current_dir(&repo.path);
        } else {
            anyhow::bail!("no repo_slug or local checkout for {}", repo.path);
        }
        let output = cmd
            .output()
            .with_context(|| format!("failed to execute `gh issue list` for {}", repo.path))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!("gh issue list failed for {}: {}", repo.path, stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let issues: Vec<GithubIssue> = serde_json::from_str(&stdout)
            .with_context(|| format!("failed to parse gh issue list JSON for {}", repo.path))?;
        Ok(issues)
    }

    async fn run_seed(
        &self,
        repo: &RepoConfig,
        mode_name: &str,
        seed_cfg: &crate::types::SeedConfig,
    ) -> Result<()> {
        let session_dir = std::fs::canonicalize("store/sessions/seed")
            .unwrap_or_else(|_| {
                std::fs::create_dir_all("store/sessions/seed").ok();
                std::fs::canonicalize("store/sessions/seed")
                    .unwrap_or_else(|_| std::path::PathBuf::from("store/sessions/seed"))
            })
            .to_string_lossy()
            .to_string();
        tokio::fs::create_dir_all(&session_dir).await.ok();

        let task = Task {
            id: 0,
            title: format!("seed:{}", seed_cfg.name),
            description: String::new(),
            repo_path: repo.path.clone(),
            branch: String::new(),
            status: "seed".to_string(),
            attempt: 0,
            max_attempts: 1,
            last_error: String::new(),
            created_by: "seed".to_string(),
            notify_chat: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            session_id: String::new(),
            mode: mode_name.to_string(),
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
        };

        let task_suffix =
            "\n\nFor each improvement, output EXACTLY this format (one per task):\n\n\
            TASK_START\n\
            Title: <short imperative title, max 80 chars>\n\
            Description: <2-4 sentences explaining what to change and why>\n\
            TASK_END\n\n\
            Output ONLY the task blocks above. No other text.";
        let proposal_suffix = "\n\nFor each proposal, output EXACTLY this format:\n\n\
            PROPOSAL_START\n\
            Title: <short imperative title, max 80 chars>\n\
            Description: <2-4 sentences explaining the feature or change>\n\
            Rationale: <1-2 sentences on why this would be valuable>\n\
            PROPOSAL_END\n\n\
            Output ONLY the proposal blocks above. No other text.";
        let preamble = "First, thoroughly explore the codebase before making any suggestions. \
            Use Read to examine key source files, Grep to search for patterns, \
            and Glob to discover the project structure. Understand the architecture, \
            existing patterns, and current state of the code.\n\nThen, based on your exploration:\n\n";
        let suffix = match seed_cfg.output_type {
            SeedOutputType::Task => task_suffix,
            SeedOutputType::Proposal => proposal_suffix,
        };
        let instruction = format!("{preamble}{}{suffix}", seed_cfg.prompt);

        let phase = PhaseConfig {
            name: format!("seed_{}", seed_cfg.name),
            label: seed_cfg.label.clone(),
            instruction,
            fresh_session: true,
            include_file_listing: true,
            allowed_tools: if seed_cfg.allowed_tools.is_empty() {
                "Read,Glob,Grep,Bash".to_string()
            } else {
                seed_cfg.allowed_tools.clone()
            },
            ..Default::default()
        };

        let ctx = self.make_context(&task, repo.path.clone(), session_dir, Vec::new());

        info!("running seed '{}' for {}", seed_cfg.name, repo.path);
        let backend = self
            .resolve_backend(&task)
            .ok_or_else(|| anyhow::anyhow!("no backends configured for seed"))?;
        let result = self.run_backend_phase(&backend, &task, &phase, ctx).await?;

        if !result.success {
            warn!(
                "seed '{}' for {} failed (output: {:?})",
                seed_cfg.name, repo.path, &result.output
            );
        } else {
            info!("seed '{}' output: {:?}", seed_cfg.name, &result.output);
        }

        let target_repo = if seed_cfg.target_primary_repo {
            self.config
                .watched_repos
                .iter()
                .find(|r| r.is_self)
                .map(|r| r.path.as_str())
                .unwrap_or(&repo.path)
        } else {
            &repo.path
        };
        self.parse_seed_output(&result.output, target_repo, mode_name, seed_cfg.output_type)?;
        Ok(())
    }

    fn parse_seed_output(
        &self,
        output: &str,
        repo_path: &str,
        mode_name: &str,
        output_type: SeedOutputType,
    ) -> Result<()> {
        match output_type {
            SeedOutputType::Task => {
                for block in extract_blocks(output, "TASK_START", "TASK_END") {
                    let title = extract_field(&block, "Title:").unwrap_or_default();
                    let description = extract_field(&block, "Description:").unwrap_or_default();
                    if title.is_empty() {
                        continue;
                    }
                    let task = Task {
                        id: 0,
                        title,
                        description,
                        repo_path: repo_path.to_string(),
                        branch: String::new(),
                        status: "backlog".to_string(),
                        attempt: 0,
                        max_attempts: 5,
                        last_error: String::new(),
                        created_by: "seed".to_string(),
                        notify_chat: String::new(),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        session_id: String::new(),
                        mode: mode_name.to_string(),
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
                    };
                    match self.db.insert_task(&task) {
                        Ok(id) => info!("seed created task #{id}: {}", task.title),
                        Err(e) => warn!("seed insert_task: {e}"),
                    }
                }
            },
            SeedOutputType::Proposal => {
                for block in extract_blocks(output, "PROPOSAL_START", "PROPOSAL_END") {
                    let title = extract_field(&block, "Title:").unwrap_or_default();
                    let description = extract_field(&block, "Description:").unwrap_or_default();
                    let rationale = extract_field(&block, "Rationale:").unwrap_or_default();
                    if title.is_empty() {
                        continue;
                    }
                    let proposal = Proposal {
                        id: 0,
                        repo_path: repo_path.to_string(),
                        title,
                        description,
                        rationale,
                        status: "proposed".to_string(),
                        created_at: Utc::now(),
                        triage_score: 0,
                        triage_impact: 0,
                        triage_feasibility: 0,
                        triage_risk: 0,
                        triage_effort: 0,
                        triage_reasoning: String::new(),
                    };
                    match self.db.insert_proposal(&proposal) {
                        Ok(id) => info!("seed created proposal #{id}: {}", proposal.title),
                        Err(e) => warn!("seed insert_proposal: {e}"),
                    }
                }
            },
        }
        Ok(())
    }


    /// Run `claude --print --model <model>` with prompt on stdin, return stdout.
    pub(crate) async fn run_claude_print(&self, model: &str, prompt: &str) -> Result<String> {
        use tokio::io::AsyncWriteExt;
        let mut child = tokio::process::Command::new("claude")
            .args([
                "--print",
                "--model",
                model,
                "--permission-mode",
                "bypassPermissions",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .env("CLAUDE_CODE_OAUTH_TOKEN", &self.config.oauth_token)
            .spawn()
            .context("spawn claude --print")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await.ok();
        }
        let out = child
            .wait_with_output()
            .await
            .context("wait claude --print")?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}
