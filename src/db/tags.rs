use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use rusqlite::params;

use super::Database;
use super::helpers::*;
use crate::models::*;

impl Database {
    pub fn has_recent_duplicate(
        &self,
        command: &str,
        cwd: Option<&str>,
        window_seconds: i64,
    ) -> Result<bool> {
        let cutoff = Utc::now() - Duration::seconds(window_seconds.max(0));
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*)
             FROM commands
             WHERE command = ?1
               AND COALESCE(cwd, '') = COALESCE(?2, '')
               AND started_at >= ?3",
            params![command, cwd, cutoff.to_rfc3339()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn command_hash_exists(&self, command_hash: &str) -> Result<bool> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM commands WHERE command_hash = ?1",
            params![command_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn latest_command_ids(&self, limit: usize) -> Result<Vec<i64>> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM commands ORDER BY started_at DESC, id DESC LIMIT ?")?;
        let rows = statement
            .query_map(params![limit as i64], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_tags(&self, command_ids: &[i64], tags: &[String]) -> Result<usize> {
        let tags = normalize_tags(tags);
        if command_ids.is_empty() {
            bail!("at least one command id is required");
        }
        if tags.is_empty() {
            bail!("at least one tag is required");
        }

        let tx = self
            .connection
            .unchecked_transaction()
            .context("failed to begin tag transaction")?;

        let mut inserted = 0usize;
        for command_id in command_ids {
            self.ensure_command_exists(*command_id)?;
            for tag in &tags {
                inserted += tx.execute(
                    "INSERT OR IGNORE INTO tags(command_id, tag) VALUES (?1, ?2)",
                    params![command_id, tag],
                )?;
            }
        }

        tx.commit().context("failed to commit tag transaction")?;
        Ok(inserted)
    }

    pub fn remove_tags(&self, command_id: i64, tags: &[String]) -> Result<usize> {
        self.ensure_command_exists(command_id)?;
        let tags = normalize_tags(tags);
        if tags.is_empty() {
            bail!("at least one tag is required");
        }

        let mut removed = 0;
        for tag in tags {
            removed += self.connection.execute(
                "DELETE FROM tags WHERE command_id = ?1 AND tag = ?2",
                params![command_id, tag],
            )?;
        }

        Ok(removed)
    }

    pub fn list_tags(&self) -> Result<Vec<TagSummary>> {
        let mut statement = self.connection.prepare(
            "SELECT tag, COUNT(*)
             FROM tags
             GROUP BY tag
             ORDER BY COUNT(*) DESC, tag ASC",
        )?;
        let tags = statement
            .query_map([], |row| {
                Ok(TagSummary {
                    tag: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    pub fn set_pinned(&self, command_ids: &[i64], pinned: bool) -> Result<usize> {
        if command_ids.is_empty() {
            bail!("at least one command id is required");
        }

        let mut updated = 0;
        for command_id in command_ids {
            self.ensure_command_exists(*command_id)?;
            updated += self.connection.execute(
                "UPDATE commands SET is_pinned = ?1 WHERE id = ?2",
                params![pinned as i32, command_id],
            )?;
        }

        Ok(updated)
    }
}
