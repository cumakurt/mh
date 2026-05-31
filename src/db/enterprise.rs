use anyhow::{Result, bail};
use chrono::{Duration, Utc};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use super::Database;
use crate::models::*;

impl Database {
    pub fn session_timeline(&self, session_id: &str) -> Result<Vec<TimelineEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT id, started_at, command, cwd, exit_code, duration_ms, environment_tier
             FROM commands
             WHERE session_id = ?
             ORDER BY started_at ASC, id ASC",
        )?;
        let rows = statement
            .query_map(params![session_id], |row| {
                Ok(TimelineEntry {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    command: row.get(2)?,
                    cwd: row.get(3)?,
                    exit_code: row.get(4)?,
                    duration_ms: row.get(5)?,
                    environment_tier: row.get(6)?,
                    risk_level: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_legal_hold(
        &self,
        label: &str,
        session_id: Option<&str>,
        command_id: Option<i64>,
        tag: Option<&str>,
        git_repo: Option<&str>,
        reason: Option<&str>,
    ) -> Result<i64> {
        self.connection.execute(
            "INSERT INTO legal_holds(label, session_id, command_id, tag, git_repo, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![label, session_id, command_id, tag, git_repo, reason],
        )?;
        let hold_id = self.connection.last_insert_rowid();
        self.apply_legal_holds()?;
        Ok(hold_id)
    }

    pub fn apply_legal_holds(&self) -> Result<usize> {
        let holds = self.list_legal_holds()?;
        let mut updated = 0usize;
        for hold in holds {
            let mut sql =
                String::from("UPDATE commands SET is_legal_hold = 1 WHERE is_legal_hold = 0");
            let mut values = Vec::<Value>::new();
            let mut conditions = Vec::new();

            if let Some(session_id) = hold.session_id.as_deref().filter(|value| !value.is_empty()) {
                conditions.push("session_id = ?");
                values.push(Value::Text(session_id.to_string()));
            }
            if let Some(command_id) = hold.command_id {
                conditions.push("id = ?");
                values.push(Value::Integer(command_id));
            }
            if let Some(tag) = hold.tag.as_deref().filter(|value| !value.is_empty()) {
                conditions.push("id IN (SELECT command_id FROM tags WHERE tag = ?)");
                values.push(Value::Text(tag.to_string()));
            }
            if let Some(git_repo) = hold.git_repo.as_deref().filter(|value| !value.is_empty()) {
                conditions.push("git_repo = ?");
                values.push(Value::Text(git_repo.to_string()));
            }

            if conditions.is_empty() {
                continue;
            }

            sql.push_str(" AND (");
            sql.push_str(&conditions.join(" OR "));
            sql.push(')');

            updated += self
                .connection
                .execute(&sql, params_from_iter(values.iter()))?;
        }
        Ok(updated)
    }

    pub fn list_legal_holds(&self) -> Result<Vec<LegalHoldRow>> {
        let mut statement = self.connection.prepare(
            "SELECT id, label, session_id, command_id, tag, git_repo, created_at, reason
             FROM legal_holds ORDER BY id DESC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(LegalHoldRow {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    session_id: row.get(2)?,
                    command_id: row.get(3)?,
                    tag: row.get(4)?,
                    git_repo: row.get(5)?,
                    created_at: row.get(6)?,
                    reason: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn remove_legal_hold(&self, hold_id: i64) -> Result<bool> {
        let deleted = self
            .connection
            .execute("DELETE FROM legal_holds WHERE id = ?", params![hold_id])?;
        if deleted > 0 {
            self.connection
                .execute("UPDATE commands SET is_legal_hold = 0", [])?;
            self.apply_legal_holds()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn retention_purge(&self, retention_days: u64, respect_legal_hold: bool) -> Result<usize> {
        let cutoff = (Utc::now() - Duration::days(retention_days as i64)).to_rfc3339();
        let mut sql =
            String::from("SELECT id FROM commands WHERE started_at < ? AND is_pinned = 0");
        if respect_legal_hold {
            sql.push_str(" AND is_legal_hold = 0");
        }
        let mut statement = self.connection.prepare(&sql)?;
        let ids = statement
            .query_map(params![cutoff], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        let deleted = self.delete_command_ids(&ids)?;
        if deleted > 0 && !self.fts_sync_via_triggers()? {
            self.rebuild_fts_index()?;
        }
        Ok(deleted)
    }

    pub fn insert_purge_audit(
        &self,
        action: &str,
        target: Option<&str>,
        count: usize,
        username: Option<&str>,
        hostname: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO purge_audit(action, target, count, username, hostname)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![action, target, count as i64, username, hostname],
        )?;
        Ok(())
    }

    pub fn create_runbook_from_session(
        &self,
        name: &str,
        description: Option<&str>,
        session_id: &str,
    ) -> Result<i64> {
        let timeline = self.session_timeline(session_id)?;
        if timeline.is_empty() {
            bail!("session {session_id} has no commands");
        }

        let tx = self.connection.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO runbooks(name, description, source_session_id, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
            params![name, description, session_id],
        )?;
        let runbook_id = tx.last_insert_rowid();
        for (index, entry) in timeline.iter().enumerate() {
            tx.execute(
                "INSERT INTO runbook_steps(runbook_id, step_order, command, cwd)
                 VALUES (?1, ?2, ?3, ?4)",
                params![runbook_id, index as i32 + 1, entry.command, entry.cwd],
            )?;
        }
        tx.commit()?;
        Ok(runbook_id)
    }

    pub fn list_runbooks(&self) -> Result<Vec<RunbookRow>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, description, source_session_id, created_at, updated_at
             FROM runbooks ORDER BY name ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(RunbookRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_session_id: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn runbook_steps(&self, name: &str) -> Result<Vec<RunbookStepRow>> {
        let runbook_id: i64 = self.connection.query_row(
            "SELECT id FROM runbooks WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;
        let mut statement = self.connection.prepare(
            "SELECT id, runbook_id, step_order, command, cwd, notes
             FROM runbook_steps WHERE runbook_id = ? ORDER BY step_order ASC",
        )?;
        let rows = statement
            .query_map(params![runbook_id], |row| {
                Ok(RunbookStepRow {
                    id: row.get(0)?,
                    runbook_id: row.get(1)?,
                    step_order: row.get(2)?,
                    command: row.get(3)?,
                    cwd: row.get(4)?,
                    notes: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
