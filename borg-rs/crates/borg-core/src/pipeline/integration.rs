use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{info, warn};

use super::*;

impl Pipeline {
    // ── Integration merge ─────────────────────────────────────────────────

    pub(crate) async fn check_integration(self: &Arc<Self>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let last = self.db.get_ts("last_release_ts");
        let min_interval = if self.config.pipeline.continuous_mode {
            60i64
        } else {
            self.config.pipeline.release_interval_mins as i64 * 60
        };
        if now - last < min_interval {
            return Ok(());
        }

        let mut any_merged = false;
        for repo in &self.config.watched_repos {
            let queued = self.db.get_queued_branches_for_repo(&repo.path)?;
            if queued.is_empty() {
                continue;
            }
            if repo.repo_slug.is_empty() {
                warn!("Integration: no repo_slug for {}, skipping", repo.path);
                continue;
            }
            info!("Integration: {} branches for {}", queued.len(), repo.path);
            match self
                .run_integration(queued, &repo.repo_slug, repo.auto_merge)
                .await
            {
                Ok(merged) => any_merged |= merged,
                Err(e) => warn!("Integration error for {}: {e}", repo.path),
            }
        }

        // Only reset the release timer when merges actually happened.
        // If integration ran but only sent branches to rebase (no merges),
        // skip resetting so we re-check promptly after rebase completes.
        if any_merged {
            self.db
                .set_ts("last_release_ts", chrono::Utc::now().timestamp());
        }
        Ok(())
    }

    /// Run a `gh` command without a working directory.
    pub(crate) async fn gh(&self, args: &[&str]) -> Result<TestOutput> {
        let timeout = std::time::Duration::from_secs(self.config.agent_timeout_s.max(300) as u64);
        let mut cmd = tokio::process::Command::new("gh");
        cmd.args(args);
        if !self.config.git.github_token.is_empty() {
            cmd.env("GH_TOKEN", &self.config.git.github_token);
        }
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "gh {} timed out after {}s",
                    args.join(" "),
                    timeout.as_secs()
                )
            })?
            .context("gh command")?;
        Ok(TestOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(1),
        })
    }

    /// Returns true if any branches were actually merged.
    async fn run_integration(
        &self,
        queued: Vec<crate::types::QueueEntry>,
        slug: &str,
        auto_merge: bool,
    ) -> Result<bool> {
        let mut live = Vec::new();
        for entry in queued {
            let check = self
                .gh(&[
                    "api",
                    "--silent",
                    &format!("repos/{slug}/branches/{}", entry.branch),
                ])
                .await;
            if check.map(|r| r.exit_code == 0).unwrap_or(false) {
                live.push(entry);
            } else {
                warn!(
                    "Excluding {} from integration: branch not found on remote",
                    entry.branch
                );
                self.db
                    .update_queue_status_with_error(entry.id, "excluded", "branch not found")?;
            }
        }
        if live.is_empty() {
            return Ok(false);
        }

        let mut excluded_ids: HashSet<i64> = HashSet::new();
        let mut freshly_created: HashSet<i64> = HashSet::new();

        for entry in &live {
            // Check if already merged on GitHub
            let state_out = self
                .gh(&[
                    "pr",
                    "view",
                    &entry.branch,
                    "--repo",
                    slug,
                    "--json",
                    "state",
                    "--jq",
                    ".state",
                ])
                .await;
            if let Ok(ref o) = state_out {
                let s = o.stdout.trim();
                if s == "MERGED" {
                    info!(
                        "Task #{} {}: PR already merged",
                        entry.task_id, entry.branch
                    );
                    self.db.update_queue_status(entry.id, "merged")?;
                    self.db.update_task_status(entry.task_id, "merged", None)?;
                    excluded_ids.insert(entry.id);
                    continue;
                }
                // CLOSED + identical to main → squash-merged
                if s == "CLOSED" {
                    let cmp = self
                        .gh(&[
                            "api",
                            &format!("repos/{slug}/compare/main...{}", entry.branch),
                            "--jq",
                            ".status",
                        ])
                        .await;
                    if cmp.map(|r| r.stdout.trim() == "identical").unwrap_or(false) {
                        info!(
                            "Task #{} {}: identical to main, marking merged",
                            entry.task_id, entry.branch
                        );
                        self.db.update_queue_status(entry.id, "merged")?;
                        self.db.update_task_status(entry.task_id, "merged", None)?;
                        excluded_ids.insert(entry.id);
                        continue;
                    }
                    // Closed but not identical: attempt reopen so the branch can re-enter merge flow.
                    let pr_num = self
                        .gh(&[
                            "pr",
                            "view",
                            &entry.branch,
                            "--repo",
                            slug,
                            "--json",
                            "number",
                            "--jq",
                            ".number",
                        ])
                        .await
                        .ok()
                        .map(|o| o.stdout.trim().to_string())
                        .filter(|s| !s.is_empty());
                    if let Some(num) = pr_num {
                        let reopened = self
                            .gh(&["pr", "reopen", &num, "--repo", slug])
                            .await
                            .ok()
                            .filter(|o| o.exit_code == 0);
                        if reopened.is_some() {
                            info!(
                                "Task #{} {}: reopened closed PR #{}",
                                entry.task_id, entry.branch, num
                            );
                            continue;
                        }
                    }
                }
            }

            // Check if PR already exists
            let view_out = self
                .gh(&[
                    "pr",
                    "view",
                    &entry.branch,
                    "--repo",
                    slug,
                    "--json",
                    "number,state",
                    "--jq",
                    ".state + \" \" + (.number|tostring)",
                ])
                .await;
            let view_out = match view_out {
                Ok(o) => o,
                Err(e) => {
                    warn!("gh pr view {}: {e}", entry.branch);
                    continue;
                },
            };
            if view_out.exit_code == 0 && !view_out.stdout.trim().is_empty() {
                let mut parts = view_out.stdout.split_whitespace();
                let state = parts.next().unwrap_or_default();
                let number = parts.next().unwrap_or_default();
                if state == "OPEN" {
                    continue;
                }
                if state == "CLOSED" && !number.is_empty() {
                    let reopened = self
                        .gh(&["pr", "reopen", number, "--repo", slug])
                        .await
                        .ok()
                        .filter(|o| o.exit_code == 0);
                    if reopened.is_some() {
                        info!(
                            "Task #{} {}: reopened PR #{}",
                            entry.task_id, entry.branch, number
                        );
                        continue;
                    }
                }
            }

            // Get task for PR title and attribution
            let task_row = self.db.get_task(entry.task_id)?;
            let title = task_row
                .as_ref()
                .map(|t| t.title.chars().take(100).collect::<String>())
                .unwrap_or_else(|| entry.branch.clone());
            let created_by = task_row
                .as_ref()
                .map(|t| t.created_by.as_str())
                .unwrap_or_default()
                .to_string();

            let (_, is_user_token) = self.resolve_gh_token(&created_by);
            let body = if !is_user_token && !created_by.is_empty() {
                format!(
                    "Automated implementation.\n\n---\n\
                     Submitted by Borg on behalf of **{}**",
                    created_by
                )
            } else {
                "Automated implementation.".to_string()
            };

            let create_out = self
                .gh(&[
                    "pr",
                    "create",
                    "--repo",
                    slug,
                    "--base",
                    "main",
                    "--head",
                    &entry.branch,
                    "--title",
                    &title,
                    "--body",
                    &body,
                ])
                .await;
            let create_out = match create_out {
                Ok(o) => o,
                Err(e) => {
                    warn!("gh pr create {}: {e}", entry.branch);
                    continue;
                },
            };

            if create_out.exit_code != 0 {
                let err = &create_out.stderr[..create_out.stderr.len().min(300)];
                if err.contains("No commits between") {
                    info!(
                        "Task #{} {}: no commits vs main, marking merged",
                        entry.task_id, entry.branch
                    );
                    self.db.update_queue_status(entry.id, "merged")?;
                    self.db.update_task_status(entry.task_id, "merged", None)?;
                    excluded_ids.insert(entry.id);
                } else {
                    warn!("gh pr create {}: {}", entry.branch, err);
                }
            } else {
                info!("Created PR for {}", entry.branch);
                freshly_created.insert(entry.id);
            }
        }

        let mut merged_branches: Vec<String> = Vec::new();

        if !auto_merge {
            for entry in &live {
                if excluded_ids.contains(&entry.id) {
                    continue;
                }
                self.db.update_queue_status(entry.id, "pending_review")?;
                info!(
                    "Task #{} {}: PR ready for manual review",
                    entry.task_id, entry.branch
                );
            }
        } else {
            // ── Merge queue: serialize to one merge per cycle ──────────────
            //
            // Pick the oldest non-excluded, non-freshly-created entry. Verify
            // it is current with main (behind_by == 0) before merging. A branch
            // rebased onto main N has behind_by=0 and will fast-forward onto N,
            // producing an identical file tree to what the compile check tested.
            // If any other PR was merged since the rebase, behind_by > 0 and we
            // send the branch back to rebase rather than risk a corrupted merge.
            let candidate = live
                .iter()
                .find(|e| !excluded_ids.contains(&e.id) && !freshly_created.contains(&e.id));

            if let Some(entry) = candidate {
                // Check if PR is already merged (picked up from a prior run)
                let state_check = self
                    .gh(&[
                        "pr",
                        "view",
                        &entry.branch,
                        "--repo",
                        slug,
                        "--json",
                        "state",
                        "--jq",
                        ".state",
                    ])
                    .await;
                let pr_state = state_check
                    .as_ref()
                    .map(|o| o.stdout.trim().to_string())
                    .unwrap_or_default();

                if pr_state == "MERGED" {
                    info!("Task #{} {}: already merged", entry.task_id, entry.branch);
                    self.db.update_queue_status(entry.id, "merged")?;
                    self.db.update_task_status(entry.task_id, "merged", None)?;
                    merged_branches.push(entry.branch.clone());
                } else {
                    // Check how far behind main this branch is.
                    // behind_by == 0 means the branch was rebased onto current main tip.
                    // A fast-forward merge then produces exactly what the rebase compile
                    // check tested — no new conflicts can arise.
                    let compare = self
                        .gh(&[
                            "api",
                            &format!("repos/{slug}/compare/main...{}", entry.branch),
                            "--jq",
                            ".behind_by",
                        ])
                        .await;
                    let behind_by: u64 = compare
                        .as_ref()
                        .ok()
                        .and_then(|o| o.stdout.trim().parse().ok())
                        .unwrap_or(1); // default conservative: treat unknown as stale

                    if behind_by > 0 {
                        info!(
                            "Task #{} {}: behind main by {}, sending to rebase",
                            entry.task_id, entry.branch, behind_by
                        );
                        self.db.update_queue_status_with_error(
                            entry.id,
                            "excluded",
                            "behind main — rebase required",
                        )?;
                        self.db.update_task_status(entry.task_id, "rebase", None)?;
                    } else {
                        // behind_by == 0 → safe to fast-forward merge
                        self.db.update_queue_status(entry.id, "merging")?;
                        let merge_out = self
                            .gh(&["pr", "merge", &entry.branch, "--repo", slug, "--merge"])
                            .await;

                        match merge_out {
                            Err(e) => {
                                warn!("gh pr merge {}: {e}", entry.branch);
                                self.db.update_queue_status(entry.id, "queued")?;
                            },
                            Ok(out) if out.exit_code != 0 => {
                                let err = &out.stderr[..out.stderr.len().min(200)];
                                warn!("gh pr merge {}: {}", entry.branch, err);
                                if err.contains("not mergeable")
                                    || err.contains("cannot be cleanly created")
                                {
                                    self.db.update_queue_status_with_error(
                                        entry.id,
                                        "excluded",
                                        "merge conflict with main",
                                    )?;
                                    self.db.update_task_status(entry.task_id, "rebase", None)?;
                                    info!("Task #{} has conflicts, sent to rebase", entry.task_id);
                                } else {
                                    self.db.update_queue_status(entry.id, "queued")?;
                                }
                            },
                            Ok(_) => {
                                self.db.update_queue_status(entry.id, "merged")?;
                                self.db.update_task_status(entry.task_id, "merged", None)?;
                                merged_branches.push(entry.branch.clone());
                                let _ = self
                                    .gh(&[
                                        "api",
                                        "-X",
                                        "DELETE",
                                        &format!("repos/{slug}/git/refs/heads/{}", entry.branch),
                                    ])
                                    .await;
                                if let Ok(Some(task)) = self.db.get_task(entry.task_id) {
                                    let duration_str = task
                                        .duration_secs
                                        .map(|s| {
                                            if s >= 3600 {
                                                format!(" ({}h{}m)", s / 3600, (s % 3600) / 60)
                                            } else if s >= 60 {
                                                format!(" ({}m{}s)", s / 60, s % 60)
                                            } else {
                                                format!(" ({}s)", s)
                                            }
                                        })
                                        .unwrap_or_default();
                                    self.notify(
                                        &task.notify_chat,
                                        &format!(
                                            "Task #{} \"{}\" merged via PR{}.",
                                            task.id, task.title, duration_str
                                        ),
                                    );
                                }
                            },
                        }
                    }
                }
            }
        }

        if !merged_branches.is_empty() {
            let digest = self.generate_digest(&merged_branches);
            self.notify(&self.config.pipeline.admin_chat, &digest);
            info!("Integration complete: {} merged", merged_branches.len());
        }

        Ok(!merged_branches.is_empty())
    }

    fn generate_digest(&self, merged: &[String]) -> String {
        let mut s = format!("*{} PR(s) merged*\n", merged.len());
        for branch in merged {
            s.push_str(&format!("  + {branch}\n"));
        }
        s
    }

}
