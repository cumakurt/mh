use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use super::Database;
use super::helpers::*;
use crate::audit_chain;
use crate::models::*;

impl Database {
    pub fn insert_audit_log(
        &self,
        event_type: &str,
        raw_command: &str,
        reason: &str,
        username: Option<&str>,
        hostname: Option<&str>,
    ) -> Result<AuditRow> {
        self.connection
            .execute("BEGIN IMMEDIATE", [])
            .context("failed to begin audit log transaction")?;

        let result = (|| -> Result<AuditRow> {
            let prev_hash: String = self
                .connection
                .query_row(
                    "SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or_default();

            let created_at = Utc::now().to_rfc3339();
            let entry_hash = audit_chain::compute_entry_hash(
                &prev_hash,
                event_type,
                Some(raw_command),
                Some(reason),
                username,
                hostname,
                &created_at,
            );

            self.connection.execute(
                "INSERT INTO audit_log(
                    event_type, raw_command, reason, username, hostname, created_at, prev_hash, entry_hash
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    event_type,
                    raw_command,
                    reason,
                    username,
                    hostname,
                    created_at,
                    prev_hash,
                    entry_hash
                ],
            )?;

            let id = self.connection.last_insert_rowid();
            Ok(AuditRow {
                id,
                event_type: event_type.to_string(),
                raw_command: Some(raw_command.to_string()),
                reason: Some(reason.to_string()),
                username: username.map(ToOwned::to_owned),
                hostname: hostname.map(ToOwned::to_owned),
                created_at: created_at.clone(),
                prev_hash: Some(prev_hash.clone()),
                entry_hash: Some(entry_hash.clone()),
            })
        })();

        match result {
            Ok(row) => {
                self.connection
                    .execute("COMMIT", [])
                    .context("failed to commit audit log transaction")?;
                Ok(row)
            }
            Err(error) => {
                let _ = self.connection.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }

    pub fn last_audit_hash(&self) -> Result<String> {
        let hash: Option<String> = self
            .connection
            .query_row(
                "SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(hash.unwrap_or_default())
    }

    pub fn audit_rows_chronological(&self, limit: usize) -> Result<Vec<AuditRow>> {
        let mut statement = self.connection.prepare(
            "SELECT id, event_type, raw_command, reason, username, hostname, created_at, prev_hash, entry_hash
             FROM audit_log ORDER BY id ASC LIMIT ?",
        )?;
        let rows = statement
            .query_map(params![limit.min(i64::MAX as usize) as i64], map_audit_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn verify_audit_chain(&self) -> Result<()> {
        let rows = self.audit_rows_chronological(usize::MAX)?;
        audit_chain::verify_chain(&rows)
    }

    pub fn rebuild_audit_chain(&self) -> Result<usize> {
        let rows = self.audit_rows_chronological(usize::MAX)?;
        self.connection
            .execute("BEGIN IMMEDIATE", [])
            .context("failed to begin audit chain rebuild transaction")?;

        let result = (|| -> Result<usize> {
            let mut prev_hash = String::new();
            let mut updated = 0usize;
            for row in rows {
                let entry_hash = audit_chain::compute_entry_hash(
                    &prev_hash,
                    &row.event_type,
                    row.raw_command.as_deref(),
                    row.reason.as_deref(),
                    row.username.as_deref(),
                    row.hostname.as_deref(),
                    &row.created_at,
                );
                self.connection.execute(
                    "UPDATE audit_log SET prev_hash = ?1, entry_hash = ?2 WHERE id = ?3",
                    params![prev_hash, entry_hash, row.id],
                )?;
                prev_hash = entry_hash;
                updated += 1;
            }
            Ok(updated)
        })();

        match result {
            Ok(updated) => {
                self.connection
                    .execute("COMMIT", [])
                    .context("failed to commit audit chain rebuild transaction")?;
                Ok(updated)
            }
            Err(error) => {
                let _ = self.connection.execute("ROLLBACK", []);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn rebuild_audit_chain_seals_legacy_rows() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut config = AppConfig::default();
        config.database.path = temp_dir
            .path()
            .join("history.db")
            .to_string_lossy()
            .to_string();
        let database = Database::open(&config).expect("database should open");
        database
            .connection
            .execute(
                "INSERT INTO audit_log(event_type, raw_command, reason, created_at, prev_hash, entry_hash)
                 VALUES ('legacy', 'cmd', 'test', '2026-01-01T00:00:00Z', '', '')",
                [],
            )
            .expect("legacy insert");

        let updated = database.rebuild_audit_chain().expect("rebuild");
        assert_eq!(updated, 1);
        database.verify_audit_chain().expect("chain should verify");
    }
}
