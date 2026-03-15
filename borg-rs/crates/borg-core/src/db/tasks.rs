use super::*;

impl Db {
    // ── Pipeline Tasks ────────────────────────────────────────────────────

    pub fn get_task(&self, id: i64) -> Result<Option<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                &format!("SELECT {TASK_COLS} FROM pipeline_tasks WHERE id = ?1"),
                params![id],
                row_to_task,
            )
            .optional()
            .context("get_task")?;
        Ok(result)
    }

    pub fn list_active_tasks(&self) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!(
            "SELECT {TASK_COLS} FROM pipeline_tasks \
             WHERE status NOT IN ('done', 'merged', 'failed', 'blocked', 'pending_review', 'human_review', 'purged') \
             ORDER BY CASE status \
               WHEN 'rebase' THEN 0 \
               WHEN 'validate' THEN 1 \
               WHEN 'implement' THEN 1 \
               WHEN 'impl' THEN 1 \
               WHEN 'retry' THEN 1 \
               WHEN 'qa' THEN 2 \
               WHEN 'spec' THEN 3 \
               ELSE 4 \
             END, id ASC",
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map([], row_to_task)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_active_tasks")?;
        Ok(tasks)
    }

    pub fn insert_task(&self, task: &Task) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = task.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let project_id = if task.project_id == 0 {
            None
        } else {
            Some(task.project_id)
        };
        let workspace_id = if task.workspace_id > 0 {
            Some(task.workspace_id)
        } else if let Some(project_id) = project_id {
            conn.query_row(
                "SELECT workspace_id FROM projects WHERE id = ?1",
                params![project_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
        } else {
            conn.query_row(
                "SELECT id FROM workspaces WHERE kind = 'system' ORDER BY id ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?
        };
        let id = conn.execute_returning_id(
            "INSERT INTO pipeline_tasks \
             (title, description, repo_path, branch, status, attempt, max_attempts, \
              last_error, created_by, notify_chat, created_at, session_id, mode, backend, workspace_id, project_id, task_type, \
              requires_exhaustive_corpus_review, chat_thread) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                task.title,
                task.description,
                task.repo_path,
                task.branch,
                task.status,
                task.attempt,
                task.max_attempts,
                task.last_error,
                task.created_by,
                task.notify_chat,
                created_at,
                task.session_id,
                task.mode,
                if task.backend.is_empty() {
                    None
                } else {
                    Some(task.backend.as_str())
                },
                workspace_id,
                project_id,
                &task.task_type,
                if task.requires_exhaustive_corpus_review {
                    1i64
                } else {
                    0i64
                },
                &task.chat_thread,
            ],
        )
        .context("insert_task")?;
        Ok(id)
    }

    pub fn update_task_status(&self, id: i64, status: &str, error: Option<&str>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let updated_at = now_str();
        conn.execute(
            "UPDATE pipeline_tasks SET status = ?1, last_error = COALESCE(?2, last_error), \
             updated_at = ?3 WHERE id = ?4",
            params![status, error, updated_at, id],
        )
        .context("update_task_status")?;
        Ok(())
    }

    pub fn mark_task_started(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let now = now_str();
        conn.execute(
            "UPDATE pipeline_tasks SET started_at = COALESCE(started_at, ?1) WHERE id = ?2",
            params![now, id],
        )
        .context("mark_task_started")?;
        Ok(())
    }

    pub fn mark_task_completed(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let now = now_str();
        conn.execute(
            "UPDATE pipeline_tasks SET completed_at = ?1, \
             duration_secs = CASE WHEN started_at IS NOT NULL AND started_at != '' \
               THEN GREATEST(0, CAST(EXTRACT(EPOCH FROM ((?2)::timestamp - started_at::timestamp)) AS BIGINT)) \
               ELSE NULL END \
             WHERE id = ?3",
            params![now.clone(), now, id],
        )
        .context("mark_task_completed")?;
        Ok(())
    }

    pub fn set_review_status(&self, id: i64, status: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET review_status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now_str(), id],
        )
        .context("set_review_status")?;
        Ok(())
    }

    pub fn request_task_revision(&self, id: i64, target_phase: &str, feedback: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let tx = conn
            .transaction()
            .context("request_task_revision transaction")?;
        let updated_at = now_str();
        tx.execute(
            "INSERT INTO task_messages (task_id, role, content, created_at) \
             VALUES (?1, 'user', ?2, ?3)",
            params![id, feedback, updated_at],
        )
        .context("request_task_revision insert_task_message")?;
        tx.execute(
            "UPDATE pipeline_tasks SET status = ?1, review_status = 'revision_requested', \
             revision_count = revision_count + 1, attempt = 0, session_id = '', \
             last_error = '', updated_at = ?2 WHERE id = ?3",
            params![target_phase, updated_at, id],
        )
        .context("request_task_revision update_task")?;
        tx.commit().context("request_task_revision commit")?;
        Ok(())
    }

    pub fn increment_revision_count(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET revision_count = revision_count + 1, updated_at = ?1 WHERE id = ?2",
            params![now_str(), id],
        )
        .context("increment_revision_count")?;
        Ok(())
    }

    pub fn get_task_revision_count(&self, id: i64) -> i64 {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row(
            "SELECT revision_count FROM pipeline_tasks WHERE id = ?1",
            params![id],
            |r: &pg::Row| r.get(0),
        )
        .unwrap_or(0)
    }

    pub fn update_task_branch(&self, id: i64, branch: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET branch = ?1 WHERE id = ?2",
            params![branch, id],
        )
        .context("update_task_branch")?;
        Ok(())
    }

    pub fn update_task_repo_path(&self, id: i64, repo_path: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET repo_path = ?1 WHERE id = ?2",
            params![repo_path, id],
        )
        .context("update_task_repo_path")?;
        Ok(())
    }

    pub fn update_task_session(&self, id: i64, session_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET session_id = ?1 WHERE id = ?2",
            params![session_id, id],
        )
        .context("update_task_session")?;
        Ok(())
    }

    pub fn update_task_description(&self, id: i64, title: &str, description: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET title = ?1, description = ?2 WHERE id = ?3",
            params![title, description, id],
        )
        .context("update_task_description")?;
        Ok(())
    }

    pub fn requeue_task(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let updated_at = now_str();
        conn.execute(
            "UPDATE pipeline_tasks SET status = 'backlog', attempt = 0, \
             session_id = '', last_error = '', updated_at = ?1 WHERE id = ?2",
            params![updated_at, id],
        )
        .context("requeue_task")?;
        Ok(())
    }

    pub fn increment_attempt(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET attempt = attempt + 1 WHERE id = ?1",
            params![id],
        )
        .context("increment_attempt")?;
        Ok(())
    }

    pub fn update_task_backend(&self, id: i64, backend: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET backend = ?1 WHERE id = ?2",
            params![
                if backend.is_empty() {
                    None
                } else {
                    Some(backend)
                },
                id
            ],
        )
        .context("update_task_backend")?;
        Ok(())
    }

    pub fn update_task_structured_data(&self, id: i64, data: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET structured_data = ?1 WHERE id = ?2",
            params![data, id],
        )
        .context("update_task_structured_data")?;
        Ok(())
    }

    pub fn get_task_structured_data(&self, id: i64) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let data: String = conn
            .query_row(
                "SELECT structured_data FROM pipeline_tasks WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        Ok(data)
    }

    // ── Task Outputs ──────────────────────────────────────────────────────

    pub fn insert_task_output(
        &self,
        task_id: i64,
        phase: &str,
        output: &str,
        raw_stream: &str,
        exit_code: i64,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO task_outputs (task_id, phase, output, raw_stream, exit_code, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![task_id, phase, output, raw_stream, exit_code, created_at],
        )
        .context("insert_task_output")?;
        Ok(id)
    }

    pub fn purge_task_data(&self, task_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;

        // Delete vector embeddings
        conn.execute(
            "DELETE FROM embeddings WHERE task_id = ?1",
            params![task_id],
        )
        .context("delete embeddings")?;

        // Delete chat history (keep outputs for UI visibility)
        conn.execute(
            "DELETE FROM task_messages WHERE task_id = ?1",
            params![task_id],
        )
        .context("delete messages")?;

        Ok(())
    }

    pub fn get_task_outputs(&self, task_id: i64) -> Result<Vec<TaskOutput>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, phase, output, raw_stream, exit_code, created_at \
             FROM task_outputs WHERE task_id = ?1 ORDER BY id ASC",
        )?;
        let outputs = stmt
            .query_map(params![task_id], row_to_task_output)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_task_outputs")?;
        Ok(outputs)
    }

    // ── Task Messages ─────────────────────────────────────────────────────

    pub fn insert_task_message(&self, task_id: i64, role: &str, content: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn
            .execute_returning_id(
                "INSERT INTO task_messages (task_id, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
                params![task_id, role, content, created_at],
            )
            .context("insert_task_message")?;
        Ok(id)
    }

    pub fn get_task_messages(&self, task_id: i64) -> Result<Vec<TaskMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, role, content, created_at, delivered_phase \
             FROM task_messages WHERE task_id = ?1 ORDER BY id ASC",
        )?;
        let messages = stmt
            .query_map(params![task_id], row_to_task_message)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_task_messages")?;
        Ok(messages)
    }

    pub fn get_pending_task_messages(&self, task_id: i64) -> Result<Vec<TaskMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, role, content, created_at, delivered_phase \
             FROM task_messages WHERE task_id = ?1 AND delivered_phase IS NULL ORDER BY id ASC",
        )?;
        let messages = stmt
            .query_map(params![task_id], row_to_task_message)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_pending_task_messages")?;
        Ok(messages)
    }

    pub fn mark_messages_delivered(&self, task_id: i64, phase: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE task_messages SET delivered_phase = ?1 \
             WHERE task_id = ?2 AND delivered_phase IS NULL",
            params![phase, task_id],
        )
        .context("mark_messages_delivered")?;
        Ok(())
    }

    pub fn create_pipeline_task(
        &self,
        title: &str,
        description: &str,
        repo_path: &str,
        source: &str,
        notify_chat: &str,
        mode: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let system_workspace_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM workspaces WHERE kind = 'system' ORDER BY id ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let id = conn
            .execute_returning_id(
                "INSERT INTO pipeline_tasks \
             (title, description, repo_path, status, attempt, max_attempts, last_error, \
              created_by, notify_chat, created_at, session_id, mode, backend, workspace_id) \
             VALUES (?1, ?2, ?3, 'backlog', 0, 5, '', ?4, ?5, ?6, '', ?7, '', ?8)",
                params![
                    title,
                    description,
                    repo_path,
                    source,
                    notify_chat,
                    now_str(),
                    mode,
                    system_workspace_id,
                ],
            )
            .context("create_pipeline_task")?;
        Ok(id)
    }

    /// Return "done" tasks that have no integration_queue entry (orphaned after restart).
    pub fn list_done_tasks_without_queue(&self) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!(
            "SELECT {TASK_COLS} FROM pipeline_tasks \
             WHERE status = 'done' \
             AND NOT EXISTS ( \
               SELECT 1 FROM integration_queue q \
               WHERE q.task_id = pipeline_tasks.id \
               AND q.status IN ('queued', 'excluded', 'merged') \
             )",
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map([], row_to_task)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_done_tasks_without_queue")?;
        Ok(tasks)
    }

    /// Reset integration_queue entries stuck in "merging" where the task is not yet merged.
    pub fn reset_stale_merging_queue(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn.execute(
            "UPDATE integration_queue SET status = 'queued' \
             WHERE status = 'merging' \
             AND task_id IN (SELECT id FROM pipeline_tasks WHERE status != 'merged')",
            [],
        )?;
        Ok(n)
    }

    pub fn active_task_count(&self) -> i64 {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row(
            "SELECT COUNT(*) FROM pipeline_tasks WHERE status NOT IN ('done','merged','failed','blocked','pending_review','human_review','purged')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    pub fn get_recent_merged_tasks(&self, limit: i64) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!(
            "SELECT {TASK_COLS} FROM pipeline_tasks WHERE status = 'merged' ORDER BY id DESC LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map(params![limit], row_to_task)?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_recent_merged_tasks")?;
        Ok(tasks)
    }

    pub fn recycle_failed_tasks(&self, repo_path: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute(
                "UPDATE pipeline_tasks SET status='backlog', attempt=0, last_error='' \
             WHERE status='failed' AND repo_path=?1",
                params![repo_path],
            )
            .context("recycle_failed_tasks")?;
        Ok(n)
    }

    pub fn reset_task_attempt(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE pipeline_tasks SET attempt=0 WHERE id=?1",
            params![id],
        )
        .context("reset_task_attempt")?;
        Ok(())
    }

    // ── Full Task List ────────────────────────────────────────────────────

    pub fn list_all_tasks(&self, repo_path: Option<&str>) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = if repo_path.is_some() {
            format!(
                "SELECT {TASK_COLS} FROM pipeline_tasks \
                 WHERE repo_path = ?1 \
                 ORDER BY id DESC"
            )
        } else {
            format!("SELECT {TASK_COLS} FROM pipeline_tasks ORDER BY id DESC")
        };
        let mut stmt = conn.prepare(&sql)?;
        let tasks = if let Some(repo_path) = repo_path {
            stmt.query_map(params![repo_path], row_to_task)?
                .collect::<pg::Result<Vec<_>>>()
        } else {
            stmt.query_map([], row_to_task)?
                .collect::<pg::Result<Vec<_>>>()
        }
        .context("list_all_tasks")?;
        Ok(tasks)
    }

    pub fn list_all_tasks_in_workspace(
        &self,
        workspace_id: i64,
        repo_path: Option<&str>,
    ) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = if repo_path.is_some() {
            format!(
                "SELECT {TASK_COLS} FROM pipeline_tasks \
                 WHERE workspace_id = ?1 AND repo_path = ?2 \
                 ORDER BY id DESC"
            )
        } else {
            format!(
                "SELECT {TASK_COLS} FROM pipeline_tasks WHERE workspace_id = ?1 ORDER BY id DESC"
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let tasks = if let Some(repo_path) = repo_path {
            stmt.query_map(params![workspace_id, repo_path], row_to_task)?
                .collect::<pg::Result<Vec<_>>>()
        } else {
            stmt.query_map(params![workspace_id], row_to_task)?
                .collect::<pg::Result<Vec<_>>>()
        }
        .context("list_all_tasks_in_workspace")?;
        Ok(tasks)
    }

    pub fn get_task_in_workspace(&self, workspace_id: i64, id: i64) -> Result<Option<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                &format!(
                    "SELECT {TASK_COLS} FROM pipeline_tasks WHERE id = ?1 AND workspace_id = ?2"
                ),
                params![id, workspace_id],
                row_to_task,
            )
            .optional()
            .context("get_task_in_workspace")?;
        Ok(result)
    }

    pub fn get_task_with_outputs(&self, id: i64) -> Result<Option<(Task, Vec<TaskOutput>)>> {
        let task = self.get_task(id)?;
        match task {
            None => Ok(None),
            Some(t) => {
                let outputs = self.get_task_outputs(id)?;
                Ok(Some((t, outputs)))
            },
        }
    }

    pub fn get_task_with_outputs_in_workspace(
        &self,
        workspace_id: i64,
        id: i64,
    ) -> Result<Option<(Task, Vec<TaskOutput>)>> {
        let task = self.get_task_in_workspace(workspace_id, id)?;
        match task {
            None => Ok(None),
            Some(t) => {
                let outputs = self.get_task_outputs(id)?;
                Ok(Some((t, outputs)))
            },
        }
    }

}
