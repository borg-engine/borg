use super::*;

impl Db {
    // ── Knowledge files ───────────────────────────────────────────────────

    pub fn total_knowledge_file_bytes(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let total = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM knowledge_files",
                [],
                |r| r.get(0),
            )
            .context("total_knowledge_file_bytes")?;
        Ok(total)
    }

    pub fn total_knowledge_file_bytes_in_workspace(&self, workspace_id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let total = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM knowledge_files WHERE workspace_id = ?1",
                params![workspace_id],
                |r| r.get(0),
            )
            .context("total_knowledge_file_bytes_in_workspace")?;
        Ok(total)
    }

    pub fn list_knowledge_files(&self) -> Result<Vec<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], row_to_knowledge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_knowledge_files_in_workspace(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE workspace_id = ?1 AND user_id IS NULL ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![workspace_id], row_to_knowledge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_knowledge_file_page(
        &self,
        query: Option<&str>,
        category: Option<&str>,
        jurisdiction: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<KnowledgeFile>, i64)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = vec!["1=1".to_string()];
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = Vec::new();

        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            where_clauses.push(
                "(lower(file_name) LIKE ? OR lower(description) LIKE ? OR lower(tags) LIKE ?)"
                    .to_string(),
            );
            let pattern = format!("%{}%", q.to_ascii_lowercase());
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
        if let Some(cat) = category.map(str::trim).filter(|c| !c.is_empty()) {
            where_clauses.push("category = ?".to_string());
            params_vec.push(Box::new(cat.to_string()));
        }
        if let Some(jur) = jurisdiction.map(str::trim).filter(|j| !j.is_empty()) {
            where_clauses.push("(jurisdiction = ? OR jurisdiction = '')".to_string());
            params_vec.push(Box::new(jur.to_string()));
        }

        let where_sql = where_clauses.join(" AND ");
        let total_sql = format!("SELECT COUNT(*) FROM knowledge_files WHERE {where_sql}");
        let total_params: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn
            .query_row(&total_sql, total_params.as_slice(), |row| row.get(0))
            .context("list_knowledge_file_page count")?;

        let lim = limit.clamp(1, 200);
        let off = offset.max(0);
        let mut page_params: Vec<Box<dyn pg::types::ToSql>> = params_vec;
        page_params.push(Box::new(lim));
        page_params.push(Box::new(off));
        let page_refs: Vec<&dyn pg::types::ToSql> =
            page_params.iter().map(|p| p.as_ref()).collect();
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE {where_sql} \
             ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("list_knowledge_file_page prepare")?;
        let items = stmt
            .query_map(page_refs.as_slice(), row_to_knowledge)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_knowledge_file_page rows")?;
        Ok((items, total))
    }

    pub fn list_knowledge_file_page_in_workspace(
        &self,
        workspace_id: i64,
        query: Option<&str>,
        category: Option<&str>,
        jurisdiction: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<KnowledgeFile>, i64)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = vec![
            "workspace_id = ?".to_string(),
            "user_id IS NULL".to_string(),
        ];
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = vec![Box::new(workspace_id)];

        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            where_clauses.push(
                "(lower(file_name) LIKE ? OR lower(description) LIKE ? OR lower(tags) LIKE ?)"
                    .to_string(),
            );
            let pattern = format!("%{}%", q.to_ascii_lowercase());
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
        if let Some(cat) = category.map(str::trim).filter(|c| !c.is_empty()) {
            where_clauses.push("category = ?".to_string());
            params_vec.push(Box::new(cat.to_string()));
        }
        if let Some(jur) = jurisdiction.map(str::trim).filter(|j| !j.is_empty()) {
            where_clauses.push("(jurisdiction = ? OR jurisdiction = '')".to_string());
            params_vec.push(Box::new(jur.to_string()));
        }

        let where_sql = where_clauses.join(" AND ");
        let total_sql = format!("SELECT COUNT(*) FROM knowledge_files WHERE {where_sql}");
        let total_params: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn
            .query_row(&total_sql, total_params.as_slice(), |row| row.get(0))
            .context("list_knowledge_file_page_in_workspace count")?;

        let lim = limit.clamp(1, 200);
        let off = offset.max(0);
        let mut page_params: Vec<Box<dyn pg::types::ToSql>> = params_vec;
        page_params.push(Box::new(lim));
        page_params.push(Box::new(off));
        let page_refs: Vec<&dyn pg::types::ToSql> =
            page_params.iter().map(|p| p.as_ref()).collect();
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE {where_sql} \
             ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("list_knowledge_file_page_in_workspace prepare")?;
        let items = stmt
            .query_map(page_refs.as_slice(), row_to_knowledge)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_knowledge_file_page_in_workspace rows")?;
        Ok((items, total))
    }

    /// Like list_knowledge_file_page_in_workspace but includes user-scoped files too.
    pub fn list_all_knowledge_in_workspace(
        &self,
        workspace_id: i64,
        query: Option<&str>,
        jurisdiction: Option<&str>,
        limit: i64,
    ) -> Result<Vec<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = vec!["workspace_id = ?".to_string()];
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = vec![Box::new(workspace_id)];
        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            where_clauses.push(
                "(lower(file_name) LIKE ? OR lower(description) LIKE ? OR lower(tags) LIKE ?)"
                    .to_string(),
            );
            let pattern = format!("%{}%", q.to_ascii_lowercase());
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
        if let Some(jur) = jurisdiction.map(str::trim).filter(|j| !j.is_empty()) {
            where_clauses.push("(jurisdiction = ? OR jurisdiction = '')".to_string());
            params_vec.push(Box::new(jur.to_string()));
        }
        let lim = limit.clamp(1, 200);
        params_vec.push(Box::new(lim));
        let where_sql = where_clauses.join(" AND ");
        let page_refs: Vec<&dyn pg::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE {where_sql} \
             ORDER BY created_at DESC, id DESC LIMIT ?"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("list_all_knowledge_in_workspace")?;
        let items = stmt
            .query_map(page_refs.as_slice(), row_to_knowledge)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_all_knowledge_in_workspace rows")?;
        Ok(items)
    }

    pub fn get_knowledge_file(&self, id: i64) -> Result<Option<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE id=?1",
            params![id],
            row_to_knowledge,
        )
        .optional()
        .context("get_knowledge_file")
    }

    pub fn get_knowledge_file_in_workspace(
        &self,
        workspace_id: i64,
        id: i64,
    ) -> Result<Option<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE id=?1 AND workspace_id = ?2",
            params![id, workspace_id],
            row_to_knowledge,
        )
        .optional()
        .context("get_knowledge_file_in_workspace")
    }

    pub fn list_templates(
        &self,
        category: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<Vec<KnowledgeFile>> {
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
        if let Some(jurisdiction) = jurisdiction.map(str::trim).filter(|j| !j.is_empty()) {
            where_clauses.push("(jurisdiction = ? OR jurisdiction = '')".to_string());
            params_vec.push(Box::new(jurisdiction.to_string()));
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files{where_sql} \
             ORDER BY category, file_name"
        );
        let param_refs: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), row_to_knowledge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_templates_in_workspace(
        &self,
        workspace_id: i64,
        category: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<Vec<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = vec!["workspace_id = ?".to_string()];
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> = vec![Box::new(workspace_id)];
        if let Some(category) = category.map(str::trim).filter(|c| !c.is_empty()) {
            where_clauses.push("category = ?".to_string());
            params_vec.push(Box::new(category.to_string()));
        }
        if let Some(jurisdiction) = jurisdiction.map(str::trim).filter(|j| !j.is_empty()) {
            where_clauses.push("(jurisdiction = ? OR jurisdiction = '')".to_string());
            params_vec.push(Box::new(jurisdiction.to_string()));
        }
        let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files{where_sql} \
             ORDER BY category, file_name"
        );
        let param_refs: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), row_to_knowledge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn insert_knowledge_file(
        &self,
        workspace_id: i64,
        file_name: &str,
        description: &str,
        size_bytes: i64,
        inline: bool,
    ) -> Result<i64> {
        self.insert_knowledge_file_for_user(
            workspace_id,
            None,
            file_name,
            description,
            size_bytes,
            inline,
        )
    }

    pub fn insert_knowledge_file_for_user(
        &self,
        workspace_id: i64,
        user_id: Option<i64>,
        file_name: &str,
        description: &str,
        size_bytes: i64,
        inline: bool,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id = conn.execute_returning_id(
            "INSERT INTO knowledge_files (workspace_id, user_id, file_name, description, size_bytes, \"inline\") \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![workspace_id, user_id, file_name, description, size_bytes, inline as i64],
        )?;
        Ok(id)
    }

    pub fn delete_knowledge_file(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute("DELETE FROM knowledge_files WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn delete_knowledge_file_in_workspace(&self, workspace_id: i64, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM knowledge_files WHERE id=?1 AND workspace_id = ?2",
            params![id, workspace_id],
        )?;
        Ok(())
    }

    pub fn delete_all_knowledge_files(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let deleted = conn
            .execute("DELETE FROM knowledge_files", [])
            .context("delete_all_knowledge_files")?;
        Ok(deleted as i64)
    }

    pub fn delete_all_knowledge_files_in_workspace(&self, workspace_id: i64) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let deleted = conn
            .execute(
                "DELETE FROM knowledge_files WHERE workspace_id = ?1 AND user_id IS NULL",
                params![workspace_id],
            )
            .context("delete_all_knowledge_files_in_workspace")?;
        Ok(deleted as i64)
    }

    // ── User-scoped knowledge ("My Knowledge") ──────────────────────────

    pub fn list_user_knowledge_page(
        &self,
        workspace_id: i64,
        user_id: i64,
        query: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<KnowledgeFile>, i64)> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut where_clauses = vec!["workspace_id = ?".to_string(), "user_id = ?".to_string()];
        let mut params_vec: Vec<Box<dyn pg::types::ToSql>> =
            vec![Box::new(workspace_id), Box::new(user_id)];
        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            where_clauses.push(
                "(lower(file_name) LIKE ? OR lower(description) LIKE ? OR lower(tags) LIKE ?)"
                    .to_string(),
            );
            let pattern = format!("%{}%", q.to_ascii_lowercase());
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
        let where_sql = where_clauses.join(" AND ");
        let total_sql = format!("SELECT COUNT(*) FROM knowledge_files WHERE {where_sql}");
        let total_params: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn
            .query_row(&total_sql, total_params.as_slice(), |row| row.get(0))
            .context("list_user_knowledge_page count")?;
        let lim = limit.clamp(1, 200);
        let off = offset.max(0);
        let mut page_params: Vec<Box<dyn pg::types::ToSql>> = params_vec;
        page_params.push(Box::new(lim));
        page_params.push(Box::new(off));
        let page_refs: Vec<&dyn pg::types::ToSql> =
            page_params.iter().map(|p| p.as_ref()).collect();
        let sql = format!(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE {where_sql} \
             ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
        );
        let mut stmt = conn
            .prepare(&sql)
            .context("list_user_knowledge_page prepare")?;
        let items = stmt
            .query_map(page_refs.as_slice(), row_to_knowledge)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_user_knowledge_page rows")?;
        Ok((items, total))
    }

    pub fn list_user_knowledge_files(
        &self,
        workspace_id: i64,
        user_id: i64,
    ) -> Result<Vec<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE workspace_id = ?1 AND user_id = ?2 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![workspace_id, user_id], row_to_knowledge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_user_knowledge_file(
        &self,
        workspace_id: i64,
        user_id: i64,
        id: i64,
    ) -> Result<Option<KnowledgeFile>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, workspace_id, file_name, description, size_bytes, \"inline\", created_at, \
                    tags, category, jurisdiction, project_id, user_id \
             FROM knowledge_files WHERE id=?1 AND workspace_id = ?2 AND user_id = ?3",
            params![id, workspace_id, user_id],
            row_to_knowledge,
        )
        .optional()
        .context("get_user_knowledge_file")
    }

    pub fn delete_user_knowledge_file(
        &self,
        workspace_id: i64,
        user_id: i64,
        id: i64,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM knowledge_files WHERE id=?1 AND workspace_id = ?2 AND user_id = ?3",
            params![id, workspace_id, user_id],
        )?;
        Ok(())
    }

    pub fn delete_all_user_knowledge_files(&self, workspace_id: i64, user_id: i64) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let deleted = conn
            .execute(
                "DELETE FROM knowledge_files WHERE workspace_id = ?1 AND user_id = ?2",
                params![workspace_id, user_id],
            )
            .context("delete_all_user_knowledge_files")?;
        Ok(deleted as i64)
    }

    pub fn total_user_knowledge_bytes(&self, workspace_id: i64, user_id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let total = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM knowledge_files WHERE workspace_id = ?1 AND user_id = ?2",
                params![workspace_id, user_id],
                |r| r.get(0),
            )
            .context("total_user_knowledge_bytes")?;
        Ok(total)
    }

    pub fn update_knowledge_file(
        &self,
        id: i64,
        description: Option<&str>,
        inline: Option<bool>,
        tags: Option<&str>,
        category: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        if let Some(d) = description {
            conn.execute(
                "UPDATE knowledge_files SET description=?1 WHERE id=?2",
                params![d, id],
            )?;
        }
        if let Some(i) = inline {
            conn.execute(
                "UPDATE knowledge_files SET \"inline\"=?1 WHERE id=?2",
                params![i as i64, id],
            )?;
        }
        if let Some(t) = tags {
            conn.execute(
                "UPDATE knowledge_files SET tags=?1 WHERE id=?2",
                params![t, id],
            )?;
        }
        if let Some(c) = category {
            conn.execute(
                "UPDATE knowledge_files SET category=?1 WHERE id=?2",
                params![c, id],
            )?;
        }
        if let Some(j) = jurisdiction {
            conn.execute(
                "UPDATE knowledge_files SET jurisdiction=?1 WHERE id=?2",
                params![j, id],
            )?;
        }
        Ok(())
    }

    pub fn update_knowledge_file_in_workspace(
        &self,
        workspace_id: i64,
        id: i64,
        description: Option<&str>,
        inline: Option<bool>,
        tags: Option<&str>,
        category: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        if let Some(d) = description {
            conn.execute(
                "UPDATE knowledge_files SET description=?1 WHERE id=?2 AND workspace_id = ?3",
                params![d, id, workspace_id],
            )?;
        }
        if let Some(i) = inline {
            conn.execute(
                "UPDATE knowledge_files SET \"inline\"=?1 WHERE id=?2 AND workspace_id = ?3",
                params![i as i64, id, workspace_id],
            )?;
        }
        if let Some(t) = tags {
            conn.execute(
                "UPDATE knowledge_files SET tags=?1 WHERE id=?2 AND workspace_id = ?3",
                params![t, id, workspace_id],
            )?;
        }
        if let Some(c) = category {
            conn.execute(
                "UPDATE knowledge_files SET category=?1 WHERE id=?2 AND workspace_id = ?3",
                params![c, id, workspace_id],
            )?;
        }
        if let Some(j) = jurisdiction {
            conn.execute(
                "UPDATE knowledge_files SET jurisdiction=?1 WHERE id=?2 AND workspace_id = ?3",
                params![j, id, workspace_id],
            )?;
        }
        Ok(())
    }

    // ── Embeddings ────────────────────────────────────────────────────────

    pub fn upsert_embedding(
        &self,
        project_id: Option<i64>,
        task_id: Option<i64>,
        chunk_text: &str,
        file_path: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let hash = crate::knowledge::hash_chunk(chunk_text);
        let blob = crate::knowledge::embedding_to_bytes(embedding);
        conn.execute(
            "INSERT INTO embeddings (project_id, task_id, chunk_text, chunk_hash, file_path, embedding, dims) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(chunk_hash) DO UPDATE SET embedding = excluded.embedding",
            params![project_id, task_id, chunk_text, hash, file_path, blob, embedding.len() as i64],
        )
        .context("upsert_embedding")?;
        Ok(())
    }

    pub fn remove_task_embeddings(&self, task_id: i64) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute(
                "DELETE FROM embeddings WHERE task_id = ?1",
                params![task_id],
            )
            .context("remove_task_embeddings")?;
        Ok(n)
    }

    pub fn search_embeddings(
        &self,
        query_embedding: &[f32],
        limit: usize,
        project_id: Option<i64>,
    ) -> Result<Vec<crate::knowledge::EmbeddingSearchResult>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let cap = limit.clamp(1, 5000);
        let (sql, params_vec): (String, Vec<Box<dyn pg::types::ToSql>>) = match project_id {
            Some(pid) => (
                "SELECT id, project_id, task_id, chunk_text, file_path, embedding FROM embeddings WHERE project_id = ?1".to_string(),
                vec![Box::new(pid) as Box<dyn pg::types::ToSql>],
            ),
            None => (
                "SELECT id, project_id, task_id, chunk_text, file_path, embedding FROM embeddings".to_string(),
                vec![],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn pg::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row: &pg::Row| {
            Ok((
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        })?;

        let mut results = Vec::with_capacity(cap.min(128));
        let flush_at = cap.saturating_mul(4).max(cap + 8);
        for row in rows {
            let (pid, tid, text, path, blob) = row.context("search_embeddings row")?;
            let emb = crate::knowledge::bytes_to_embedding(&blob);
            let score = crate::knowledge::cosine_similarity(query_embedding, &emb);
            results.push(crate::knowledge::EmbeddingSearchResult {
                chunk_text: text,
                file_path: path,
                project_id: pid,
                task_id: tid,
                score,
            });
            if results.len() >= flush_at {
                results.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                results.truncate(cap);
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(cap);
        Ok(results)
    }

    pub fn list_recent_project_files(
        &self,
        project_id: i64,
        limit: i64,
        require_text: bool,
    ) -> Result<Vec<ProjectFileRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let lim = limit.clamp(1, 100);
        let sql = if require_text {
            format!(
                "SELECT {PROJECT_FILE_COLS} FROM project_files \
                 WHERE project_id=?1 AND extracted_text != '' \
                 ORDER BY created_at DESC, id DESC LIMIT ?2"
            )
        } else {
            format!(
                "SELECT {PROJECT_FILE_COLS} FROM project_files \
                 WHERE project_id=?1 ORDER BY created_at DESC, id DESC LIMIT ?2"
            )
        };
        let mut stmt = conn
            .prepare(&sql)
            .context("list_recent_project_files prepare")?;
        let files = stmt
            .query_map(params![project_id, lim], row_to_project_file)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_recent_project_files rows")?;
        Ok(files)
    }

    pub fn list_recent_completed_project_tasks(
        &self,
        project_id: i64,
        limit: i64,
    ) -> Result<Vec<Task>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let lim = limit.clamp(1, 50);
        let sql = format!(
            "SELECT {TASK_COLS} FROM pipeline_tasks \
             WHERE project_id = ?1 AND status IN ('merged','done','complete','purge','purged') \
             ORDER BY id DESC LIMIT ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map(params![project_id, lim], row_to_task)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_recent_completed_project_tasks")?;
        Ok(tasks)
    }

    pub fn embedding_count(&self) -> i64 {
        let Ok(conn) = self.conn.lock() else { return 0 };
        conn.query_row("SELECT COUNT(*) FROM embeddings", [], |r: &pg::Row| {
            r.get(0)
        })
        .unwrap_or(0)
    }

    // ── Knowledge Repos ───────────────────────────────────────────────────

    fn row_to_knowledge_repo(row: &pg::Row<'_>) -> pg::Result<KnowledgeRepo> {
        Ok(KnowledgeRepo {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            user_id: row.get(2)?,
            url: row.get(3)?,
            name: row.get(4)?,
            local_path: row.get(5)?,
            status: row.get(6)?,
            error_msg: row.get(7)?,
            created_at: row.get(8)?,
        })
    }

    pub fn list_knowledge_repos(
        &self,
        workspace_id: i64,
        user_id: Option<i64>,
    ) -> Result<Vec<KnowledgeRepo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = if user_id.is_some() {
            conn.prepare(
                "SELECT id, workspace_id, user_id, url, name, local_path, status, error_msg, created_at \
                 FROM knowledge_repos WHERE workspace_id = ?1 AND user_id = ?2 ORDER BY created_at ASC",
            )?
        } else {
            conn.prepare(
                "SELECT id, workspace_id, user_id, url, name, local_path, status, error_msg, created_at \
                 FROM knowledge_repos WHERE workspace_id = ?1 AND user_id IS NULL ORDER BY created_at ASC",
            )?
        };
        let rows = if let Some(uid) = user_id {
            stmt.query_map(params![workspace_id, uid], Self::row_to_knowledge_repo)?
        } else {
            stmt.query_map(params![workspace_id], Self::row_to_knowledge_repo)?
        };
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_all_knowledge_repos(&self) -> Result<Vec<KnowledgeRepo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, user_id, url, name, local_path, status, error_msg, created_at \
             FROM knowledge_repos ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_knowledge_repo)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn insert_knowledge_repo(
        &self,
        workspace_id: i64,
        user_id: Option<i64>,
        url: &str,
        name: &str,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        Ok(conn.execute_returning_id(
            "INSERT INTO knowledge_repos (workspace_id, user_id, url, name, status) VALUES (?1, ?2, ?3, ?4, 'pending')",
            params![workspace_id, user_id, url, name],
        )?)
    }

    pub fn update_knowledge_repo_status(
        &self,
        id: i64,
        status: &str,
        local_path: &str,
        error_msg: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE knowledge_repos SET status = ?1, local_path = ?2, error_msg = ?3 WHERE id = ?4",
            params![status, local_path, error_msg, id],
        )?;
        Ok(())
    }

    pub fn delete_knowledge_repo(&self, id: i64, workspace_id: i64) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let local_path: Option<String> = conn
            .query_row(
                "SELECT local_path FROM knowledge_repos WHERE id = ?1 AND workspace_id = ?2",
                params![id, workspace_id],
                |r| r.get(0),
            )
            .optional()?;
        conn.execute(
            "DELETE FROM knowledge_repos WHERE id = ?1 AND workspace_id = ?2",
            params![id, workspace_id],
        )?;
        Ok(local_path.unwrap_or_default())
    }

}
