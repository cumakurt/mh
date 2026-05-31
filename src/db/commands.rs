use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Utc};
use rusqlite::params;

use super::Database;
use super::helpers::{restrict_wal_sidecar_permissions, sqlite};
use crate::models::*;

impl Database {
    pub fn insert_command(&self, record: &CommandRecord) -> Result<i64> {
        self.insert_command_inner(record, None)?
            .ok_or_else(|| anyhow!("command insert unexpectedly skipped"))
    }

    /// Inserts unless an identical command exists in the same cwd within the dedupe window.
    pub fn insert_command_unless_recent_duplicate(
        &self,
        record: &CommandRecord,
        window_seconds: i64,
    ) -> Result<Option<i64>> {
        self.insert_command_inner(record, Some(window_seconds))
    }

    fn insert_command_inner(
        &self,
        record: &CommandRecord,
        dedupe_window_seconds: Option<i64>,
    ) -> Result<Option<i64>> {
        let begin = if dedupe_window_seconds.is_some() {
            "BEGIN IMMEDIATE"
        } else {
            "BEGIN DEFERRED"
        };
        self.connection
            .execute(begin, [])
            .map_err(crate::errors::map_sqlite_error)
            .context("failed to begin command insert transaction")?;

        let result = (|| -> Result<Option<i64>> {
            if let Some(window_seconds) = dedupe_window_seconds {
                let cutoff = Utc::now() - Duration::seconds(window_seconds.max(0));
                let count: i64 = sqlite(self.connection.query_row(
                    "SELECT COUNT(*)
                     FROM commands
                     WHERE command = ?1
                       AND COALESCE(cwd, '') = COALESCE(?2, '')
                       AND started_at >= ?3",
                    params![record.command, record.cwd, cutoff.to_rfc3339()],
                    |row| row.get(0),
                ))?;
                if count > 0 {
                    return Ok(None);
                }
            }

            sqlite(self.connection.execute(
                "INSERT INTO commands (
                    command, command_hash, cwd, shell, username, hostname, exit_code, duration_ms,
                    started_at, finished_at, session_id, tty, is_ssh, is_root, git_repo, git_branch,
                    git_commit, category, env_context, is_pinned, is_masked, environment_tier
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
                )",
                params![
                    record.command,
                    record.command_hash,
                    record.cwd,
                    record.shell,
                    record.username,
                    record.hostname,
                    record.exit_code,
                    record.duration_ms,
                    record.started_at,
                    record.finished_at,
                    record.session_id,
                    record.tty,
                    record.is_ssh as i32,
                    record.is_root as i32,
                    record.git_repo,
                    record.git_branch,
                    record.git_commit,
                    record.category,
                    record.env_context,
                    record.is_pinned as i32,
                    record.is_masked as i32,
                    record.environment_tier,
                ],
            ))?;

            let id = self.connection.last_insert_rowid();
            if !self.fts_sync_via_triggers()? {
                sqlite(self.connection.execute(
                    "INSERT INTO commands_fts(rowid, command, cwd) VALUES (?1, ?2, ?3)",
                    params![id, record.command, record.cwd],
                ))?;
            }

            for tag in &record.tags {
                sqlite(self.connection.execute(
                    "INSERT OR IGNORE INTO tags(command_id, tag) VALUES (?1, ?2)",
                    params![id, tag],
                ))?;
            }

            Ok(Some(id))
        })();

        match result {
            Ok(Some(id)) => {
                self.connection
                    .execute("COMMIT", [])
                    .map_err(crate::errors::map_sqlite_error)
                    .context("failed to commit command insert transaction")?;
                restrict_wal_sidecar_permissions(&self.path)?;
                Ok(Some(id))
            }
            Ok(None) => {
                let _ = self.connection.execute("ROLLBACK", []);
                Ok(None)
            }
            Err(error) => {
                let _ = self.connection.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }

    pub fn exceeds_size_limit(&self, max_size_mb: u64) -> Result<bool> {
        if max_size_mb == 0 {
            return Ok(false);
        }
        let size_bytes = std::fs::metadata(&self.path)
            .with_context(|| format!("failed to stat database at {}", self.path.display()))?
            .len();
        Ok(size_bytes > max_size_mb.saturating_mul(1024 * 1024))
    }
}
