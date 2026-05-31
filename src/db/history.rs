use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::types::Value;
use rusqlite::params_from_iter;

use super::Database;
use super::helpers::*;
use crate::models::*;

impl Database {
    pub fn clear_history(
        &self,
        user: Option<&str>,
        before: Option<&str>,
        keep_pinned: bool,
    ) -> Result<usize> {
        let mut sql = String::from("SELECT id FROM commands WHERE 1 = 1");
        let mut values = Vec::<Value>::new();

        if let Some(user) = user.filter(|value| !value.is_empty()) {
            sql.push_str(" AND username = ?");
            values.push(Value::Text(user.to_string()));
        }

        if let Some(before) = before.filter(|value| !value.is_empty()) {
            sql.push_str(" AND started_at <= ?");
            values.push(Value::Text(normalize_date_bound(before, false)));
        }

        if keep_pinned {
            sql.push_str(" AND is_pinned = 0");
        }
        sql.push_str(" AND is_legal_hold = 0");

        let mut statement = self.connection.prepare(&sql)?;
        let ids = statement
            .query_map(params_from_iter(values.iter()), |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        let deleted = self.delete_command_ids(&ids)?;
        if deleted > 0 && !self.fts_sync_via_triggers()? {
            self.rebuild_fts_index()?;
        }
        Ok(deleted)
    }

    pub(super) fn rebuild_fts_index(&self) -> Result<()> {
        self.connection
            .execute(
                "INSERT INTO commands_fts(commands_fts) VALUES ('rebuild')",
                [],
            )
            .context("failed to rebuild full-text search index")?;
        Ok(())
    }

    pub fn audit_rows(&self, today: bool, limit: usize) -> Result<Vec<AuditRow>> {
        let mut sql = String::from(
            "SELECT id, event_type, raw_command, reason, username, hostname, created_at, prev_hash, entry_hash
             FROM audit_log",
        );
        let mut values = Vec::<Value>::new();
        if today {
            let start = crate::timestamp::today_start_utc().unwrap_or_else(|_| Utc::now());
            sql.push_str(" WHERE created_at >= ?");
            values.push(Value::Text(start.to_rfc3339()));
        }
        sql.push_str(" ORDER BY datetime(created_at) DESC, id DESC LIMIT ?");
        values.push(Value::Integer(limit.min(i64::MAX as usize) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), map_audit_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

}
