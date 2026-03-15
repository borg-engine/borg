use super::*;

impl Db {
    // ── Users ─────────────────────────────────────────────────────────────

    pub fn get_user_default_workspace_id(&self, user_id: i64) -> Result<Option<i64>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT default_workspace_id FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .context("get_user_default_workspace_id")?;
        Ok(result.flatten())
    }

    pub fn set_user_default_workspace_id(&self, user_id: i64, workspace_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE users SET default_workspace_id = ?1 WHERE id = ?2",
            params![workspace_id, user_id],
        )
        .context("set_user_default_workspace_id")?;
        Ok(())
    }

    pub fn set_preferred_admin_workspace(&self, user_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let preferred = conn
            .query_row(
                "SELECT id FROM workspaces \
                 WHERE kind IN ('shared', 'system') \
                 ORDER BY CASE kind WHEN 'system' THEN 0 ELSE 1 END, id ASC LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(workspace_id) = preferred {
            conn.execute(
                "UPDATE users SET default_workspace_id = ?1 WHERE id = ?2",
                params![workspace_id, user_id],
            )
            .context("set_preferred_admin_workspace")?;
        }
        Ok(())
    }

    pub fn list_user_workspaces(&self, user_id: i64) -> Result<Vec<WorkspaceMembershipRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let default_workspace_id = conn
            .query_row(
                "SELECT default_workspace_id FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0);
        let mut stmt = conn.prepare(
            "SELECT w.id, w.name, w.slug, w.kind, wm.role, w.created_at \
             FROM workspace_memberships wm \
             JOIN workspaces w ON w.id = wm.workspace_id \
             WHERE wm.user_id = ?1 ORDER BY w.kind, w.name",
        )?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                let workspace_id: i64 = row.get(0)?;
                Ok(WorkspaceMembershipRow {
                    workspace_id,
                    name: row.get(1)?,
                    slug: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    kind: row.get(3)?,
                    role: row.get(4)?,
                    is_default: workspace_id == default_workspace_id,
                    created_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_user_workspaces")?;
        Ok(rows)
    }

    pub fn get_user_workspace_membership(
        &self,
        user_id: i64,
        workspace_id: i64,
    ) -> Result<Option<WorkspaceMembershipRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let default_workspace_id = conn
            .query_row(
                "SELECT default_workspace_id FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0);
        conn.query_row(
            "SELECT w.id, w.name, w.slug, w.kind, wm.role, w.created_at \
             FROM workspace_memberships wm \
             JOIN workspaces w ON w.id = wm.workspace_id \
             WHERE wm.user_id = ?1 AND wm.workspace_id = ?2",
            params![user_id, workspace_id],
            |row| {
                let resolved_workspace_id: i64 = row.get(0)?;
                Ok(WorkspaceMembershipRow {
                    workspace_id: resolved_workspace_id,
                    name: row.get(1)?,
                    slug: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    kind: row.get(3)?,
                    role: row.get(4)?,
                    is_default: resolved_workspace_id == default_workspace_id,
                    created_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })
            },
        )
        .optional()
        .context("get_user_workspace_membership")
    }

    pub fn user_has_workspace_access(&self, user_id: i64, workspace_id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let exists = conn
            .query_row(
                "SELECT 1 FROM workspace_memberships WHERE user_id = ?1 AND workspace_id = ?2",
                params![user_id, workspace_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("user_has_workspace_access")?
            .is_some();
        Ok(exists)
    }

    pub fn get_workspace(&self, workspace_id: i64) -> Result<Option<WorkspaceRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, name, slug, kind, owner_user_id, created_at FROM workspaces WHERE id = ?1",
            params![workspace_id],
            row_to_workspace,
        )
        .optional()
        .context("get_workspace")
    }

    pub fn get_system_workspace(&self) -> Result<Option<WorkspaceRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, name, slug, kind, owner_user_id, created_at FROM workspaces WHERE kind = 'system' ORDER BY id ASC LIMIT 1",
            [],
            row_to_workspace,
        )
        .optional()
        .context("get_system_workspace")
    }

    pub fn get_first_workspace_by_kind(&self, kind: &str) -> Result<Option<WorkspaceRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, name, slug, kind, owner_user_id, created_at FROM workspaces WHERE kind = ?1 ORDER BY id ASC LIMIT 1",
            params![kind],
            row_to_workspace,
        )
        .optional()
        .context("get_first_workspace_by_kind")
    }

    pub fn list_all_workspaces(&self) -> Result<Vec<WorkspaceRow>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, slug, kind, owner_user_id, created_at FROM workspaces ORDER BY kind, name, id",
        )?;
        let rows = stmt
            .query_map([], row_to_workspace)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_all_workspaces")?;
        Ok(rows)
    }

    pub fn create_workspace(
        &self,
        name: &str,
        kind: &str,
        owner_user_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let base_slug = unique_slug(name, 0);
        let slug = if base_slug.is_empty() {
            format!("workspace-{}", Utc::now().timestamp())
        } else {
            let mut candidate = base_slug.clone();
            let mut suffix = 2;
            loop {
                let taken = conn
                    .query_row(
                        "SELECT 1 FROM workspaces WHERE slug = ?1",
                        params![candidate.clone()],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?
                    .is_some();
                if !taken {
                    break candidate;
                }
                candidate = unique_slug(&base_slug, suffix);
                suffix += 1;
            }
        };
        let id = conn.execute_returning_id(
            "INSERT INTO workspaces (name, slug, kind, owner_user_id) VALUES (?1, ?2, ?3, ?4)",
            params![name, slug, kind, owner_user_id],
        )?;
        Ok(id)
    }

    pub fn add_workspace_member(&self, workspace_id: i64, user_id: i64, role: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, ?3) \
             ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
            params![workspace_id, user_id, role],
        )
        .context("add_workspace_member")?;
        Ok(())
    }

    pub fn ensure_system_workspace_membership(&self, user_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let system_ws = conn
            .query_row(
                "SELECT id FROM workspaces WHERE kind = 'system' ORDER BY id ASC LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(workspace_id) = system_ws {
            conn.execute(
                "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, 'member') \
                 ON CONFLICT (workspace_id, user_id) DO NOTHING",
                params![workspace_id, user_id],
            )
            .context("ensure_system_workspace_membership")?;
        }
        Ok(())
    }

    pub fn ensure_admin_workspace_memberships(&self, user_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare("SELECT id FROM workspaces ORDER BY id ASC")?;
        let workspace_ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<pg::Result<Vec<_>>>()?;
        for workspace_id in workspace_ids {
            conn.execute(
                "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, 'admin') \
                 ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = 'admin'",
                params![workspace_id, user_id],
            )?;
        }
        Ok(())
    }

    pub fn count_users(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", params![], |row| row.get(0))
            .context("count_users")?;
        Ok(count)
    }

    pub fn count_admin_users(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE is_admin = true",
                params![],
                |row| row.get(0),
            )
            .context("count_admin_users")?;
        Ok(count)
    }

    pub fn create_user(
        &self,
        username: &str,
        display_name: &str,
        password_hash: &str,
        is_admin: bool,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id: i64 = conn
            .query_row(
                "INSERT INTO users (username, display_name, password_hash, is_admin) \
                 VALUES (?1, ?2, ?3, ?4) RETURNING id",
                params![username, display_name, password_hash, is_admin],
                |row| row.get(0),
            )
            .context("create_user")?;
        let workspace_name = if display_name.trim().is_empty() {
            format!("{username} Personal")
        } else {
            format!("{display_name} Personal")
        };
        let workspace_id = Self::get_or_create_workspace(
            &conn,
            &workspace_name,
            "personal",
            Some(id),
            &unique_slug(&format!("{username}-personal"), 0),
        )?;
        conn.execute(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES (?1, ?2, 'owner') \
             ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
            params![workspace_id, id],
        )
        .context("create_user workspace membership")?;
        conn.execute(
            "UPDATE users SET default_workspace_id = ?1 WHERE id = ?2",
            params![workspace_id, id],
        )
        .context("create_user default workspace")?;
        Ok(id)
    }

    pub fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<(i64, String, String, String, bool)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT id, username, display_name, password_hash, is_admin FROM users WHERE username = ?1",
                params![username],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .context("get_user_by_username")?;
        Ok(result)
    }

    /// Look up a user by email address.
    /// For SSO users the username IS their email; as a fallback also checks the
    /// `contact_email` user setting.
    pub fn get_user_by_email(&self, email: &str) -> Result<Option<(i64, String, String, bool)>> {
        // SSO users have their email as username
        if let Ok(Some((id, username, display_name, _, is_admin))) =
            self.get_user_by_username(email)
        {
            return Ok(Some((id, username, display_name, is_admin)));
        }
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT u.id, u.username, u.display_name, u.is_admin \
                 FROM users u \
                 JOIN user_settings us ON us.user_id = u.id \
                 WHERE us.key = 'contact_email' AND LOWER(us.value) = LOWER(?1) \
                 LIMIT 1",
                params![email],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .context("get_user_by_email")?;
        Ok(result)
    }

    pub fn get_user_by_id(&self, id: i64) -> Result<Option<(i64, String, String, bool)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT id, username, display_name, is_admin FROM users WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .context("get_user_by_id")?;
        Ok(result)
    }

    pub fn set_user_admin(&self, id: i64, is_admin: bool) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE users SET is_admin = ?1 WHERE id = ?2",
            params![is_admin, id],
        )
        .context("set_user_admin")?;
        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<(i64, String, String, bool, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, username, display_name, is_admin, created_at FROM users ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_users")?;
        Ok(rows)
    }

    pub fn delete_user(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])
            .context("delete_user")?;
        Ok(())
    }

    pub fn update_user_password(&self, id: i64, password_hash: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            params![password_hash, id],
        )
        .context("update_user_password")?;
        Ok(())
    }

    // ── User Settings ────────────────────────────────────────────────────

    pub fn get_user_setting(&self, user_id: i64, key: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let result = conn
            .query_row(
                "SELECT value FROM user_settings WHERE user_id = ?1 AND key = ?2",
                params![user_id, key],
                |row| row.get(0),
            )
            .optional()
            .context("get_user_setting")?;
        Ok(result)
    }

    pub fn set_user_setting(&self, user_id: i64, key: &str, value: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "INSERT INTO user_settings (user_id, key, value) VALUES (?1, ?2, ?3) \
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value",
            params![user_id, key, value],
        )
        .context("set_user_setting")?;
        Ok(())
    }

    pub fn get_all_user_settings(&self, user_id: i64) -> Result<HashMap<String, String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare("SELECT key, value FROM user_settings WHERE user_id = ?1")?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                let k: String = row.get(0)?;
                let v: String = row.get(1)?;
                Ok((k, v))
            })?
            .collect::<pg::Result<Vec<_>>>()
            .context("get_all_user_settings")?;
        Ok(rows.into_iter().collect())
    }

    pub fn delete_user_setting(&self, user_id: i64, key: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "DELETE FROM user_settings WHERE user_id = ?1 AND key = ?2",
            params![user_id, key],
        )
        .context("delete_user_setting")?;
        Ok(())
    }

    // ── Chat sessions ─────────────────────────────────────────────────────

    pub fn get_session(&self, folder: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT session_id FROM sessions WHERE folder = ?1",
            params![folder],
            |r| r.get(0),
        )
        .optional()
        .context("get_session")
    }

    pub fn set_session(&self, folder: &str, session_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.execute(
            "INSERT INTO sessions (folder, session_id, created_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(folder) DO UPDATE SET session_id=excluded.session_id, created_at=excluded.created_at",

            params![folder, session_id, now_str()],
        )
        .context("set_session")?;
        Ok(())
    }

}
