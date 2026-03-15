use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use super::*;

impl Pipeline {
    // ── Main loop ─────────────────────────────────────────────────────────

    /// Main tick: dispatch ready tasks and run all periodic background work.
    pub async fn tick(self: Arc<Self>) -> Result<()> {
        // Reset integration_queue entries stuck in "merging" (crash mid-merge)
        if let Ok(n) = self.db.reset_stale_merging_queue() {
            if n > 0 {
                info!("Reset {n} stale merging integration_queue entries to queued");
            }
        }

        // Re-enqueue any "done" tasks that lost their queue entry (e.g. after restart)
        if let Ok(orphans) = self.db.list_done_tasks_without_queue() {
            for task in orphans {
                if let Some(mode) = self.resolve_mode(&task.mode) {
                    if mode.integration == IntegrationType::GitPr {
                        let branch = format!("task-{}", task.id);
                        if let Err(e) =
                            self.db
                                .enqueue_or_requeue(task.id, &branch, &task.repo_path, 0)
                        {
                            warn!("re-enqueue orphaned done task #{}: {e}", task.id);
                        } else {
                            info!(
                                "re-enqueued orphaned done task #{}: {}",
                                task.id, task.title
                            );
                        }
                    }
                }
            }
        }

        // Dispatch ready tasks
        // Skip dispatch entirely when draining for graceful shutdown
        if self.draining.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }

        let tasks = self.db.list_active_tasks().context("list_active_tasks")?;
        let max_agents = self.config.pipeline_max_agents as usize;
        let mut dispatched = 0usize;

        for task in tasks {
            if !self.task_ready_for_dispatch(&task) {
                continue;
            }
            let mut id_guard = self.in_flight.lock().await;
            if id_guard.len() >= max_agents {
                break;
            }
            if id_guard.contains(&task.id) {
                continue;
            }
            let mut repo_guard = self.in_flight_repos.lock().await;
            if repo_guard.contains(&task.repo_path) {
                continue;
            }
            id_guard.insert(task.id);
            repo_guard.insert(task.repo_path.clone());
            drop(repo_guard);
            drop(id_guard);

            dispatched += 1;
            let pipeline = Arc::clone(&self);
            let inner_pipeline = Arc::clone(&self);
            let task_id = task.id;
            let task_repo = task.repo_path.clone();
            let task_for_recovery = task.clone();
            tokio::spawn(async move {
                // Drop guard ensures in_flight slot is released even if this future is cancelled.
                struct InFlightGuard {
                    pipeline: Arc<Pipeline>,
                    task_id: i64,
                    task_repo: String,
                }
                impl Drop for InFlightGuard {
                    fn drop(&mut self) {
                        let pipeline = Arc::clone(&self.pipeline);
                        let task_id = self.task_id;
                        let task_repo = self.task_repo.clone();
                        tokio::spawn(async move {
                            pipeline.in_flight.lock().await.remove(&task_id);
                            pipeline.in_flight_repos.lock().await.remove(&task_repo);
                        });
                    }
                }
                let _guard = InFlightGuard {
                    pipeline: Arc::clone(&pipeline),
                    task_id,
                    task_repo,
                };

                let timeout_s = pipeline.task_wall_timeout_s();
                let mut handle =
                    tokio::spawn(
                        async move { Arc::clone(&inner_pipeline).process_task(task).await },
                    );
                match tokio::time::timeout(std::time::Duration::from_secs(timeout_s), &mut handle)
                    .await
                {
                    Ok(Ok(Ok(()))) => {},
                    Ok(Ok(Err(e))) => error!("process_task #{task_id} error: {e}"),
                    Ok(Err(join_err)) => {
                        let msg = if join_err.is_panic() {
                            let panic = join_err.into_panic();
                            match panic.downcast_ref::<String>() {
                                Some(s) => s.clone(),
                                None => match panic.downcast_ref::<&str>() {
                                    Some(s) => s.to_string(),
                                    None => "unknown panic".to_string(),
                                },
                            }
                        } else {
                            "task cancelled".to_string()
                        };
                        error!("process_task #{task_id} panicked: {msg}");
                        if let Err(e) = pipeline.fail_or_retry(
                            &task_for_recovery,
                            &task_for_recovery.status,
                            &format!("panic: {msg}"),
                        ) {
                            error!("process_task #{task_id} panic recovery DB update failed: {e}");
                        }
                    },
                    Err(_) => {
                        handle.abort();
                        let msg = format!("task wall timeout after {timeout_s}s");
                        error!("process_task #{task_id} timed out: {msg}");
                        if let Err(e) = pipeline.fail_or_retry(
                            &task_for_recovery,
                            &task_for_recovery.status,
                            &msg,
                        ) {
                            error!(
                                "process_task #{task_id} timeout recovery DB update failed: {e}"
                            );
                        }
                    },
                }
            });
        }

        if dispatched == 0 {
            // Hold the lock across the CAS so the emptiness check and the
            // seeding_active flip are jointly atomic with task dispatch.
            let guard = self.in_flight.lock().await;
            if guard.is_empty()
                && self
                    .seeding_active
                    .compare_exchange(
                        false,
                        true,
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Relaxed,
                    )
                    .is_ok()
            {
                drop(guard);
                let pipeline = Arc::clone(&self);
                tokio::spawn(async move {
                    if let Err(e) = pipeline.seed_if_idle().await {
                        warn!("seed_if_idle error: {e}");
                    }
                    pipeline
                        .seeding_active
                        .store(false, std::sync::atomic::Ordering::Release);
                });
            }
        }

        // Periodic background work (each is internally throttled)
        self.clone()
            .check_integration()
            .await
            .unwrap_or_else(|e| warn!("check_integration: {e}"));
        self.maybe_auto_promote_proposals();
        self.maybe_auto_triage().await;
        self.check_health()
            .await
            .unwrap_or_else(|e| warn!("check_health: {e}"));
        self.check_remote_updates().await;
        self.maybe_apply_self_update();
        self.refresh_mirrors().await;
        self.maybe_self_heal_tmp();
        self.maybe_alert_guardrails();
        self.maybe_prune_cache_volumes().await;
        self.maybe_prune_session_dirs().await;

        // Check if main loop should exit for self-update restart
        if self
            .force_restart
            .load(std::sync::atomic::Ordering::Acquire)
        {
            info!("force_restart flag set — returning error to trigger graceful shutdown");
            anyhow::bail!("force_restart");
        }

        Ok(())
    }
    // ── Task dispatch ─────────────────────────────────────────────────────

    /// Process a single task through its current phase.
    async fn process_task(self: Arc<Self>, task: Task) -> Result<()> {
        if let Some(wait_s) = self.should_defer_retry(task.id) {
            info!(
                "task #{} [{}] deferred by retry backoff ({}s remaining)",
                task.id, task.status, wait_s
            );
            return Ok(());
        }

        // Freshly requeued tasks should not inherit in-memory loop signatures
        // from previous failed runs.
        if task.attempt == 0 || task.status == "backlog" {
            self.clear_failure_signatures(task.id);
        }

        if let Some(latest) = self.db.get_task(task.id)? {
            if latest.status != task.status {
                info!(
                    "task #{} status changed from '{}' to '{}' before dispatch; skipping stale snapshot",
                    task.id, task.status, latest.status
                );
                let project_id = if latest.project_id > 0 {
                    Some(latest.project_id)
                } else {
                    None
                };
                let _ = self.db.log_event_full(
                    Some(task.id),
                    None,
                    project_id,
                    "pipeline",
                    "task.dispatch_stale_snapshot_skipped",
                    &serde_json::json!({
                        "snapshot_status": task.status,
                        "latest_status": latest.status,
                    }),
                );
                return Ok(());
            }
        }

        let mode = match self.resolve_mode(&task.mode) {
            Some(m) => m,
            None => {
                let err = format!("unknown pipeline mode: {}", task.mode);
                error!("task #{}: {err}", task.id);
                let _ = self.db.update_task_status(task.id, "failed", Some(&err));
                return Ok(());
            },
        };

        let phase = match mode.get_phase(&task.status) {
            Some(p) => p.clone(),
            None => {
                error!(
                    "task #{} has unknown phase '{}' for mode '{}'",
                    task.id, task.status, mode.name
                );
                return Ok(());
            },
        };

        if let Some(wait_s) = phase.wait_s {
            let ready_at = task.updated_at + chrono::Duration::seconds(wait_s.max(0));
            if Utc::now() < ready_at {
                return Ok(());
            }
        }

        // Rate-limit only agent phases (spawns a Claude subprocess).
        // Setup, Validate, LintFix, and Rebase are local ops — no cooldown needed.
        if phase.phase_type == PhaseType::Agent {
            let cooldown = self.config.pipeline_agent_cooldown_s;
            if cooldown > 0 {
                let now = Utc::now().timestamp();
                let mut map = self.last_agent_dispatch.lock().await;
                if let Some(&last) = map.get(&task.id) {
                    let elapsed = now - last;
                    if elapsed < cooldown {
                        info!(
                            "task #{} [{}] rate-limited ({elapsed}s/{cooldown}s), skipping",
                            task.id, task.status
                        );
                        return Ok(());
                    }
                }
                map.insert(task.id, now);
                // Prune stale entries to prevent unbounded growth
                if map.len() > 100 {
                    let cutoff = now - cooldown * 2;
                    map.retain(|_, &mut ts| ts > cutoff);
                }
            }
        }

        info!(
            "pipeline dispatching task #{} [{}] in {}: {}",
            task.id, task.status, task.repo_path, task.title
        );

        if phase.phase_type == PhaseType::Agent {
            let _ = self.db.mark_task_started(task.id);
        }

        match phase.phase_type {
            PhaseType::Setup => self.setup_branch(&task, &mode).await?,
            PhaseType::Agent => self.run_agent_phase(&task, &phase, &mode).await?,
            PhaseType::Validate => self.run_validate_phase(&task, &phase, &mode).await?,
            PhaseType::Rebase => self.run_rebase_phase(&task, &phase, &mode).await?,
            PhaseType::LintFix => self.run_lint_fix_phase(&task, &phase, &mode).await?,
            PhaseType::ComplianceCheck => {
                self.run_compliance_check_phase(&task, &phase, &mode)
                    .await?
            },
            PhaseType::HumanReview => {
                // Task sits in this status until a human acts via the API.
                // Do not dispatch to any backend — just return.
                return Ok(());
            },
            PhaseType::Purge => self.run_purge_phase(&task, &phase, &mode).await?,
        }

        // Async embedding indexing for completed tasks
        if phase.next == "done" && !task.repo_path.is_empty() {
            let db = Arc::clone(&self.db);
            let embed = self.embed_registry.client_for_mode(&task.mode);
            let pid = if task.project_id > 0 {
                Some(task.project_id)
            } else {
                None
            };
            crate::knowledge::index_task_embeddings(&db, embed, task.id, pid, &task.repo_path)
                .await;
        }

        self.clear_failure_signatures(task.id);

        Ok(())
    }
    // ── Mirror refresh ────────────────────────────────────────────────────

    /// Refresh bare mirrors for all watched repos at the configured interval.
    /// Mirrors are mounted read-only into containers to accelerate `git clone`.
    async fn refresh_mirrors(&self) {
        let interval = self.config.mirror_refresh_interval_s;
        if interval <= 0 {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        if now - self.db.get_ts("last_mirror_refresh_ts") < interval {
            return;
        }
        self.db.set_ts("last_mirror_refresh_ts", now);

        let mirrors_dir = format!("{}/mirrors", self.config.data_dir);
        if let Err(e) = std::fs::create_dir_all(&mirrors_dir) {
            warn!("refresh_mirrors: cannot create mirrors dir: {e}");
            return;
        }

        for repo in &self.config.watched_repos {
            let repo_name = std::path::Path::new(&repo.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if repo_name.is_empty() {
                continue;
            }
            let mirror_path = format!("{mirrors_dir}/{repo_name}.git");
            let path = repo.path.clone();
            let mirror = mirror_path.clone();
            tokio::spawn(async move {
                if !std::path::Path::new(&mirror).exists() {
                    let out = tokio::process::Command::new("git")
                        .args(["clone", "--mirror", &path, &mirror])
                        .output()
                        .await;
                    match out {
                        Ok(o) if o.status.success() => info!("mirrored {path} → {mirror}"),
                        Ok(o) => warn!(
                            "git clone --mirror failed for {path}: {}",
                            String::from_utf8_lossy(&o.stderr).trim()
                        ),
                        Err(e) => warn!("git clone --mirror spawn failed for {path}: {e}"),
                    }
                } else {
                    let out = tokio::process::Command::new("git")
                        .args(["-C", &mirror, "fetch", "--prune", "--tags"])
                        .output()
                        .await;
                    if let Ok(o) = out {
                        if !o.status.success() {
                            warn!(
                                "git fetch on mirror {mirror} failed: {}",
                                String::from_utf8_lossy(&o.stderr).trim()
                            );
                        }
                    }
                }
            });
        }
    }

    // ── Auto-promote + auto-triage ────────────────────────────────────────

    pub fn maybe_auto_promote_proposals(&self) {
        let active = self.db.active_task_count();
        let max = self.config.pipeline_max_backlog as i64;
        if active >= max {
            return;
        }
        let slots = max - active;
        let proposals = match self
            .db
            .get_top_scored_proposals(self.config.proposal_promote_threshold, slots)
        {
            Ok(p) => p,
            Err(e) => {
                warn!("auto_promote: {e}");
                return;
            },
        };
        for p in proposals {
            let repo_cfg = self
                .config
                .watched_repos
                .iter()
                .find(|r| r.path == p.repo_path);
            // Only auto-promote for repos that allow auto-merge
            if let Some(repo) = repo_cfg {
                if !repo.auto_merge {
                    continue;
                }
            }
            let mode = repo_cfg.map(|r| r.mode.as_str()).unwrap_or("sweborg");
            let task = crate::types::Task {
                id: 0,
                title: p.title.clone(),
                description: p.description.clone(),
                repo_path: p.repo_path.clone(),
                branch: String::new(),
                status: "backlog".into(),
                attempt: 0,
                max_attempts: 5,
                last_error: String::new(),
                created_by: "proposal".into(),
                notify_chat: String::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                session_id: String::new(),
                mode: mode.to_string(),
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
                    self.db.update_proposal_status(p.id, "approved").ok();
                    info!(
                        "Auto-promoted proposal #{} (score={}) → task #{}: {}",
                        p.id, p.triage_score, id, p.title
                    );
                },
                Err(e) => warn!("auto_promote insert_task: {e}"),
            }
        }
    }

    pub async fn maybe_auto_triage(&self) {
        const TRIAGE_INTERVAL_S: i64 = 6 * 3600;
        let now = chrono::Utc::now().timestamp();
        if now - self.db.get_ts("last_triage_ts") < TRIAGE_INTERVAL_S {
            return;
        }
        if self.db.count_unscored_proposals() == 0 {
            return;
        }
        self.db.set_ts("last_triage_ts", now);

        let proposals = match self.db.list_untriaged_proposals() {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };
        let merged = self.db.get_recent_merged_tasks(50).unwrap_or_default();

        let mut prompt = String::from(
            "Rate each proposal on 4 dimensions (1-5 scale), and flag proposals that should be auto-dismissed.\n\n\
            Dimensions:\n\
            - impact: How much value does this deliver? (5=critical, 1=cosmetic)\n\
            - feasibility: How likely is an AI agent to implement this correctly? (5=trivial, 1=needs human)\n\
            - risk: How likely to break existing functionality? (5=very risky, 1=safe)\n\
            - effort: How many agent cycles will this need? (5=massive, 1=simple)\n\n\
            Overall score formula: (impact*2 + feasibility*2 - risk - effort) mapped to 1-10.\n\n\
            Set \"dismiss\": true if: already implemented, duplicate, nonsensical, vague, or irrelevant.\n\n\
            Reply with ONLY a JSON array, no markdown fences:\n\
            [{\"id\": <n>, \"impact\": <1-5>, \"feasibility\": <1-5>, \"risk\": <1-5>, \"effort\": <1-5>, \"score\": <1-10>, \"reasoning\": \"<one sentence>\", \"dismiss\": <bool>}]\n\n",
        );
        if !merged.is_empty() {
            prompt.push_str("Recently merged tasks (for duplicate detection):\n");
            for t in &merged {
                prompt.push_str(&format!("- {}\n", t.title));
            }
            prompt.push('\n');
        }
        prompt.push_str("Proposals to evaluate:\n\n");
        for p in &proposals {
            prompt.push_str(&format!(
                "- ID {}: {}\n  Description: {}\n  Rationale: {}\n\n",
                p.id,
                p.title,
                if p.description.is_empty() {
                    "(none)"
                } else {
                    &p.description
                },
                if p.rationale.is_empty() {
                    "(none)"
                } else {
                    &p.rationale
                },
            ));
        }

        let output = self
            .run_claude_print(&self.config.triage_model, &prompt)
            .await;
        let output = match output {
            Ok(o) => o,
            Err(e) => {
                warn!("auto_triage: {e}");
                return;
            },
        };

        let arr_start = match output.find('[') {
            Some(i) => i,
            None => {
                warn!("auto_triage: no JSON array in output");
                return;
            },
        };
        let arr_end = match output.rfind(']') {
            Some(i) => i + 1,
            None => return,
        };
        let json_slice = &output[arr_start..arr_end];

        let items: Vec<serde_json::Value> = match serde_json::from_str(json_slice) {
            Ok(v) => v,
            Err(e) => {
                warn!("auto_triage: JSON parse failed: {e}");
                return;
            },
        };

        let mut scored = 0u32;
        let mut dismissed = 0u32;
        for item in &items {
            let Some((p_id, impact, feasibility, risk, effort, score, reasoning, should_dismiss)) =
                parse_triage_item(item)
            else {
                continue;
            };

            if let Err(e) = self.db.update_proposal_triage(
                p_id,
                score,
                impact,
                feasibility,
                risk,
                effort,
                reasoning,
            ) {
                warn!("auto_triage: update_proposal_triage #{p_id}: {e}");
                continue;
            }
            scored += 1;
            if should_dismiss {
                self.db.update_proposal_status(p_id, "auto_dismissed").ok();
                dismissed += 1;
                info!("Auto-triage: dismissed proposal #{p_id}: {reasoning}");
            }
        }
        info!(
            "Auto-triage: scored {scored}/{} proposals, dismissed {dismissed}",
            proposals.len()
        );
    }

    async fn maybe_prune_cache_volumes(&self) {
        const PRUNE_INTERVAL_S: i64 = 24 * 3600;
        let now = chrono::Utc::now().timestamp();
        let last = self
            .last_cache_prune_secs
            .load(std::sync::atomic::Ordering::Relaxed);
        if now - last < PRUNE_INTERVAL_S {
            return;
        }
        self.last_cache_prune_secs
            .store(now, std::sync::atomic::Ordering::Relaxed);
        Sandbox::prune_stale_cache_volumes(7).await;
    }

    async fn maybe_prune_session_dirs(&self) {
        const PRUNE_INTERVAL_S: i64 = 3600;
        let now = chrono::Utc::now().timestamp();
        let last = self
            .last_session_prune_secs
            .load(std::sync::atomic::Ordering::Relaxed);
        if now - last < PRUNE_INTERVAL_S {
            return;
        }
        self.last_session_prune_secs
            .store(now, std::sync::atomic::Ordering::Relaxed);

        let max_age_secs = self.config.session_max_age_hours * 3600;
        if max_age_secs <= 0 {
            return;
        }

        let sessions_dir = format!("{}/sessions", self.config.data_dir);
        let in_flight_ids: HashSet<i64> = self
            .in_flight
            .try_lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        let to_remove = collect_stale_session_dirs(
            &sessions_dir,
            now,
            max_age_secs,
            &in_flight_ids,
            |task_id| {
                self.db
                    .get_task(task_id)
                    .ok()
                    .flatten()
                    .map(|t| t.created_at.timestamp())
            },
        );

        let mut pruned = 0usize;
        for path in to_remove {
            match tokio::fs::remove_dir_all(&path).await {
                Ok(()) => pruned += 1,
                Err(e) => warn!("failed to remove session dir {}: {e}", path.display()),
            }
        }
        if pruned > 0 {
            info!("pruned {pruned} stale session dir(s) from {sessions_dir}");
        }
    }

    fn maybe_alert_guardrails(&self) {
        const ALERT_INTERVAL_S: i64 = 5 * 60;
        let now = chrono::Utc::now().timestamp();
        let last = self.db.get_ts("last_guardrail_check_ts");
        if now - last < ALERT_INTERVAL_S {
            return;
        }
        self.db.set_ts("last_guardrail_check_ts", now);

        let rebase_count = self.db.count_tasks_with_status("rebase").unwrap_or(0);
        if rebase_count >= 50 {
            let last_alert = self.db.get_ts("last_alert_rebase_backlog_ts");
            if now - last_alert >= 15 * 60 {
                self.db.set_ts("last_alert_rebase_backlog_ts", now);
                let msg = format!(
                    "Guardrail alert: rebase backlog is high ({rebase_count} tasks in rebase)."
                );
                warn!("{msg}");
                self.notify(&self.config.pipeline_admin_chat, &msg);
            }
        }

        let queued_count = self.db.count_queue_with_status("queued").unwrap_or(0)
            + self.db.count_queue_with_status("merging").unwrap_or(0);
        let last_merge_ts = self.db.get_ts("last_release_ts");
        let backlog_started_ts = self.db.get_ts("last_no_merge_backlog_started_ts");
        let (baseline_ts, next_backlog_started_ts) =
            no_merge_guardrail_baseline(queued_count, last_merge_ts, backlog_started_ts, now);
        if next_backlog_started_ts != backlog_started_ts {
            self.db
                .set_ts("last_no_merge_backlog_started_ts", next_backlog_started_ts);
        }
        if let Some(baseline_ts) = baseline_ts.filter(|baseline| now - baseline >= 60 * 60) {
            let last_alert = self.db.get_ts("last_alert_no_merge_ts");
            if now - last_alert >= 15 * 60 {
                self.db.set_ts("last_alert_no_merge_ts", now);
                let mins = (now - baseline_ts) / 60;
                let msg = format!(
                    "Guardrail alert: {queued_count} queued/merging entries and no merge for {mins} minutes."
                );
                warn!("{msg}");
                self.notify(&self.config.pipeline_admin_chat, &msg);
            }
        }

        if let Some(inode_used_pct) = tmp_inode_usage_percent("/tmp") {
            if inode_used_pct >= 90.0 {
                let last_alert = self.db.get_ts("last_alert_tmp_inode_ts");
                if now - last_alert >= 15 * 60 {
                    self.db.set_ts("last_alert_tmp_inode_ts", now);
                    let msg = format!(
                        "Guardrail alert: /tmp inode usage is high ({inode_used_pct:.1}%)."
                    );
                    warn!("{msg}");
                    self.notify(&self.config.pipeline_admin_chat, &msg);
                }
            }
        }
    }
}
