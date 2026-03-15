use super::*;

impl Db {
    // ── Projects ──────────────────────────────────────────────────────────

    pub fn list_projects(&self) -> Result<Vec<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!("SELECT {PROJECT_COLS} FROM projects ORDER BY id DESC");
        let mut stmt = conn.prepare(&sql)?;
        let projects = stmt
            .query_map([], row_to_project)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_projects")?;
        Ok(projects)
    }

    pub fn list_projects_in_workspace(&self, workspace_id: i64) -> Result<Vec<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql =
            format!("SELECT {PROJECT_COLS} FROM projects WHERE workspace_id = ?1 ORDER BY id DESC");
        let mut stmt = conn.prepare(&sql)?;
        let projects = stmt
            .query_map(params![workspace_id], row_to_project)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_projects_in_workspace")?;
        Ok(projects)
    }

    pub fn search_projects(&self, query: &str) -> Result<Vec<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let pattern = format!("%{query}%");
        let sql = format!(
            "SELECT {PROJECT_COLS} FROM projects \
             WHERE name LIKE ?1 OR client_name LIKE ?1 OR case_number LIKE ?1 \
             OR jurisdiction LIKE ?1 OR matter_type LIKE ?1 \
             ORDER BY id DESC LIMIT 50"
        );
        let mut stmt = conn.prepare(&sql)?;
        let projects = stmt
            .query_map(params![pattern], row_to_project)?
            .collect::<pg::Result<Vec<_>>>()
            .context("search_projects")?;
        Ok(projects)
    }

    pub fn search_projects_in_workspace(
        &self,
        workspace_id: i64,
        query: &str,
    ) -> Result<Vec<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let pattern = format!("%{query}%");
        let sql = format!(
            "SELECT {PROJECT_COLS} FROM projects \
             WHERE workspace_id = ?1 AND (name LIKE ?2 OR client_name LIKE ?2 OR case_number LIKE ?2 \
             OR jurisdiction LIKE ?2 OR matter_type LIKE ?2) \
             ORDER BY id DESC LIMIT 50"
        );
        let mut stmt = conn.prepare(&sql)?;
        let projects = stmt
            .query_map(params![workspace_id, pattern], row_to_project)?
            .collect::<pg::Result<Vec<_>>>()
            .context("search_projects_in_workspace")?;
        Ok(projects)
    }

    pub fn get_project(&self, id: i64) -> Result<Option<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!("SELECT {PROJECT_COLS} FROM projects WHERE id=?1");
        let project = conn
            .query_row(&sql, params![id], row_to_project)
            .optional()
            .context("get_project")?;
        Ok(project)
    }

    pub fn get_project_in_workspace(
        &self,
        workspace_id: i64,
        id: i64,
    ) -> Result<Option<ProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!("SELECT {PROJECT_COLS} FROM projects WHERE id=?1 AND workspace_id = ?2");
        let project = conn
            .query_row(&sql, params![id, workspace_id], row_to_project)
            .optional()
            .context("get_project_in_workspace")?;
        Ok(project)
    }

    pub fn insert_project(
        &self,
        workspace_id: i64,
        name: &str,
        mode: &str,
        repo_path: &str,
        client_name: &str,
        jurisdiction: &str,
        matter_type: &str,
        privilege_level: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO projects (name, mode, repo_path, client_name, jurisdiction, matter_type, \
             privilege_level, workspace_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                name,
                mode,
                repo_path,
                client_name,
                jurisdiction,
                matter_type,
                privilege_level,
                workspace_id,
                created_at
            ],
        )
        .context("insert_project")?;
        Ok(id)
    }

    pub fn update_project(
        &self,
        id: i64,
        name: Option<&str>,
        client_name: Option<&str>,
        case_number: Option<&str>,
        jurisdiction: Option<&str>,
        matter_type: Option<&str>,
        opposing_counsel: Option<&str>,
        deadline: Option<Option<&str>>,
        privilege_level: Option<&str>,
        status: Option<&str>,
        repo_path: Option<&str>,
        default_template_id: Option<Option<i64>>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut sets = Vec::new();
        let mut vals: Vec<Box<dyn pg::ToSql>> = Vec::new();
        let mut idx = 1;

        macro_rules! maybe_set {
            ($field:expr, $col:expr) => {
                if let Some(v) = $field {
                    sets.push(format!("{} = ?{}", $col, idx));
                    vals.push(Box::new(v.to_string()));
                    idx += 1;
                }
            };
        }
        maybe_set!(name, "name");
        maybe_set!(client_name, "client_name");
        maybe_set!(case_number, "case_number");
        maybe_set!(jurisdiction, "jurisdiction");
        maybe_set!(matter_type, "matter_type");
        maybe_set!(opposing_counsel, "opposing_counsel");
        maybe_set!(privilege_level, "privilege_level");
        maybe_set!(status, "status");
        maybe_set!(repo_path, "repo_path");

        if let Some(dl) = deadline {
            sets.push(format!("deadline = ?{}", idx));
            vals.push(Box::new(dl.map(|s| s.to_string())));
            idx += 1;
        }

        if let Some(tid) = default_template_id {
            sets.push(format!("default_template_id = ?{}", idx));
            vals.push(Box::new(tid));
            idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE projects SET {} WHERE id = ?{}",
            sets.join(", "),
            idx,
        );
        vals.push(Box::new(id));
        let params: Vec<&dyn pg::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
        conn.execute(&sql, params.as_slice())
            .context("update_project")?;
        Ok(())
    }

    pub fn delete_project(&self, id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let tx = conn.transaction().context("delete_project transaction")?;

        tx.execute("DELETE FROM embeddings WHERE project_id=?1", params![id])
            .context("delete embeddings for project")?;
        tx.execute("DELETE FROM legal_fts WHERE project_id=?1", params![id])
            .context("delete legal_fts for project")?;
        tx.execute(
            "DELETE FROM project_corpus_stats WHERE project_id=?1",
            params![id],
        )
        .context("delete project_corpus_stats for project")?;
        tx.execute(
            "DELETE FROM upload_sessions WHERE project_id=?1",
            params![id],
        )
        .context("delete upload_sessions for project")?;
        tx.execute(
            "DELETE FROM cloud_connections WHERE project_id=?1",
            params![id],
        )
        .context("delete cloud_connections for project")?;
        tx.execute("DELETE FROM deadlines WHERE project_id=?1", params![id])
            .context("delete deadlines for project")?;
        tx.execute("DELETE FROM parties WHERE project_id=?1", params![id])
            .context("delete parties for project")?;
        tx.execute("DELETE FROM project_files WHERE project_id=?1", params![id])
            .context("delete project_files for project")?;
        tx.execute(
            "UPDATE knowledge_files SET project_id=NULL WHERE project_id=?1",
            params![id],
        )
        .context("unlink knowledge_files from project")?;
        tx.execute(
            "UPDATE pipeline_tasks SET project_id=NULL WHERE project_id=?1",
            params![id],
        )
        .context("unlink tasks from project")?;
        tx.execute(
            "UPDATE pipeline_events SET project_id=NULL WHERE project_id=?1",
            params![id],
        )
        .context("unlink pipeline_events from project")?;
        let affected = tx
            .execute("DELETE FROM projects WHERE id=?1", params![id])
            .context("delete_project")?;
        tx.commit().context("delete_project commit")?;
        Ok(affected > 0)
    }

    // ── Project sharing ──────────────────────────────────────────────────

    pub fn add_project_share(
        &self,
        project_id: i64,
        user_id: i64,
        role: &str,
        granted_by: i64,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn
            .execute_returning_id(
                "INSERT INTO project_shares (project_id, user_id, role, granted_by, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role",
                params![project_id, user_id, role, granted_by, created_at],
            )
            .context("add_project_share")?;
        Ok(id)
    }

    pub fn remove_project_share(&self, project_id: i64, user_id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let affected = conn
            .execute(
                "DELETE FROM project_shares WHERE project_id = ?1 AND user_id = ?2",
                params![project_id, user_id],
            )
            .context("remove_project_share")?;
        Ok(affected > 0)
    }

    pub fn list_project_shares(&self, project_id: i64) -> Result<Vec<ProjectShareRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT ps.id, ps.project_id, ps.user_id, ps.role, ps.granted_by, \
                    u.username, u.display_name, ps.created_at \
             FROM project_shares ps JOIN users u ON u.id = ps.user_id \
             WHERE ps.project_id = ?1 ORDER BY ps.created_at",
        )?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                Ok(ProjectShareRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    user_id: row.get(2)?,
                    role: row.get(3)?,
                    granted_by: row.get(4)?,
                    username: row.get(5)?,
                    display_name: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_shares")?;
        Ok(rows)
    }

    pub fn get_user_project_share(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> Result<Option<ProjectShareRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let row = conn
            .query_row(
                "SELECT ps.id, ps.project_id, ps.user_id, ps.role, ps.granted_by, \
                        u.username, u.display_name, ps.created_at \
                 FROM project_shares ps JOIN users u ON u.id = ps.user_id \
                 WHERE ps.project_id = ?1 AND ps.user_id = ?2",
                params![project_id, user_id],
                |row| {
                    Ok(ProjectShareRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        user_id: row.get(2)?,
                        role: row.get(3)?,
                        granted_by: row.get(4)?,
                        username: row.get(5)?,
                        display_name: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .context("get_user_project_share")?;
        Ok(row)
    }

    pub fn list_user_shared_projects(&self, user_id: i64) -> Result<Vec<(ProjectRow, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql =
            "SELECT p.id, p.workspace_id, p.name, p.mode, p.repo_path, p.client_name, \
             p.case_number, p.jurisdiction, p.matter_type, p.opposing_counsel, p.deadline, \
             p.privilege_level, p.status, p.default_template_id, p.created_at, p.session_privileged, \
             ps.role \
             FROM projects p \
             JOIN project_shares ps ON ps.project_id = p.id \
             WHERE ps.user_id = ?1 ORDER BY p.id DESC";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                let project = row_to_project(row)?;
                let role: String = row.get(16)?;
                Ok((project, role))
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_user_shared_projects")?;
        Ok(rows)
    }

    pub fn list_projects_shared_with_user(&self, user_id: i64) -> Result<Vec<SharedProjectRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = "SELECT p.id, p.workspace_id, p.name, p.mode, p.repo_path, p.client_name, \
             p.case_number, p.jurisdiction, p.matter_type, p.opposing_counsel, p.deadline, \
             p.privilege_level, p.status, p.default_template_id, p.created_at, p.session_privileged, \
             ps.role, w.name \
             FROM project_shares ps \
             JOIN projects p ON p.id = ps.project_id \
             JOIN workspaces w ON w.id = p.workspace_id \
             WHERE ps.user_id = ?1 \
             ORDER BY ps.created_at DESC";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                let created_at_str: String = row.get(14)?;
                let session_privileged_int: i64 = row.get(15)?;
                Ok(SharedProjectRow {
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
                    default_template_id: row.get(13)?,
                    created_at: created_at_str,
                    session_privileged: session_privileged_int != 0,
                    share_role: row.get(16)?,
                    workspace_name: row.get(17)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_projects_shared_with_user")?;
        Ok(rows)
    }

    pub fn create_project_share_link(
        &self,
        project_id: i64,
        token: &str,
        label: &str,
        expires_at: &str,
        created_by: i64,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO project_share_links (project_id, token, label, expires_at, created_by, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project_id, token, label, expires_at, created_by, created_at],
        )
        .context("create_project_share_link")?;
        Ok(id)
    }

    pub fn get_project_share_link_by_token(
        &self,
        token: &str,
    ) -> Result<Option<ProjectShareLinkRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let row = conn
            .query_row(
                "SELECT id, project_id, token, label, expires_at, created_by, revoked, created_at \
                 FROM project_share_links WHERE token = ?1 AND revoked = 0",
                params![token],
                |row| {
                    let revoked_int: i64 = row.get(6)?;
                    Ok(ProjectShareLinkRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        token: row.get(2)?,
                        label: row.get(3)?,
                        expires_at: row.get(4)?,
                        created_by: row.get(5)?,
                        revoked: revoked_int != 0,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .context("get_project_share_link_by_token")?;
        Ok(row)
    }

    pub fn list_project_share_links(&self, project_id: i64) -> Result<Vec<ProjectShareLinkRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, token, label, expires_at, created_by, revoked, created_at \
             FROM project_share_links WHERE project_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                let revoked_int: i64 = row.get(6)?;
                Ok(ProjectShareLinkRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    token: row.get(2)?,
                    label: row.get(3)?,
                    expires_at: row.get(4)?,
                    created_by: row.get(5)?,
                    revoked: revoked_int != 0,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_share_links")?;
        Ok(rows)
    }

    pub fn revoke_project_share_link(&self, id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let affected = conn
            .execute(
                "UPDATE project_share_links SET revoked = 1 WHERE id = ?1",
                params![id],
            )
            .context("revoke_project_share_link")?;
        Ok(affected > 0)
    }

    // ── Full-text search ──────────────────────────────────────────────────

    pub fn fts_index_document(
        &self,
        project_id: i64,
        task_id: i64,
        file_path: &str,
        title: &str,
        content: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        // Delete existing entry for this task+file, then re-insert
        conn.execute(
            "DELETE FROM legal_fts WHERE task_id = ?1 AND file_path = ?2",
            params![task_id, file_path],
        )?;
        conn.execute(
            "INSERT INTO legal_fts (project_id, task_id, file_path, title, content) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_id, task_id, file_path, title, content],
        ).context("fts_index_document")?;
        Ok(())
    }

    pub fn fts_remove_task(&self, task_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute("DELETE FROM legal_fts WHERE task_id = ?1", params![task_id])?;
        Ok(())
    }

    pub fn fts_search(
        &self,
        query: &str,
        project_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<FtsResult>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = if project_id.is_some() {
            "SELECT project_id, task_id, file_path, \
                    left(title, 240) as title_snip, \
                    left(content, 640) as content_snip, \
                    ts_rank_cd(search_vector, websearch_to_tsquery('english', ?1)) as rank \
             FROM legal_fts \
             WHERE search_vector @@ websearch_to_tsquery('english', ?1) AND project_id = ?2 \
             ORDER BY rank DESC, task_id DESC LIMIT ?3"
        } else {
            "SELECT project_id, task_id, file_path, \
                    left(title, 240) as title_snip, \
                    left(content, 640) as content_snip, \
                    ts_rank_cd(search_vector, websearch_to_tsquery('english', ?1)) as rank \
             FROM legal_fts \
             WHERE search_vector @@ websearch_to_tsquery('english', ?1) \
             ORDER BY rank DESC, task_id DESC LIMIT ?2"
        };
        let mut stmt = conn.prepare(sql)?;
        let results = if let Some(pid) = project_id {
            stmt.query_map(params![query, pid, limit], |r| {
                Ok(FtsResult {
                    project_id: r.get(0)?,
                    task_id: r.get(1)?,
                    file_path: r.get(2)?,
                    title_snippet: r.get(3)?,
                    content_snippet: r.get(4)?,
                    rank: r.get(5)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("fts_search")?
        } else {
            stmt.query_map(params![query, limit], |r| {
                Ok(FtsResult {
                    project_id: r.get(0)?,
                    task_id: r.get(1)?,
                    file_path: r.get(2)?,
                    title_snippet: r.get(3)?,
                    content_snippet: r.get(4)?,
                    rank: r.get(5)?,
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("fts_search")?
        };
        Ok(results)
    }

    pub fn list_project_tasks(&self, project_id: i64) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let sql = format!(
            "SELECT {TASK_COLS} FROM pipeline_tasks WHERE project_id = ?1 ORDER BY id DESC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map(params![project_id], row_to_task)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_tasks")?;
        Ok(tasks)
    }

    pub fn list_project_files(&self, project_id: i64) -> Result<Vec<ProjectFileRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {PROJECT_FILE_COLS} FROM project_files WHERE project_id=?1 ORDER BY id ASC"
        ))?;
        let files = stmt
            .query_map(params![project_id], row_to_project_file)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_files")?;
        Ok(files)
    }

    pub fn list_project_file_page(
        &self,
        project_id: i64,
        query: Option<&str>,
        limit: i64,
        offset: i64,
        cursor: Option<&ProjectFilePageCursor>,
        has_text: Option<bool>,
        privileged_only: Option<bool>,
    ) -> Result<(Vec<ProjectFileMetaRow>, i64)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let trimmed_query = query.map(str::trim).filter(|q| !q.is_empty());
        let mut base_where = vec!["project_id = ?".to_string()];
        let mut base_params: Vec<Box<dyn pg::types::ToSql>> = vec![Box::new(project_id)];

        if let Some(q) = trimmed_query {
            base_where.push("(lower(file_name) LIKE ? OR lower(source_path) LIKE ?)".to_string());
            let like = format!("%{}%", q.to_lowercase());
            base_params.push(Box::new(like.clone()));
            base_params.push(Box::new(like));
        }
        if let Some(flag) = has_text {
            base_where.push(if flag {
                "extracted_text != ''".to_string()
            } else {
                "extracted_text = ''".to_string()
            });
        }
        if let Some(flag) = privileged_only {
            base_where.push("privileged = ?".to_string());
            base_params.push(Box::new(if flag { 1_i64 } else { 0_i64 }));
        }

        let base_where_sql = base_where.join(" AND ");
        let fast_total = if trimmed_query.is_none() {
            conn.query_row(
                "SELECT total_files, privileged_files, text_files FROM project_corpus_stats WHERE project_id = ?1",
                params![project_id],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                )),
            )
            .ok()
            .map(|(total_files, privileged_files, text_files)| match (has_text, privileged_only) {
                (None, None) => total_files,
                (Some(true), None) => text_files,
                (Some(false), None) => (total_files - text_files).max(0),
                (None, Some(true)) => privileged_files,
                (None, Some(false)) => (total_files - privileged_files).max(0),
                _ => -1,
            })
            .filter(|n| *n >= 0)
        } else {
            None
        };
        let total: i64 = if let Some(total) = fast_total {
            total
        } else {
            let total_sql = format!("SELECT COUNT(*) FROM project_files WHERE {base_where_sql}");
            let total_params: Vec<&dyn pg::types::ToSql> =
                base_params.iter().map(|p| p.as_ref()).collect();
            conn.query_row(&total_sql, total_params.as_slice(), |row| row.get(0))
                .context("list_project_file_page count")?
        };

        let lim = limit.clamp(1, 200);
        let off = offset.max(0);
        let mut page_where = base_where;
        let mut page_params: Vec<Box<dyn pg::types::ToSql>> = base_params;
        if let Some(cursor) = cursor {
            page_where.push("(created_at < ? OR (created_at = ? AND id < ?))".to_string());
            page_params.push(Box::new(cursor.created_at.clone()));
            page_params.push(Box::new(cursor.created_at.clone()));
            page_params.push(Box::new(cursor.id));
        }
        page_params.push(Box::new(lim));
        if cursor.is_none() {
            page_params.push(Box::new(off));
        }
        let page_refs: Vec<&dyn pg::types::ToSql> =
            page_params.iter().map(|p| p.as_ref()).collect();
        let page_where_sql = page_where.join(" AND ");
        let sql = if cursor.is_some() {
            format!(
                "SELECT {PROJECT_FILE_META_COLS} FROM project_files \
                 WHERE {page_where_sql} ORDER BY created_at DESC, id DESC LIMIT ?"
            )
        } else {
            format!(
                "SELECT {PROJECT_FILE_META_COLS} FROM project_files \
                 WHERE {page_where_sql} ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
            )
        };
        let mut stmt = conn
            .prepare(&sql)
            .context("list_project_file_page prepare")?;
        let items = stmt
            .query_map(page_refs.as_slice(), row_to_project_file_meta)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_project_file_page rows")?;
        Ok((items, total))
    }

    pub fn search_project_file_name_hits(
        &self,
        project_id: i64,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ProjectFileMetaRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let lim = limit.clamp(1, 50);
        let like = format!("%{q}%");
        let sql = format!(
            "SELECT {PROJECT_FILE_META_COLS} FROM project_files \
             WHERE project_id = ?1 AND (lower(file_name) LIKE ?2 OR lower(source_path) LIKE ?2) \
             ORDER BY created_at DESC, id DESC LIMIT ?3"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("search_project_file_name_hits prepare")?;
        let rows = stmt
            .query_map(params![project_id, like, lim], row_to_project_file_meta)?
            .collect::<pg::Result<Vec<_>>>()
            .context("search_project_file_name_hits rows")?;
        Ok(rows)
    }

    pub fn get_project_file(
        &self,
        project_id: i64,
        file_id: i64,
    ) -> Result<Option<ProjectFileRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            &format!("SELECT {PROJECT_FILE_COLS} FROM project_files WHERE id=?1 AND project_id=?2"),
            params![file_id, project_id],
            row_to_project_file,
        )
        .optional()
        .context("get_project_file")
    }

    pub fn delete_project_file(&self, project_id: i64, file_id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute(
                "DELETE FROM project_files WHERE id = ?1 AND project_id = ?2",
                params![file_id, project_id],
            )
            .context("delete_project_file")?;
        Ok(n > 0)
    }

    pub fn delete_all_project_files(&self, project_id: i64) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let tx = conn
            .transaction()
            .context("delete_all_project_files transaction")?;
        tx.execute(
            "DELETE FROM embeddings WHERE project_id=?1",
            params![project_id],
        )
        .context("delete embeddings for project files")?;
        tx.execute(
            "DELETE FROM legal_fts WHERE project_id=?1",
            params![project_id],
        )
        .context("delete legal_fts for project files")?;
        let deleted = tx
            .execute(
                "DELETE FROM project_files WHERE project_id=?1",
                params![project_id],
            )
            .context("delete project files")?;
        tx.execute(
            "DELETE FROM project_corpus_stats WHERE project_id=?1",
            params![project_id],
        )
        .context("delete project corpus stats")?;
        tx.commit().context("delete_all_project_files commit")?;
        Ok(deleted as i64)
    }

    pub fn find_latest_project_file_by_source_path(
        &self,
        project_id: i64,
        source_path: &str,
    ) -> Result<Option<ProjectFileRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            &format!(
                "SELECT {PROJECT_FILE_COLS} FROM project_files \
                 WHERE project_id=?1 AND source_path=?2 ORDER BY id DESC LIMIT 1"
            ),
            params![project_id, source_path],
            row_to_project_file,
        )
        .optional()
        .context("find_latest_project_file_by_source_path")
    }

    pub fn insert_project_file(
        &self,
        project_id: i64,
        file_name: &str,
        source_path: &str,
        stored_path: &str,
        mime_type: &str,
        size_bytes: i64,
        content_hash: &str,
        privileged: bool,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let created_at = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO project_files \
             (project_id, file_name, source_path, stored_path, mime_type, size_bytes, content_hash, created_at, privileged) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project_id,
                file_name,
                source_path,
                stored_path,
                mime_type,
                size_bytes,
                content_hash,
                created_at,
                if privileged { 1i64 } else { 0i64 }
            ],
        )
        .context("insert_project_file")?;
        conn.execute(
            "INSERT INTO project_corpus_stats \
             (project_id, total_files, total_bytes, privileged_files, text_files, text_chars, updated_at) \
             VALUES (?1, 1, ?2, ?3, 0, 0, ?4) \
             ON CONFLICT(project_id) DO UPDATE SET
               total_files = project_corpus_stats.total_files + 1,
               total_bytes = project_corpus_stats.total_bytes + excluded.total_bytes,
               privileged_files = project_corpus_stats.privileged_files + excluded.privileged_files,
               updated_at = excluded.updated_at",
            params![
                project_id,
                size_bytes,
                if privileged { 1_i64 } else { 0_i64 },
                created_at,
            ],
        )
        .context("insert_project_file stats")?;
        if privileged {
            conn.execute(
                "UPDATE projects SET session_privileged = 1 WHERE id = ?1",
                params![project_id],
            )?;
        }
        Ok(id)
    }

    pub fn is_session_privileged(&self, project_id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let priv_int: i64 = conn
            .query_row(
                "SELECT session_privileged FROM projects WHERE id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(priv_int != 0)
    }

    pub fn set_session_privileged(&self, project_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE projects SET session_privileged = 1 WHERE id = ?1",
            params![project_id],
        )
        .context("set_session_privileged")?;
        Ok(())
    }

    pub fn find_project_file_by_hash(
        &self,
        project_id: i64,
        content_hash: &str,
    ) -> Result<Option<ProjectFileRow>> {
        if content_hash.trim().is_empty() {
            return Ok(None);
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            &format!("SELECT {PROJECT_FILE_COLS} FROM project_files WHERE project_id=?1 AND content_hash=?2 ORDER BY id ASC LIMIT 1"),
            params![project_id, content_hash],
            row_to_project_file,
        )
        .optional()
        .context("find_project_file_by_hash")
    }

    pub fn update_project_file_text(&self, file_id: i64, text: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let (project_id, old_chars): (i64, i64) = conn.query_row(
            "SELECT project_id, COALESCE(length(extracted_text), 0)::bigint FROM project_files WHERE id = ?1",
            params![file_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        conn.execute(
            "UPDATE project_files SET extracted_text = ?1 WHERE id = ?2",
            params![text, file_id],
        )?;
        let new_chars = text.chars().count() as i64;
        conn.execute(
            "INSERT INTO project_corpus_stats \
             (project_id, total_files, total_bytes, privileged_files, text_files, text_chars, updated_at) \
             VALUES (?1, 0, 0, 0, ?2, ?3, ?4) \
             ON CONFLICT(project_id) DO UPDATE SET
               text_files = project_corpus_stats.text_files + excluded.text_files,
               text_chars = project_corpus_stats.text_chars + excluded.text_chars,
               updated_at = excluded.updated_at",
            params![
                project_id,
                if old_chars == 0 && new_chars > 0 { 1_i64 } else { 0_i64 },
                new_chars - old_chars,
                now_str(),
            ],
        )
        .context("update_project_file_text stats")?;
        Ok(())
    }

    pub fn total_project_file_bytes(&self, project_id: i64) -> Result<i64> {
        Ok(self.get_project_file_stats(project_id)?.total_bytes)
    }

    pub fn get_project_file_stats(&self, project_id: i64) -> Result<ProjectFileStats> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let stats = conn
            .query_row(
                "SELECT project_id, total_files, total_bytes, privileged_files, text_files, text_chars, updated_at \
                 FROM project_corpus_stats WHERE project_id=?1",
                params![project_id],
                |row| {
                    Ok(ProjectFileStats {
                        project_id: row.get(0)?,
                        total_files: row.get(1)?,
                        total_bytes: row.get(2)?,
                        privileged_files: row.get(3)?,
                        text_files: row.get(4)?,
                        text_chars: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("get_project_file_stats")?;
        Ok(stats.unwrap_or(ProjectFileStats {
            project_id,
            ..ProjectFileStats::default()
        }))
    }

    pub fn create_upload_session(
        &self,
        project_id: i64,
        file_name: &str,
        mime_type: &str,
        file_size: i64,
        chunk_size: i64,
        total_chunks: i64,
        is_zip: bool,
        privileged: bool,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_str();
        let id = conn.execute_returning_id(
            "INSERT INTO upload_sessions \
             (project_id, file_name, mime_type, file_size, chunk_size, total_chunks, uploaded_bytes, is_zip, privileged, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, 'uploading', ?9, ?9)",
            params![
                project_id,
                file_name,
                mime_type,
                file_size,
                chunk_size,
                total_chunks,
                if is_zip { 1i64 } else { 0i64 },
                if privileged { 1i64 } else { 0i64 },
                now
            ],
        )
        .context("create_upload_session")?;
        Ok(id)
    }

    pub fn get_upload_session(&self, session_id: i64) -> Result<Option<UploadSession>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            "SELECT id, project_id, file_name, mime_type, file_size, chunk_size, total_chunks, \
                    uploaded_bytes, is_zip, privileged, status, stored_path, error, created_at, updated_at \
             FROM upload_sessions WHERE id = ?1",
            params![session_id],
            row_to_upload_session,
        )
        .optional()
        .context("get_upload_session")
    }

    pub fn list_upload_sessions(
        &self,
        project_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<UploadSession>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let lim = limit.clamp(1, 500);
        let sql = if project_id.is_some() {
            "SELECT id, project_id, file_name, mime_type, file_size, chunk_size, total_chunks, uploaded_bytes, \
                    is_zip, privileged, status, stored_path, error, created_at, updated_at \
             FROM upload_sessions WHERE project_id=?1 ORDER BY id DESC LIMIT ?2"
        } else {
            "SELECT id, project_id, file_name, mime_type, file_size, chunk_size, total_chunks, uploaded_bytes, \
                    is_zip, privileged, status, stored_path, error, created_at, updated_at \
             FROM upload_sessions ORDER BY id DESC LIMIT ?1"
        };
        let mut stmt = conn.prepare(sql).context("list_upload_sessions prepare")?;
        let out = if let Some(pid) = project_id {
            stmt.query_map(params![pid, lim], row_to_upload_session)?
                .collect::<pg::Result<Vec<_>>>()
                .context("list_upload_sessions map")?
        } else {
            stmt.query_map(params![lim], row_to_upload_session)?
                .collect::<pg::Result<Vec<_>>>()
                .context("list_upload_sessions map")?
        };
        Ok(out)
    }

    pub fn count_upload_sessions_by_status(
        &self,
        project_id: Option<i64>,
    ) -> Result<HashMap<String, i64>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let sql = if project_id.is_some() {
            "SELECT status, COUNT(*) FROM upload_sessions WHERE project_id=?1 GROUP BY status"
        } else {
            "SELECT status, COUNT(*) FROM upload_sessions GROUP BY status"
        };
        let mut stmt = conn
            .prepare(sql)
            .context("count_upload_sessions_by_status prepare")?;
        let mut out = HashMap::new();
        if let Some(pid) = project_id {
            let rows = stmt.query_map(params![pid], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (status, count) = row?;
                out.insert(status, count);
            }
        } else {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (status, count) = row?;
                out.insert(status, count);
            }
        }
        Ok(out)
    }

    pub fn count_active_upload_sessions(&self, project_id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let count = conn
            .query_row(
                "SELECT COUNT(*) FROM upload_sessions \
                 WHERE project_id=?1 AND status IN ('uploading','processing')",
                params![project_id],
                |row| row.get::<_, i64>(0),
            )
            .context("count_active_upload_sessions")?;
        Ok(count)
    }

    pub fn list_uploaded_chunks(&self, session_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT chunk_index FROM upload_session_chunks WHERE session_id=?1 ORDER BY chunk_index ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| row.get::<_, i64>(0))?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_uploaded_chunks")?;
        Ok(rows)
    }

    pub fn upsert_upload_chunk(
        &self,
        session_id: i64,
        chunk_index: i64,
        size_bytes: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_str();
        conn.execute(
            "INSERT INTO upload_session_chunks (session_id, chunk_index, size_bytes, created_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(session_id, chunk_index) DO UPDATE SET size_bytes=excluded.size_bytes",
            params![session_id, chunk_index, size_bytes, now],
        )
        .context("upsert_upload_chunk")?;
        conn.execute(
            "UPDATE upload_sessions \
             SET uploaded_bytes = (SELECT COALESCE(SUM(size_bytes), 0) FROM upload_session_chunks WHERE session_id = ?1), \
                 updated_at = ?2 \
             WHERE id = ?1",
            params![session_id, now],
        )
        .context("upsert_upload_chunk aggregate")?;
        Ok(())
    }

    pub fn set_upload_session_state(
        &self,
        session_id: i64,
        status: &str,
        stored_path: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "UPDATE upload_sessions \
             SET status = ?1, \
                 stored_path = COALESCE(?2, stored_path), \
                 error = COALESCE(?3, error), \
                 updated_at = ?4 \
             WHERE id = ?5",
            params![status, stored_path, error, now_str(), session_id],
        )
        .context("set_upload_session_state")?;
        Ok(())
    }

    pub fn summarize_themes(
        &self,
        project_id: Option<i64>,
        limit: i64,
        min_document_count: i64,
    ) -> Result<ThemeSummary> {
        const MAX_THEME_DOCUMENTS: i64 = 5_000;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut keyword_counts: HashMap<String, i64> = HashMap::new();
        let mut keyword_docs: HashMap<String, i64> = HashMap::new();
        let mut phrase_counts: HashMap<String, i64> = HashMap::new();
        let mut phrase_docs: HashMap<String, i64> = HashMap::new();
        let mut documents_scanned = 0i64;
        let mut tokens_scanned = 0i64;

        let sql = if project_id.is_some() {
            "SELECT extracted_text FROM project_files \
             WHERE project_id = ?1 AND extracted_text != '' \
             ORDER BY created_at DESC, id DESC LIMIT ?2"
        } else {
            "SELECT extracted_text FROM project_files \
             WHERE extracted_text != '' \
             ORDER BY created_at DESC, id DESC LIMIT ?1"
        };
        let mut stmt = conn.prepare(sql).context("summarize_themes prepare")?;
        let mut consume_text = |text: String| {
            documents_scanned += 1;
            let tokens = tokenize_for_themes(&text);
            tokens_scanned += tokens.len() as i64;
            if tokens.is_empty() {
                return;
            }
            let mut seen_keywords: HashSet<String> = HashSet::new();
            let mut seen_phrases: HashSet<String> = HashSet::new();
            for token in &tokens {
                *keyword_counts.entry(token.clone()).or_insert(0) += 1;
                seen_keywords.insert(token.clone());
            }
            for term in seen_keywords {
                *keyword_docs.entry(term).or_insert(0) += 1;
            }
            for pair in tokens.windows(2) {
                let phrase = format!("{} {}", pair[0], pair[1]);
                *phrase_counts.entry(phrase.clone()).or_insert(0) += 1;
                seen_phrases.insert(phrase);
            }
            for phrase in seen_phrases {
                *phrase_docs.entry(phrase).or_insert(0) += 1;
            }
        };
        if let Some(pid) = project_id {
            let rows =
                stmt.query_map(params![pid, MAX_THEME_DOCUMENTS], |r| r.get::<_, String>(0))?;
            for row in rows {
                consume_text(row?);
            }
        } else {
            let rows = stmt.query_map(params![MAX_THEME_DOCUMENTS], |r| r.get::<_, String>(0))?;
            for row in rows {
                consume_text(row?);
            }
        }

        let min_doc = min_document_count.max(1);
        let mut keywords = Vec::new();
        for (term, occurrences) in keyword_counts {
            let doc_count = keyword_docs.get(&term).copied().unwrap_or(0);
            if doc_count >= min_doc {
                push_theme_term(&mut keywords, term, occurrences, doc_count);
            }
        }
        keywords.sort_by(|a, b| {
            b.document_count
                .cmp(&a.document_count)
                .then_with(|| b.occurrences.cmp(&a.occurrences))
                .then_with(|| a.term.cmp(&b.term))
        });
        keywords.truncate(limit.max(1) as usize);

        let mut phrases = Vec::new();
        for (term, occurrences) in phrase_counts {
            let doc_count = phrase_docs.get(&term).copied().unwrap_or(0);
            if doc_count >= min_doc {
                push_theme_term(&mut phrases, term, occurrences, doc_count);
            }
        }
        phrases.sort_by(|a, b| {
            b.document_count
                .cmp(&a.document_count)
                .then_with(|| b.occurrences.cmp(&a.occurrences))
                .then_with(|| a.term.cmp(&b.term))
        });
        phrases.truncate(limit.max(1) as usize);

        Ok(ThemeSummary {
            documents_scanned,
            tokens_scanned,
            keywords,
            phrases,
        })
    }

    pub fn summarize_themes_for_workspace(
        &self,
        workspace_id: i64,
        limit: i64,
        min_document_count: i64,
    ) -> Result<ThemeSummary> {
        const MAX_THEME_DOCUMENTS: i64 = 5_000;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut keyword_counts: HashMap<String, i64> = HashMap::new();
        let mut keyword_docs: HashMap<String, i64> = HashMap::new();
        let mut phrase_counts: HashMap<String, i64> = HashMap::new();
        let mut phrase_docs: HashMap<String, i64> = HashMap::new();
        let mut documents_scanned = 0i64;
        let mut tokens_scanned = 0i64;

        let mut stmt = conn
            .prepare(
                "SELECT pf.extracted_text FROM project_files pf \
                 JOIN projects p ON p.id = pf.project_id \
                 WHERE p.workspace_id = ?1 AND pf.extracted_text != '' \
                 ORDER BY pf.created_at DESC, pf.id DESC LIMIT ?2",
            )
            .context("summarize_themes_for_workspace prepare")?;
        let rows = stmt.query_map(params![workspace_id, MAX_THEME_DOCUMENTS], |r| {
            r.get::<_, String>(0)
        })?;
        let mut consume_text = |text: String| {
            documents_scanned += 1;
            let tokens = tokenize_for_themes(&text);
            tokens_scanned += tokens.len() as i64;
            if tokens.is_empty() {
                return;
            }
            let mut seen_keywords: HashSet<String> = HashSet::new();
            let mut seen_phrases: HashSet<String> = HashSet::new();
            for token in &tokens {
                *keyword_counts.entry(token.clone()).or_insert(0) += 1;
                seen_keywords.insert(token.clone());
            }
            for term in seen_keywords {
                *keyword_docs.entry(term).or_insert(0) += 1;
            }
            for pair in tokens.windows(2) {
                let phrase = format!("{} {}", pair[0], pair[1]);
                *phrase_counts.entry(phrase.clone()).or_insert(0) += 1;
                seen_phrases.insert(phrase);
            }
            for phrase in seen_phrases {
                *phrase_docs.entry(phrase).or_insert(0) += 1;
            }
        };
        for row in rows {
            consume_text(row?);
        }

        let min_doc = min_document_count.max(1);
        let mut keywords = Vec::new();
        for (term, occurrences) in keyword_counts {
            let doc_count = keyword_docs.get(&term).copied().unwrap_or(0);
            if doc_count >= min_doc {
                push_theme_term(&mut keywords, term, occurrences, doc_count);
            }
        }
        keywords.sort_by(|a, b| {
            b.document_count
                .cmp(&a.document_count)
                .then_with(|| b.occurrences.cmp(&a.occurrences))
                .then_with(|| a.term.cmp(&b.term))
        });
        keywords.truncate(limit.max(1) as usize);

        let mut phrases = Vec::new();
        for (term, occurrences) in phrase_counts {
            let doc_count = phrase_docs.get(&term).copied().unwrap_or(0);
            if doc_count >= min_doc {
                push_theme_term(&mut phrases, term, occurrences, doc_count);
            }
        }
        phrases.sort_by(|a, b| {
            b.document_count
                .cmp(&a.document_count)
                .then_with(|| b.occurrences.cmp(&a.occurrences))
                .then_with(|| a.term.cmp(&b.term))
        });
        phrases.truncate(limit.max(1) as usize);

        Ok(ThemeSummary {
            documents_scanned,
            tokens_scanned,
            keywords,
            phrases,
        })
    }

    // ── Cloud connections ─────────────────────────────────────────────────

    pub fn insert_cloud_connection(
        &self,
        project_id: i64,
        provider: &str,
        access_token: &str,
        refresh_token: &str,
        token_expiry: &str,
        account_email: &str,
        account_id: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let id = conn.execute_returning_id(
            "INSERT INTO cloud_connections \
             (project_id, provider, access_token, refresh_token, token_expiry, account_email, account_id, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![project_id, provider, access_token, refresh_token, token_expiry,
                    account_email, account_id, now_str()],
        ).context("insert_cloud_connection")?;
        Ok(id)
    }

    pub fn list_cloud_connections(&self, project_id: i64) -> Result<Vec<CloudConnection>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, project_id, provider, access_token, refresh_token, token_expiry, \
                    account_email, account_id, created_at \
             FROM cloud_connections WHERE project_id=?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![project_id], row_to_cloud_connection)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_cloud_connection(&self, id: i64) -> Result<Option<CloudConnection>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            "SELECT id, project_id, provider, access_token, refresh_token, token_expiry, \
                    account_email, account_id, created_at \
             FROM cloud_connections WHERE id=?1",
            params![id],
            row_to_cloud_connection,
        )
        .optional()
        .context("get_cloud_connection")
    }

    pub fn update_cloud_connection_tokens(
        &self,
        id: i64,
        access_token: &str,
        refresh_token: &str,
        token_expiry: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "UPDATE cloud_connections SET access_token=?1, refresh_token=?2, token_expiry=?3 WHERE id=?4",
            params![access_token, refresh_token, token_expiry, id],
        ).context("update_cloud_connection_tokens")?;
        Ok(())
    }

    pub fn delete_cloud_connection(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("DELETE FROM cloud_connections WHERE id=?1", params![id])
            .context("delete_cloud_connection")?;
        Ok(())
    }

}
