use super::*;

impl Db {
    // ── Proposals ─────────────────────────────────────────────────────────

    pub fn list_proposals(&self, repo_path: &str) -> Result<Vec<Proposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, repo_path, title, description, rationale, status, created_at, \
             triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
             triage_reasoning \
             FROM proposals WHERE repo_path = ?1 ORDER BY id ASC",
        )?;
        let proposals = stmt
            .query_map(params![repo_path], row_to_proposal)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_proposals")?;
        Ok(proposals)
    }

    pub fn list_all_proposals(&self, repo_path: Option<&str>) -> Result<Vec<Proposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = if repo_path.is_some() {
            "SELECT id, repo_path, title, description, rationale, status, created_at, \
             triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
             triage_reasoning \
             FROM proposals \
             WHERE repo_path = ?1 \
             ORDER BY id DESC"
        } else {
            "SELECT id, repo_path, title, description, rationale, status, created_at, \
             triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
             triage_reasoning \
             FROM proposals \
             ORDER BY id DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let proposals = if let Some(repo_path) = repo_path {
            stmt.query_map(params![repo_path], row_to_proposal)?
                .collect::<pg::Result<Vec<_>>>()
        } else {
            stmt.query_map([], row_to_proposal)?
                .collect::<pg::Result<Vec<_>>>()
        }
        .context("list_all_proposals")?;
        Ok(proposals)
    }

    pub fn get_proposal(&self, id: i64) -> Result<Option<Proposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT id, repo_path, title, description, rationale, status, created_at, \
                 triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
                 triage_reasoning \
                 FROM proposals WHERE id = ?1",
                params![id],
                row_to_proposal,
            )
            .optional()
            .context("get_proposal")?;
        Ok(result)
    }

    pub fn task_stats(&self) -> Result<(i64, i64, i64, i64)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM pipeline_tasks", [], |r| r.get(0))
            .context("task_stats total")?;
        let active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pipeline_tasks WHERE status NOT IN ('done','merged','failed','blocked','pending_review','human_review','purged')",
                [],
                |r| r.get(0),
            )
            .context("task_stats active")?;
        let merged: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pipeline_tasks WHERE status = 'merged'",
                [],
                |r| r.get(0),
            )
            .context("task_stats merged")?;
        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pipeline_tasks WHERE status = 'failed'",
                [],
                |r| r.get(0),
            )
            .context("task_stats failed")?;
        Ok((active, merged, failed, total))
    }

    pub fn count_tasks_with_status(&self, status: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pipeline_tasks WHERE status = ?1",
                params![status],
                |r| r.get(0),
            )
            .context("count_tasks_with_status")?;
        Ok(n)
    }

    pub fn project_task_status_counts(&self, project_id: i64) -> Result<ProjectTaskCounts> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM pipeline_tasks WHERE project_id = ?1 GROUP BY status",
        )?;
        let mut counts = ProjectTaskCounts::default();
        let rows = stmt.query_map(params![project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (status, n) = row?;
            match status.as_str() {
                "running" | "backlog" => counts.active += n,
                "human_review" => counts.review += n,
                "done" => counts.done += n,
                "failed" => counts.failed += n,
                _ => {},
            }
        }
        counts.total = counts.active + counts.review + counts.done + counts.failed;
        Ok(counts)
    }

    pub fn count_queue_with_status(&self, status: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM integration_queue WHERE status = ?1",
                params![status],
                |r| r.get(0),
            )
            .context("count_queue_with_status")?;
        Ok(n)
    }

    pub fn insert_proposal(&self, proposal: &Proposal) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = proposal.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let id = conn
            .execute_returning_id(
                "INSERT INTO proposals \
             (repo_path, title, description, rationale, status, created_at, \
              triage_score, triage_impact, triage_feasibility, triage_risk, \
              triage_effort, triage_reasoning) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    proposal.repo_path,
                    proposal.title,
                    proposal.description,
                    proposal.rationale,
                    proposal.status,
                    created_at,
                    proposal.triage_score,
                    proposal.triage_impact,
                    proposal.triage_feasibility,
                    proposal.triage_risk,
                    proposal.triage_effort,
                    proposal.triage_reasoning,
                ],
            )
            .context("insert_proposal")?;
        Ok(id)
    }

    pub fn update_proposal_status(&self, id: i64, status: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE proposals SET status = ?1 WHERE id = ?2",
            params![status, id],
        )
        .context("update_proposal_status")?;
        Ok(())
    }

    pub fn update_proposal_triage(
        &self,
        id: i64,
        score: i64,
        impact: i64,
        feasibility: i64,
        risk: i64,
        effort: i64,
        reasoning: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE proposals SET triage_score=?1, triage_impact=?2, triage_feasibility=?3, \
             triage_risk=?4, triage_effort=?5, triage_reasoning=?6 WHERE id=?7",
            params![score, impact, feasibility, risk, effort, reasoning, id],
        )
        .context("update_proposal_triage")?;
        Ok(())
    }

    // ── Citation verifications ──────────────────────────────────────────

    pub fn insert_citation_verification(
        &self,
        task_id: i64,
        citation_text: &str,
        citation_type: &str,
        status: &str,
        source: &str,
        treatment: &str,
        checked_at: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id = conn.execute_returning_id(
            "INSERT INTO citation_verifications (task_id, citation_text, citation_type, status, source, treatment, checked_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![task_id, citation_text, citation_type, status, source, treatment, checked_at],
        )?;
        Ok(id)
    }

    pub fn get_task_citations(&self, task_id: i64) -> Result<Vec<CitationVerification>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, citation_text, citation_type, status, source, treatment, checked_at, created_at \
             FROM citation_verifications WHERE task_id = ?1 ORDER BY id"
        )?;
        let rows = stmt
            .query_map(params![task_id], |r: &pg::Row| {
                Ok(CitationVerification {
                    id: r.get(0)?,
                    task_id: r.get(1)?,
                    citation_text: r.get(2)?,
                    citation_type: r.get(3)?,
                    status: r.get(4)?,
                    source: r.get(5)?,
                    treatment: r.get(6)?,
                    checked_at: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    created_at: r.get::<_, String>(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn delete_task_citations(&self, task_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM citation_verifications WHERE task_id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    pub fn get_top_scored_proposals(&self, threshold: i64, limit: i64) -> Result<Vec<Proposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, repo_path, title, description, rationale, status, created_at, \
             triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
             triage_reasoning \
             FROM proposals WHERE status='proposed' AND triage_score >= ?1 \
             ORDER BY triage_score DESC LIMIT ?2",
        )?;
        let proposals = stmt
            .query_map(params![threshold, limit], row_to_proposal)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_top_scored_proposals")?;
        Ok(proposals)
    }

    pub fn count_unscored_proposals(&self) -> i64 {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row(
            "SELECT COUNT(*) FROM proposals WHERE status='proposed' AND triage_score=0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    pub fn list_untriaged_proposals(&self) -> Result<Vec<Proposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, repo_path, title, description, rationale, status, created_at, \
             triage_score, triage_impact, triage_feasibility, triage_risk, triage_effort, \
             triage_reasoning \
             FROM proposals WHERE status='proposed' AND triage_score=0 ORDER BY id ASC",
        )?;
        let proposals = stmt
            .query_map([], row_to_proposal)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_untriaged_proposals")?;
        Ok(proposals)
    }

    // ── Merge Queue ───────────────────────────────────────────────────────

    pub fn list_queue(&self) -> Result<Vec<QueueEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, branch, repo_path, status, queued_at, pr_number \
             FROM integration_queue WHERE status = 'queued' ORDER BY id ASC",
        )?;
        let entries = stmt
            .query_map([], row_to_queue_entry)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_queue")?;
        Ok(entries)
    }

    pub fn enqueue(
        &self,
        task_id: i64,
        branch: &str,
        repo_path: &str,
        pr_number: i64,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let queued_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO integration_queue (task_id, branch, repo_path, status, queued_at, pr_number) \
             VALUES (?1, ?2, ?3, 'queued', ?4, ?5)",
            params![task_id, branch, repo_path, queued_at, pr_number],
        )
        .context("enqueue")?;
        Ok(id)
    }

    /// Ensure a task/branch has exactly one active queue entry.
    ///
    /// If an existing non-merged row exists, it is recycled back to `queued`
    /// instead of inserting another row. This prevents unbounded queue growth
    /// when tasks repeatedly cycle through done -> rebase -> done.
    pub fn enqueue_or_requeue(
        &self,
        task_id: i64,
        branch: &str,
        repo_path: &str,
        pr_number: i64,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM integration_queue \
                 WHERE task_id = ?1 AND branch = ?2 AND status IN ('queued','merging','excluded','pending_review') \
                 ORDER BY id DESC LIMIT 1",
                params![task_id, branch],
                |r| r.get(0),
            )
            .optional()
            .context("enqueue_or_requeue select existing")?;

        let queued_at = now_str();
        if let Some(id) = existing {
            conn.execute(
                "UPDATE integration_queue
                 SET status = 'queued',
                     repo_path = ?1,
                     queued_at = ?2,
                     pr_number = ?3,
                     error_msg = '',
                     unknown_retries = 0
                 WHERE id = ?4",
                params![repo_path, queued_at, pr_number, id],
            )
            .context("enqueue_or_requeue update existing")?;
            return Ok(id);
        }

        let id = conn.execute_returning_id(
            "INSERT INTO integration_queue (task_id, branch, repo_path, status, queued_at, pr_number) \
             VALUES (?1, ?2, ?3, 'queued', ?4, ?5)",
            params![task_id, branch, repo_path, queued_at, pr_number],
        )
        .context("enqueue_or_requeue insert")?;
        Ok(id)
    }

    pub fn update_queue_status(&self, id: i64, status: &str) -> Result<()> {
        self.update_queue_status_with_error(id, status, "")
    }

    pub fn update_queue_status_with_error(
        &self,
        id: i64,
        status: &str,
        error_msg: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE integration_queue SET status = ?1, error_msg = ?2 WHERE id = ?3",
            params![status, error_msg, id],
        )
        .context("update_queue_status_with_error")?;
        Ok(())
    }

    pub fn get_queued_branches_for_repo(&self, repo_path: &str) -> Result<Vec<QueueEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, branch, repo_path, status, queued_at, pr_number \
             FROM integration_queue WHERE repo_path = ?1 AND status = 'queued' ORDER BY task_id ASC",
        )?;
        let entries = stmt
            .query_map(params![repo_path], row_to_queue_entry)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_queued_branches_for_repo")?;
        Ok(entries)
    }

    pub fn get_queue_entries_for_task(&self, task_id: i64) -> Result<Vec<QueueEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, branch, repo_path, status, queued_at, pr_number \
             FROM integration_queue WHERE task_id = ?1 ORDER BY id ASC",
        )?;
        let entries = stmt
            .query_map(params![task_id], row_to_queue_entry)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_queue_entries_for_task")?;
        Ok(entries)
    }

    pub fn get_unknown_retries(&self, id: i64) -> i64 {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row(
            "SELECT unknown_retries FROM integration_queue WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    pub fn increment_unknown_retries(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE integration_queue SET unknown_retries = unknown_retries + 1 WHERE id = ?1",
            params![id],
        )
        .context("increment_unknown_retries")?;
        Ok(())
    }

    pub fn reset_unknown_retries(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE integration_queue SET unknown_retries = 0 WHERE id = ?1",
            params![id],
        )
        .context("reset_unknown_retries")?;
        Ok(())
    }

    // ── Repos ─────────────────────────────────────────────────────────────

    pub fn upsert_repo(
        &self,
        path: &str,
        name: &str,
        mode: &str,
        test_cmd: &str,
        prompt_file: &str,
        auto_merge: bool,
        backend: Option<&str>,
        repo_slug: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let auto_merge_int: i64 = if auto_merge { 1 } else { 0 };
        conn.execute(
            "INSERT INTO repos (path, name, mode, test_cmd, prompt_file, auto_merge, backend, repo_slug) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(path) DO UPDATE SET \
               name = excluded.name, \
               mode = COALESCE(NULLIF(excluded.mode, ''), repos.mode), \
               test_cmd = COALESCE(NULLIF(excluded.test_cmd, ''), repos.test_cmd), \
               prompt_file = COALESCE(NULLIF(excluded.prompt_file, ''), repos.prompt_file), \
               auto_merge = excluded.auto_merge, \
               backend = COALESCE(NULLIF(excluded.backend, ''), repos.backend), \
               repo_slug = COALESCE(NULLIF(excluded.repo_slug, ''), repos.repo_slug)",
            params![
                path,
                name,
                mode,
                test_cmd,
                prompt_file,
                auto_merge_int,
                backend,
                repo_slug
            ],
        )
        .context("upsert_repo")?;
        let id: i64 = conn
            .query_row(
                "SELECT id FROM repos WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .context("upsert_repo get id")?;
        Ok(id)
    }

    pub fn list_repos(&self) -> Result<Vec<RepoRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, path, name, mode, backend, test_cmd, prompt_file, auto_merge, repo_slug \
             FROM repos ORDER BY id ASC",
        )?;
        let repos = stmt
            .query_map([], row_to_repo)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_repos")?;
        Ok(repos)
    }

    pub fn get_repo_by_path(&self, path: &str) -> Result<Option<RepoRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT id, path, name, mode, backend, test_cmd, prompt_file, auto_merge, repo_slug \
                 FROM repos WHERE path = ?1",
                params![path],
                row_to_repo,
            )
            .optional()
            .context("get_repo_by_path")?;
        Ok(result)
    }

    pub fn update_repo_backend(&self, id: i64, backend: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE repos SET backend = ?1 WHERE id = ?2",
            params![
                if backend.is_empty() {
                    None
                } else {
                    Some(backend)
                },
                id
            ],
        )
        .context("update_repo_backend")?;
        Ok(())
    }

    // ── Pipeline Events ───────────────────────────────────────────────────

    pub fn log_event(
        &self,
        task_id: Option<i64>,
        repo_id: Option<i64>,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<i64> {
        self.log_event_full(task_id, repo_id, None, "", kind, payload)
    }

    pub fn log_event_full(
        &self,
        task_id: Option<i64>,
        repo_id: Option<i64>,
        project_id: Option<i64>,
        actor: &str,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let payload_str = payload.to_string();
        let created_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO pipeline_events (task_id, repo_id, project_id, actor, kind, payload, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![task_id, repo_id, project_id, actor, kind, payload_str, created_at],
        )
        .context("log_event")?;
        Ok(id)
    }

    pub fn list_project_events(&self, project_id: i64, limit: i64) -> Result<Vec<AuditEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, project_id, actor, kind, payload, created_at \
             FROM pipeline_events WHERE project_id = ?1 \
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![project_id, limit], |r| {
                let ts: String = r.get(6)?;
                Ok(AuditEvent {
                    id: r.get(0)?,
                    task_id: r.get::<_, Option<i64>>(1)?,
                    project_id: r.get::<_, Option<i64>>(2)?,
                    actor: r.get(3)?,
                    kind: r.get(4)?,
                    payload: r.get(5)?,
                    created_at: parse_ts(&ts),
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_events")?;
        Ok(rows)
    }

    pub fn list_task_events(&self, task_id: i64, limit: i64) -> Result<Vec<AuditEvent>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, task_id, project_id, actor, kind, payload, created_at \
             FROM pipeline_events WHERE task_id = ?1 \
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![task_id, limit], |r| {
                let ts: String = r.get(6)?;
                Ok(AuditEvent {
                    id: r.get(0)?,
                    task_id: r.get::<_, Option<i64>>(1)?,
                    project_id: r.get::<_, Option<i64>>(2)?,
                    actor: r.get(3)?,
                    kind: r.get(4)?,
                    payload: r.get(5)?,
                    created_at: parse_ts(&ts),
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_task_events")?;
        Ok(rows)
    }

    // ── Legacy Event Log ──────────────────────────────────────────────────

    pub fn log_legacy_event(
        &self,
        level: &str,
        category: &str,
        message: &str,
        metadata: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let ts = Utc::now().timestamp();
        let id = conn
            .execute_returning_id(
                "INSERT INTO events (ts, level, category, message, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ts, level, category, message, metadata],
            )
            .context("log_legacy_event")?;
        Ok(id)
    }

    pub fn get_recent_events(&self, limit: i64) -> Result<Vec<LegacyEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, ts, level, category, message, metadata \
             FROM events ORDER BY ts DESC, id DESC LIMIT ?1",
        )?;
        let events = stmt
            .query_map(params![limit], row_to_legacy_event)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_recent_events")?;
        Ok(events)
    }

    // ── Chat message history ──────────────────────────────────────────────

    /// Insert a chat message (incoming or outgoing) into the messages table.
    pub fn insert_chat_message(
        &self,
        id: &str,
        chat_jid: &str,
        sender: Option<&str>,
        sender_name: Option<&str>,
        content: &str,
        is_from_me: bool,
        is_bot_message: bool,
    ) -> Result<()> {
        self.insert_chat_message_with_stream(
            id,
            chat_jid,
            sender,
            sender_name,
            content,
            is_from_me,
            is_bot_message,
            None,
        )
    }

    /// Insert a chat message with optional raw NDJSON stream for agent interactions.
    pub fn insert_chat_message_with_stream(
        &self,
        id: &str,
        chat_jid: &str,
        sender: Option<&str>,
        sender_name: Option<&str>,
        content: &str,
        is_from_me: bool,
        is_bot_message: bool,
        raw_stream: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let ts = now_str();
        conn.execute(
            "INSERT INTO messages (id, chat_jid, sender, sender_name, content, timestamp, is_from_me, is_bot_message, raw_stream) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT DO NOTHING",
            params![id, chat_jid, sender, sender_name, content, ts,
                    if is_from_me { 1i64 } else { 0i64 },
                    if is_bot_message { 1i64 } else { 0i64 },
                    raw_stream],
        )
        .context("insert_chat_message")?;
        Ok(())
    }

    /// List all chat threads (distinct chat_jid values) with msg count and last timestamp.
    pub fn get_chat_threads(&self) -> Result<Vec<(String, i64, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT chat_jid, COUNT(*) as msg_count, MAX(timestamp) as last_ts \
             FROM messages GROUP BY chat_jid ORDER BY last_ts DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_chat_threads")?;
        Ok(rows)
    }

    /// Get messages for a specific chat thread, newest last.
    pub fn get_chat_messages(&self, chat_jid: &str, limit: i64) -> Result<Vec<ChatMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, chat_jid, sender, sender_name, content, timestamp, is_from_me, is_bot_message, raw_stream \
             FROM messages WHERE chat_jid = ?1 ORDER BY timestamp ASC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![chat_jid, limit], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    chat_jid: row.get(1)?,
                    sender: row.get(2)?,
                    sender_name: row.get(3)?,
                    content: row.get(4)?,
                    timestamp: row.get(5)?,
                    is_from_me: row.get::<_, i64>(6)? != 0,
                    is_bot_message: row.get::<_, i64>(7)? != 0,
                    raw_stream: row.get(8)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_chat_messages")?;
        Ok(rows)
    }

    // ── Registered groups ─────────────────────────────────────────────────

    pub fn get_all_groups(&self) -> Result<Vec<RegisteredGroup>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT jid, name, folder, trigger_pattern, requires_trigger FROM registered_groups ORDER BY added_at ASC",
        )?;
        let groups = stmt
            .query_map([], |row| {
                Ok(RegisteredGroup {
                    jid: row.get(0)?,
                    name: row.get(1)?,
                    folder: row.get(2)?,
                    trigger_pattern: row
                        .get::<_, Option<String>>(3)?
                        .unwrap_or_else(|| "@Borg".into()),
                    requires_trigger: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_all_groups")?;
        Ok(groups)
    }

    pub fn register_group(
        &self,
        jid: &str,
        name: &str,
        folder: &str,
        trigger_pattern: &str,
        requires_trigger: bool,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "INSERT INTO registered_groups (jid, name, folder, trigger_pattern, requires_trigger) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(jid) DO UPDATE SET name=excluded.name, folder=excluded.folder, \
               trigger_pattern=excluded.trigger_pattern, requires_trigger=excluded.requires_trigger",
            params![jid, name, folder, trigger_pattern, if requires_trigger { 1i64 } else { 0i64 }],
        )
        .context("register_group")?;
        Ok(())
    }

    pub fn unregister_group(&self, jid: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute("DELETE FROM registered_groups WHERE jid = ?1", params![jid])
            .context("unregister_group")?;
        Ok(())
    }

    pub fn get_seed_cooldowns(&self) -> Result<HashMap<(String, String), i64>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn
            .prepare("SELECT folder, session_id FROM sessions WHERE folder LIKE 'seed:%'")
            .context("get_seed_cooldowns")?;
        let rows = stmt
            .query_map([], |r| {
                let folder: String = r.get(0)?;
                let ts: String = r.get(1)?;
                Ok((folder, ts))
            })
            .context("get_seed_cooldowns")?;
        let mut map = HashMap::new();
        for (folder, ts) in rows.flatten() {
            let parts: Vec<&str> = folder.splitn(3, ':').collect();
            if parts.len() == 3 {
                if let Ok(t) = ts.parse::<i64>() {
                    map.insert((parts[1].to_string(), parts[2].to_string()), t);
                }
            }
        }
        Ok(map)
    }

    pub fn set_seed_cooldown(&self, repo_path: &str, seed_name: &str, ts: i64) -> Result<()> {
        let folder = format!("seed:{repo_path}:{seed_name}");
        self.set_session(&folder, &ts.to_string())
    }

    pub fn expire_sessions(&self, max_age_hours: i64) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute(
                "DELETE FROM sessions \
                 WHERE NULLIF(created_at, '') IS NOT NULL \
                   AND created_at::timestamp < (timezone('UTC', now()) - make_interval(hours => ?1::int))",
                params![max_age_hours],
            )
            .context("expire_sessions")?;
        Ok(n)
    }

    // ── Chat agent runs ───────────────────────────────────────────────────

    pub fn create_chat_agent_run(
        &self,
        jid: &str,
        transport: &str,
        original_id: &str,
        trigger_msg_id: &str,
        folder: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id = conn.execute_returning_id(
            "INSERT INTO chat_agent_runs (jid, status, transport, original_id, trigger_msg_id, folder) \
             VALUES (?1, 'running', ?2, ?3, ?4, ?5)",
            params![jid, transport, original_id, trigger_msg_id, folder],
        )
        .context("create_chat_agent_run")?;
        Ok(id)
    }

    pub fn complete_chat_agent_run(
        &self,
        id: i64,
        output: &str,
        new_session_id: &str,
        last_msg_timestamp: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE chat_agent_runs SET status='completed', output=?1, new_session_id=?2, \
             last_msg_timestamp=?3, completed_at=?4 WHERE id=?5",
            params![output, new_session_id, last_msg_timestamp, now_str(), id],
        )
        .context("complete_chat_agent_run")?;
        Ok(())
    }

    pub fn mark_chat_agent_run_delivered(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE chat_agent_runs SET status='delivered' WHERE id=?1",
            params![id],
        )
        .context("mark_chat_agent_run_delivered")?;
        Ok(())
    }

    pub fn get_undelivered_runs(&self, jid: &str) -> Result<Vec<ChatAgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, jid, status, transport, original_id, trigger_msg_id, folder, \
             output, new_session_id, last_msg_timestamp, started_at, completed_at \
             FROM chat_agent_runs WHERE jid=?1 AND status='completed' ORDER BY id ASC",
        )?;
        let runs = stmt
            .query_map(params![jid], row_to_chat_agent_run)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_undelivered_runs")?;
        Ok(runs)
    }

    pub fn fail_chat_agent_run(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE chat_agent_runs SET status='failed', completed_at=?1 WHERE id=?2",
            params![now_str(), id],
        )
        .context("fail_chat_agent_run")?;
        Ok(())
    }

    pub fn has_running_chat_agent(&self, jid: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_agent_runs WHERE jid=?1 AND status='running'",
                params![jid],
                |row| row.get(0),
            )
            .context("has_running_chat_agent")?;
        Ok(count > 0)
    }

    pub fn abandon_running_agents(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute(
                "UPDATE chat_agent_runs SET status='abandoned' WHERE status='running'",
                [],
            )
            .context("abandon_running_agents")?;
        Ok(n)
    }

    pub fn get_messages_since(
        &self,
        chat_jid: &str,
        since_ts: &str,
        limit: i64,
    ) -> Result<Vec<ChatMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, chat_jid, sender, sender_name, content, timestamp, is_from_me, is_bot_message, raw_stream \
             FROM messages WHERE chat_jid=?1 AND timestamp > ?2 ORDER BY timestamp ASC LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![chat_jid, since_ts, limit], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    chat_jid: row.get(1)?,
                    sender: row.get(2)?,
                    sender_name: row.get(3)?,
                    content: row.get(4)?,
                    timestamp: row.get(5)?,
                    is_from_me: row.get::<_, i64>(6)? != 0,
                    is_bot_message: row.get::<_, i64>(7)? != 0,
                    raw_stream: row.get(8)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_messages_since")?;
        Ok(rows)
    }

    // ── Events query ──────────────────────────────────────────────────────

    /// Query the legacy events table with optional filters.
    pub fn get_events_filtered(
        &self,
        category: Option<&str>,
        level: Option<&str>,
        since_ts: Option<i64>,
        limit: i64,
    ) -> Result<Vec<LegacyEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = Vec::new();
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = Vec::new();
        if let Some(category) = category.map(str::trim).filter(|c| !c.is_empty()) {
            where_clauses.push("category = ?".to_string());
            params_vec.push(Box::new(category.to_string()));
        }
        if let Some(level) = level.map(str::trim).filter(|l| !l.is_empty()) {
            where_clauses.push("level = ?".to_string());
            params_vec.push(Box::new(level.to_string()));
        }
        if let Some(since_ts) = since_ts {
            where_clauses.push("ts >= ?".to_string());
            params_vec.push(Box::new(since_ts));
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };
        params_vec.push(Box::new(limit));
        let param_refs: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let sql = format!(
            "SELECT id, ts, level, category, message, metadata FROM events \
             {where_sql} \
             ORDER BY ts DESC, id DESC LIMIT ?"
        );
        let mut stmt = conn.prepare(&sql)?;
        let events = stmt
            .query_map(param_refs.as_slice(), row_to_legacy_event)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_events_filtered")?;
        Ok(events)
    }

    // ── API Keys (BYOK) ──────────────────────────────────────────────────

    fn block_on_async_option<F, T>(fut: F) -> Option<T>
    where
        F: std::future::Future<Output = Option<T>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(fut))
        } else {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(fut)
        }
    }

    fn decode_master_key_hex(key_hex: &str) -> Option<[u8; 32]> {
        if key_hex.len() != 64 {
            return None;
        }
        let key_bytes = hex::decode(key_hex).ok()?;
        if key_bytes.len() != 32 {
            return None;
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&key_bytes);
        Some(out)
    }

    fn load_master_key_from_kms() -> Option<[u8; 32]> {
        use aws_config::{BehaviorVersion, Region};
        use aws_sdk_kms::primitives::Blob;

        let ciphertext_b64 = std::env::var("BORG_MASTER_KEY_KMS_CIPHERTEXT_B64").ok()?;
        let ciphertext = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(ciphertext_b64)
                .ok()?
        };
        if ciphertext.is_empty() {
            return None;
        }

        Self::block_on_async_option(async move {
            let region = std::env::var("BORG_MASTER_KEY_KMS_REGION")
                .ok()
                .filter(|r| !r.trim().is_empty())
                .or_else(|| std::env::var("AWS_REGION").ok());

            let mut loader = aws_config::defaults(BehaviorVersion::latest());
            if let Some(region) = region {
                loader = loader.region(Region::new(region));
            }
            let shared = loader.load().await;
            let client = aws_sdk_kms::Client::new(&shared);

            let mut req = client.decrypt().ciphertext_blob(Blob::new(ciphertext));
            if let Ok(key_id) = std::env::var("BORG_MASTER_KEY_KMS_KEY_ID") {
                if !key_id.trim().is_empty() {
                    req = req.key_id(key_id);
                }
            }
            let out = req.send().await.ok()?;
            let plaintext = out.plaintext()?.as_ref();
            if plaintext.len() != 32 {
                return None;
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(plaintext);
            Some(key)
        })
    }

    fn master_key_bytes() -> Option<[u8; 32]> {
        static MASTER_KEY_CACHE: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();
        *MASTER_KEY_CACHE.get_or_init(|| {
            if let Ok(key_hex) = std::env::var("BORG_MASTER_KEY") {
                if let Some(key) = Self::decode_master_key_hex(&key_hex) {
                    return Some(key);
                }
                tracing::warn!("BORG_MASTER_KEY is set but invalid (expected 64-char hex)");
            }
            let kms_key = Self::load_master_key_from_kms();
            if kms_key.is_none() && std::env::var("BORG_MASTER_KEY_KMS_CIPHERTEXT_B64").is_ok() {
                tracing::warn!("failed to resolve master key from AWS KMS ciphertext");
            }
            kms_key
        })
    }

    fn encrypt_secret(secret: &str) -> String {
        if let Some(key_bytes) = Self::master_key_bytes() {
            use aes_gcm::{
                aead::{Aead, AeadCore, KeyInit, OsRng},
                Aes256Gcm,
            };
            let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
            let cipher = Aes256Gcm::new(key);
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits
            if let Ok(ciphertext) = cipher.encrypt(&nonce, secret.as_bytes()) {
                let mut combined = nonce.to_vec();
                combined.extend_from_slice(&ciphertext);
                use base64::Engine;
                return format!(
                    "enc:v1:{}",
                    base64::engine::general_purpose::STANDARD.encode(&combined)
                );
            }
        }
        secret.to_string()
    }

    fn decrypt_secret(secret: &str) -> String {
        if let Some(encoded) = secret.strip_prefix("enc:v1:") {
            if let Some(key_bytes) = Self::master_key_bytes() {
                use base64::Engine;
                if let Ok(combined) = base64::engine::general_purpose::STANDARD.decode(encoded)
                {
                    if combined.len() > 12 {
                        use aes_gcm::{
                            aead::{Aead, KeyInit},
                            Aes256Gcm, Nonce,
                        };
                        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
                        let cipher = Aes256Gcm::new(key);
                        let nonce = Nonce::from_slice(&combined[..12]);
                        if let Ok(plaintext) = cipher.decrypt(nonce, &combined[12..]) {
                            if let Ok(s) = String::from_utf8(plaintext) {
                                return s;
                            }
                        }
                    }
                }
            }
        }
        secret.to_string()
    }

    pub fn store_api_key(
        &self,
        owner: &str,
        provider: &str,
        key_name: &str,
        key_value: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let encrypted_value = Self::encrypt_secret(key_value);
        let id = conn.execute_returning_id(
            "INSERT INTO api_keys (owner, provider, key_name, key_value) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(owner, provider) DO UPDATE SET key_name=excluded.key_name, key_value=excluded.key_value",
            params![owner, provider, key_name, encrypted_value],
        )?;
        Ok(id)
    }

    pub fn store_workspace_api_key(
        &self,
        workspace_id: i64,
        provider: &str,
        key_name: &str,
        key_value: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let encrypted_value = Self::encrypt_secret(key_value);
        let owner = format!("workspace:{workspace_id}");
        let id = conn.execute_returning_id(
            "INSERT INTO api_keys (workspace_id, owner, provider, key_name, key_value) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(owner, provider) DO UPDATE SET workspace_id=excluded.workspace_id, key_name=excluded.key_name, key_value=excluded.key_value",
            params![workspace_id, owner, provider, key_name, encrypted_value],
        )?;
        Ok(id)
    }

    pub fn get_api_key(&self, owner: &str, provider: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        // Try owner-specific first, then fall back to global
        let result = conn
            .query_row(
                "SELECT key_value FROM api_keys WHERE owner = ?1 AND provider = ?2",
                params![owner, provider],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("get_api_key")?;
        if let Some(val) = result {
            return Ok(Some(Self::decrypt_secret(&val)));
        }
        if owner != "global" {
            let global = conn
                .query_row(
                    "SELECT key_value FROM api_keys WHERE owner = 'global' AND provider = ?1",
                    params![provider],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .context("get_api_key global fallback")?;
            if let Some(val) = global {
                return Ok(Some(Self::decrypt_secret(&val)));
            }
        }
        Ok(None)
    }

    pub fn get_api_key_exact(&self, owner: &str, provider: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT key_value FROM api_keys WHERE owner = ?1 AND provider = ?2",
                params![owner, provider],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("get_api_key_exact")?;
        Ok(result.map(|val| Self::decrypt_secret(&val)))
    }

    pub fn list_api_keys(&self, owner: &str) -> Result<Vec<ApiKeyEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, owner, provider, key_name, created_at FROM api_keys \
             WHERE owner = ?1 OR owner = 'global' ORDER BY provider",
        )?;
        let keys = stmt
            .query_map(params![owner], |row| {
                Ok(ApiKeyEntry {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    owner: row.get(2)?,
                    provider: row.get(3)?,
                    key_name: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_api_keys")?;
        Ok(keys)
    }

    pub fn list_workspace_api_keys(&self, workspace_id: i64) -> Result<Vec<ApiKeyEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let owner = format!("workspace:{workspace_id}");
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, owner, provider, key_name, created_at FROM api_keys \
             WHERE owner = ?1 ORDER BY provider",
        )?;
        let keys = stmt
            .query_map(params![owner], |row| {
                Ok(ApiKeyEntry {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    owner: row.get(2)?,
                    provider: row.get(3)?,
                    key_name: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_workspace_api_keys")?;
        Ok(keys)
    }

    pub fn delete_api_key(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_workspace_api_key(&self, workspace_id: i64, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let owner = format!("workspace:{workspace_id}");
        conn.execute(
            "DELETE FROM api_keys WHERE id = ?1 AND owner = ?2",
            params![id, owner],
        )?;
        Ok(())
    }

    // ── Custom MCP Servers ─────────────────────────────────────────────

    pub fn upsert_custom_mcp_server(
        &self,
        workspace_id: i64,
        name: &str,
        label: &str,
        command: &str,
        args_json: &str,
        env_json: &str,
        enabled: bool,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let encrypted_env = Self::encrypt_secret(env_json);
        let enabled_i: i64 = if enabled { 1 } else { 0 };
        let id = conn.execute_returning_id(
            "INSERT INTO custom_mcp_servers (workspace_id, name, label, command, args, env, enabled) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(workspace_id, name) DO UPDATE SET \
               label=excluded.label, command=excluded.command, args=excluded.args, \
               env=excluded.env, enabled=excluded.enabled",
            params![workspace_id, name, label, command, args_json, encrypted_env, enabled_i],
        )?;
        Ok(id)
    }

    pub fn list_custom_mcp_servers(&self, workspace_id: i64) -> Result<Vec<CustomMcpServerRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, name, label, command, args, env, enabled, created_at \
             FROM custom_mcp_servers WHERE workspace_id = ?1 ORDER BY name",
        )?;
        let rows = stmt
            .query_map(params![workspace_id], |row| {
                let env_encrypted: String = row.get(6)?;
                let env_json = Db::decrypt_secret(&env_encrypted);
                let env_keys: Vec<String> = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&env_json)
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default();
                Ok(CustomMcpServerRow {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    name: row.get(2)?,
                    label: row.get(3)?,
                    command: row.get(4)?,
                    args_json: row.get(5)?,
                    env_keys,
                    enabled: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_custom_mcp_servers")?;
        Ok(rows)
    }

    pub fn get_enabled_custom_mcp_servers_resolved(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<(String, String, Vec<String>, std::collections::HashMap<String, String>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT name, command, args, env FROM custom_mcp_servers \
             WHERE workspace_id = ?1 AND enabled = 1 ORDER BY name",
        )?;
        let rows = stmt
            .query_map(params![workspace_id], |row| {
                let name: String = row.get(0)?;
                let command: String = row.get(1)?;
                let args_json: String = row.get(2)?;
                let env_encrypted: String = row.get(3)?;
                let env_json = Db::decrypt_secret(&env_encrypted);
                let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                let env: std::collections::HashMap<String, String> =
                    serde_json::from_str(&env_json).unwrap_or_default();
                Ok((name, command, args, env))
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_enabled_custom_mcp_servers_resolved")?;
        Ok(rows)
    }

    pub fn delete_custom_mcp_server(&self, workspace_id: i64, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM custom_mcp_servers WHERE id = ?1 AND workspace_id = ?2",
            params![id, workspace_id],
        )?;
        Ok(())
    }

    pub fn toggle_custom_mcp_server(&self, workspace_id: i64, id: i64, enabled: bool) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let enabled_i: i64 = if enabled { 1 } else { 0 };
        conn.execute(
            "UPDATE custom_mcp_servers SET enabled = ?1 WHERE id = ?2 AND workspace_id = ?3",
            params![enabled_i, id, workspace_id],
        )?;
        Ok(())
    }

    pub fn list_user_linked_credentials(&self, user_id: i64) -> Result<Vec<LinkedCredentialEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, provider, auth_kind, account_email, account_label, status, \
                    expires_at, last_validated_at, last_used_at, last_error, created_at, updated_at \
             FROM linked_credentials WHERE user_id = ?1 ORDER BY provider",
        )?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                Ok(LinkedCredentialEntry {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    provider: row.get(2)?,
                    auth_kind: row.get(3)?,
                    account_email: row.get(4)?,
                    account_label: row.get(5)?,
                    status: row.get(6)?,
                    expires_at: row.get(7)?,
                    last_validated_at: row.get(8)?,
                    last_used_at: row.get(9)?,
                    last_error: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_user_linked_credentials")?;
        Ok(rows)
    }

    pub fn list_all_linked_credentials(&self) -> Result<Vec<LinkedCredentialEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, provider, auth_kind, account_email, account_label, status, \
                    expires_at, last_validated_at, last_used_at, last_error, created_at, updated_at \
             FROM linked_credentials ORDER BY user_id, provider",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LinkedCredentialEntry {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    provider: row.get(2)?,
                    auth_kind: row.get(3)?,
                    account_email: row.get(4)?,
                    account_label: row.get(5)?,
                    status: row.get(6)?,
                    expires_at: row.get(7)?,
                    last_validated_at: row.get(8)?,
                    last_used_at: row.get(9)?,
                    last_error: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_all_linked_credentials")?;
        Ok(rows)
    }

    pub fn get_user_linked_credential(
        &self,
        user_id: i64,
        provider: &str,
    ) -> Result<Option<LinkedCredentialSecret>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let row = conn
            .query_row(
                "SELECT id, user_id, provider, auth_kind, account_email, account_label, status, \
                        expires_at, last_validated_at, last_used_at, last_error, created_at, updated_at, credential_bundle \
                 FROM linked_credentials WHERE user_id = ?1 AND provider = ?2",
                params![user_id, provider],
                |row| {
                    Ok((
                        LinkedCredentialEntry {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            provider: row.get(2)?,
                            auth_kind: row.get(3)?,
                            account_email: row.get(4)?,
                            account_label: row.get(5)?,
                            status: row.get(6)?,
                            expires_at: row.get(7)?,
                            last_validated_at: row.get(8)?,
                            last_used_at: row.get(9)?,
                            last_error: row.get(10)?,
                            created_at: row.get(11)?,
                            updated_at: row.get(12)?,
                        },
                        row.get::<_, String>(13)?,
                    ))
                },
            )
            .optional()
            .context("get_user_linked_credential")?;
        let Some((entry, encrypted_bundle)) = row else {
            return Ok(None);
        };
        let bundle_json = Self::decrypt_secret(&encrypted_bundle);
        let bundle = serde_json::from_str::<LinkedCredentialBundle>(&bundle_json)
            .context("decode linked credential bundle")?;
        Ok(Some(LinkedCredentialSecret { entry, bundle }))
    }

    pub fn upsert_user_linked_credential(
        &self,
        user_id: i64,
        provider: &str,
        auth_kind: &str,
        account_email: &str,
        account_label: &str,
        status: &str,
        expires_at: &str,
        last_validated_at: &str,
        last_used_at: &str,
        last_error: &str,
        bundle: &LinkedCredentialBundle,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let bundle_json =
            serde_json::to_string(bundle).context("encode linked credential bundle")?;
        let encrypted_bundle = Self::encrypt_secret(&bundle_json);
        let id = conn.execute_returning_id(
            "INSERT INTO linked_credentials \
                (user_id, provider, auth_kind, account_email, account_label, credential_bundle, \
                 status, expires_at, last_validated_at, last_used_at, last_error, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, to_char(timezone('UTC', now()), 'YYYY-MM-DD HH24:MI:SS')) \
             ON CONFLICT(user_id, provider) DO UPDATE SET \
                 auth_kind = excluded.auth_kind, \
                 account_email = excluded.account_email, \
                 account_label = excluded.account_label, \
                 credential_bundle = excluded.credential_bundle, \
                 status = excluded.status, \
                 expires_at = excluded.expires_at, \
                 last_validated_at = excluded.last_validated_at, \
                 last_used_at = excluded.last_used_at, \
                 last_error = excluded.last_error, \
                 updated_at = to_char(timezone('UTC', now()), 'YYYY-MM-DD HH24:MI:SS')",
            params![
                user_id,
                provider,
                auth_kind,
                account_email,
                account_label,
                encrypted_bundle,
                status,
                expires_at,
                last_validated_at,
                last_used_at,
                last_error
            ],
        )?;
        Ok(id)
    }

    pub fn update_user_linked_credential_state(
        &self,
        user_id: i64,
        provider: &str,
        auth_kind: &str,
        account_email: &str,
        account_label: &str,
        status: &str,
        expires_at: &str,
        last_validated_at: &str,
        last_error: &str,
        bundle: Option<&LinkedCredentialBundle>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let encrypted_bundle = match bundle {
            Some(bundle) => {
                let bundle_json =
                    serde_json::to_string(bundle).context("encode linked credential bundle")?;
                Some(Self::encrypt_secret(&bundle_json))
            },
            None => None,
        };
        conn.execute(
            "UPDATE linked_credentials SET \
                 auth_kind = ?3, \
                 account_email = ?4, \
                 account_label = ?5, \
                 status = ?6, \
                 expires_at = ?7, \
                 last_validated_at = ?8, \
                 last_error = ?9, \
                 credential_bundle = COALESCE(?10, credential_bundle), \
                 updated_at = to_char(timezone('UTC', now()), 'YYYY-MM-DD HH24:MI:SS') \
             WHERE user_id = ?1 AND provider = ?2",
            params![
                user_id,
                provider,
                auth_kind,
                account_email,
                account_label,
                status,
                expires_at,
                last_validated_at,
                last_error,
                encrypted_bundle
            ],
        )?;
        Ok(())
    }

    pub fn touch_user_linked_credential_used(&self, user_id: i64, provider: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE linked_credentials SET last_used_at = ?3, updated_at = to_char(timezone('UTC', now()), 'YYYY-MM-DD HH24:MI:SS') \
             WHERE user_id = ?1 AND provider = ?2",
            params![user_id, provider, now],
        )?;
        Ok(())
    }

    pub fn delete_user_linked_credential(&self, user_id: i64, provider: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM linked_credentials WHERE user_id = ?1 AND provider = ?2",
            params![user_id, provider],
        )?;
        Ok(())
    }

    // ── Cost Tracking ────────────────────────────────────────────────────

    pub fn update_message_usage(
        &self,
        message_id: &str,
        chat_jid: &str,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
        model: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE messages SET input_tokens = ?1, output_tokens = ?2, \
             cost_usd = ?3, model = ?4 WHERE chat_jid = ?5 AND id = ?6",
            params![
                input_tokens,
                output_tokens,
                cost_usd,
                model,
                chat_jid,
                message_id
            ],
        )
        .context("update_message_usage")?;
        Ok(())
    }

    pub fn accumulate_task_usage(
        &self,
        task_id: i64,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET \
             total_input_tokens = COALESCE(total_input_tokens, 0) + ?1, \
             total_output_tokens = COALESCE(total_output_tokens, 0) + ?2, \
             total_cost_usd = COALESCE(total_cost_usd, 0) + ?3, \
             updated_at = ?4 WHERE id = ?5",
            params![input_tokens, output_tokens, cost_usd, now_str(), task_id],
        )
        .context("accumulate_task_usage")?;
        Ok(())
    }

    pub fn get_usage_summary(
        &self,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<UsageSummary> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;

        let mut where_clauses = Vec::new();
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = Vec::new();

        if let Some(from) = from {
            where_clauses.push("timestamp >= ?".to_string());
            params_vec.push(Box::new(from.format("%Y-%m-%d %H:%M:%S").to_string()));
        }
        if let Some(to) = to {
            where_clauses.push("timestamp <= ?".to_string());
            params_vec.push(Box::new(to.format("%Y-%m-%d %H:%M:%S").to_string()));
        }
        where_clauses.push("input_tokens IS NOT NULL".to_string());

        let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));
        let param_refs: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let msg_sql = format!(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), \
             COALESCE(SUM(cost_usd), 0), COUNT(*) FROM messages{where_sql}"
        );
        let (msg_input, msg_output, msg_cost, msg_count): (i64, i64, f64, i64) = conn
            .query_row(&msg_sql, param_refs.as_slice(), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .context("get_usage_summary messages")?;

        let mut task_where = Vec::new();
        let mut task_params: Vec<Box<dyn pg::types::ToSql>> = Vec::new();
        if let Some(from) = from {
            task_where.push("created_at >= ?".to_string());
            task_params.push(Box::new(from.format("%Y-%m-%d %H:%M:%S").to_string()));
        }
        if let Some(to) = to {
            task_where.push("created_at <= ?".to_string());
            task_params.push(Box::new(to.format("%Y-%m-%d %H:%M:%S").to_string()));
        }
        task_where.push("total_input_tokens > 0".to_string());

        let task_where_sql = format!(" WHERE {}", task_where.join(" AND "));
        let task_param_refs: Vec<&dyn pg::types::ToSql> =
            task_params.iter().map(|p| p.as_ref()).collect();

        let task_sql = format!(
            "SELECT COALESCE(SUM(total_input_tokens), 0), COALESCE(SUM(total_output_tokens), 0), \
             COALESCE(SUM(total_cost_usd), 0), COUNT(*) FROM pipeline_tasks{task_where_sql}"
        );
        let (task_input, task_output, task_cost, task_count): (i64, i64, f64, i64) = conn
            .query_row(&task_sql, task_param_refs.as_slice(), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .context("get_usage_summary tasks")?;

        Ok(UsageSummary {
            total_input_tokens: msg_input + task_input,
            total_output_tokens: msg_output + task_output,
            total_cost_usd: msg_cost + task_cost,
            message_count: msg_count,
            task_count,
        })
    }

    // ── Tool Call Tracking ────────────────────────────────────────────────

    pub fn insert_tool_call(
        &self,
        run_id: &str,
        tool_name: &str,
        task_id: Option<i64>,
        chat_key: Option<&str>,
        input_summary: Option<&str>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id = conn
            .execute_returning_id(
                "INSERT INTO tool_calls (run_id, tool_name, task_id, chat_key, input_summary) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
                params![run_id, tool_name, task_id, chat_key, input_summary],
            )
            .context("insert_tool_call")?;
        Ok(id)
    }

    pub fn complete_tool_call(
        &self,
        id: i64,
        output_summary: Option<&str>,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE tool_calls SET output_summary = ?1, duration_ms = ?2, \
             success = ?3, error = ?4 WHERE id = ?5",
            params![output_summary, duration_ms, success, error, id],
        )
        .context("complete_tool_call")?;
        Ok(())
    }

    pub fn list_tool_calls_by_task(
        &self,
        task_id: i64,
        limit: i64,
    ) -> Result<Vec<crate::tool_calls::ToolCallEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, chat_key, run_id, tool_name, input_summary, \
             output_summary, started_at, duration_ms, success, error \
             FROM tool_calls WHERE task_id = ?1 \
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![task_id, limit], row_to_tool_call)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_tool_calls_by_task")?;
        Ok(rows)
    }

    pub fn list_tool_calls_by_chat(
        &self,
        chat_key: &str,
        limit: i64,
    ) -> Result<Vec<crate::tool_calls::ToolCallEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, chat_key, run_id, tool_name, input_summary, \
             output_summary, started_at, duration_ms, success, error \
             FROM tool_calls WHERE chat_key = ?1 \
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![chat_key, limit], row_to_tool_call)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_tool_calls_by_chat")?;
        Ok(rows)
    }

    pub fn list_tool_calls_by_run(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<crate::tool_calls::ToolCallEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, chat_key, run_id, tool_name, input_summary, \
             output_summary, started_at, duration_ms, success, error \
             FROM tool_calls WHERE run_id = ?1 \
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![run_id, limit], row_to_tool_call)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_tool_calls_by_run")?;
        Ok(rows)
    }
}

