use super::*;

impl Db {
    // ── Cron scheduling ───────────────────────────────────────────────────

    pub fn list_cron_jobs(&self) -> Result<Vec<crate::cron::CronJob>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, schedule, job_type, config, project_id, enabled, \
             last_run, next_run, created_at \
             FROM cron_jobs ORDER BY id ASC",
        )?;
        let rows = stmt
            .query_map([], crate::cron::row_to_cron_job)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_cron_jobs")?;
        Ok(rows)
    }

    pub fn get_cron_job(&self, id: i64) -> Result<Option<crate::cron::CronJob>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        conn.query_row(
            "SELECT id, name, schedule, job_type, config, project_id, enabled, \
             last_run, next_run, created_at \
             FROM cron_jobs WHERE id = ?1",
            params![id],
            crate::cron::row_to_cron_job,
        )
        .optional()
        .context("get_cron_job")
    }

    pub fn insert_cron_job(
        &self,
        name: &str,
        schedule: &str,
        job_type: &crate::cron::CronJobType,
        config: &serde_json::Value,
        project_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let config_str = serde_json::to_string(config).unwrap_or_else(|_| "{}".into());
        let next_run = crate::cron::compute_next_run(schedule, Utc::now())
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
        let id = conn
            .execute_returning_id(
                "INSERT INTO cron_jobs (name, schedule, job_type, config, project_id, next_run) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    name,
                    schedule,
                    job_type.as_str(),
                    config_str,
                    project_id,
                    next_run
                ],
            )
            .context("insert_cron_job")?;
        Ok(id)
    }

    pub fn update_cron_job(
        &self,
        id: i64,
        name: Option<&str>,
        schedule: Option<&str>,
        job_type: Option<&crate::cron::CronJobType>,
        config: Option<&serde_json::Value>,
        project_id: Option<Option<i64>>,
        enabled: Option<bool>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut sets = Vec::new();
        let mut vals: Vec<Box<dyn pg::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(v) = name {
            sets.push(format!("name = ?{idx}"));
            vals.push(Box::new(v.to_string()));
            idx += 1;
        }
        if let Some(v) = schedule {
            sets.push(format!("schedule = ?{idx}"));
            vals.push(Box::new(v.to_string()));
            idx += 1;
            let next = crate::cron::compute_next_run(v, Utc::now())
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
            sets.push(format!("next_run = ?{idx}"));
            vals.push(Box::new(next));
            idx += 1;
        }
        if let Some(v) = job_type {
            sets.push(format!("job_type = ?{idx}"));
            vals.push(Box::new(v.as_str().to_string()));
            idx += 1;
        }
        if let Some(v) = config {
            sets.push(format!("config = ?{idx}"));
            vals.push(Box::new(
                serde_json::to_string(v).unwrap_or_else(|_| "{}".into()),
            ));
            idx += 1;
        }
        if let Some(v) = project_id {
            sets.push(format!("project_id = ?{idx}"));
            vals.push(Box::new(v));
            idx += 1;
        }
        if let Some(v) = enabled {
            sets.push(format!("enabled = ?{idx}"));
            vals.push(Box::new(if v { 1i64 } else { 0i64 }));
            idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE cron_jobs SET {} WHERE id = ?{}",
            sets.join(", "),
            idx
        );
        vals.push(Box::new(id));
        let params: Vec<&dyn pg::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
        conn.execute(&sql, params.as_slice())
            .context("update_cron_job")?;
        Ok(())
    }

    pub fn delete_cron_job(&self, id: i64) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let n = conn
            .execute("DELETE FROM cron_jobs WHERE id = ?1", params![id])
            .context("delete_cron_job")?;
        Ok(n > 0)
    }

    pub fn list_due_cron_jobs(&self) -> Result<Vec<crate::cron::CronJob>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let now = now_str();
        let mut stmt = conn.prepare(
            "SELECT id, name, schedule, job_type, config, project_id, enabled, \
             last_run, next_run, created_at \
             FROM cron_jobs \
             WHERE enabled = 1 AND next_run IS NOT NULL AND next_run <= ?1 \
             ORDER BY next_run ASC",
        )?;
        let rows = stmt
            .query_map(params![now], crate::cron::row_to_cron_job)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_due_cron_jobs")?;
        Ok(rows)
    }

    pub fn update_cron_job_after_run(
        &self,
        id: i64,
        last_run: &DateTime<Utc>,
        next_run: Option<&DateTime<Utc>>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let last_str = last_run.format("%Y-%m-%d %H:%M:%S").to_string();
        let next_str = next_run.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
        conn.execute(
            "UPDATE cron_jobs SET last_run = ?1, next_run = ?2 WHERE id = ?3",
            params![last_str, next_str, id],
        )
        .context("update_cron_job_after_run")?;
        Ok(())
    }

    pub fn insert_cron_run(&self, job_id: i64) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let id = conn
            .execute_returning_id(
                "INSERT INTO cron_runs (job_id, status) VALUES (?1, 'running')",
                params![job_id],
            )
            .context("insert_cron_run")?;
        Ok(id)
    }

    pub fn update_cron_run(
        &self,
        id: i64,
        status: &str,
        result: Option<&str>,
        error: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let finished = now_str();
        conn.execute(
            "UPDATE cron_runs SET status = ?1, result = ?2, error = ?3, \
             finished_at = ?4, task_id = ?5 WHERE id = ?6",
            params![status, result, error, finished, task_id, id],
        )
        .context("update_cron_run")?;
        Ok(())
    }

    pub fn list_cron_runs(&self, job_id: i64, limit: i64) -> Result<Vec<crate::cron::CronRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, finished_at, status, result, error, task_id \
             FROM cron_runs WHERE job_id = ?1 \
             ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![job_id, limit], crate::cron::row_to_cron_run)?
            .collect::<pg::Result<Vec<_>>>()
            .context("list_cron_runs")?;
        Ok(rows)
    }

}
