use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::Database;

pub const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/001_init.sql"),
    include_str!("../../migrations/002_sessions.sql"),
    include_str!("../../migrations/003_fts.sql"),
    include_str!("../../migrations/004_tags.sql"),
    include_str!("../../migrations/005_snippets.sql"),
    include_str!("../../migrations/006_audit_log.sql"),
    include_str!("../../migrations/007_vault.sql"),
    include_str!("../../migrations/008_enterprise.sql"),
    include_str!("../../migrations/009_performance_indexes.sql"),
    include_str!("../../migrations/010_fts_triggers.sql"),
    include_str!("../../migrations/011_dedupe_lookup_index.sql"),
];

pub const FTS_TRIGGER_SCHEMA_VERSION: i64 = 10;
pub const EXPECTED_SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;
const ENTERPRISE_MIGRATION_VERSION: i64 = 8;

pub fn schema_version(database: &Database) -> Result<i64> {
    database
        .connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .context("failed to read database schema version")
}

impl Database {
    pub(super) fn run_migrations(&self) -> Result<()> {
        let current_version: i64 =
            self.connection
                .pragma_query_value(None, "user_version", |row| row.get(0))?;

        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let target_version = (index + 1) as i64;
            if current_version < target_version {
                apply_migration(self, target_version, migration)?;
            }
        }

        Ok(())
    }

    pub(super) fn fts_sync_via_triggers(&self) -> Result<bool> {
        Ok(schema_version(self)? >= FTS_TRIGGER_SCHEMA_VERSION)
    }

    pub(super) fn ensure_command_exists(&self, command_id: i64) -> Result<()> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM commands WHERE id = ?1",
            params![command_id],
            |row| row.get(0),
        )?;
        if count == 0 {
            bail!("command id {command_id} does not exist");
        }
        Ok(())
    }
}

fn apply_migration(database: &Database, target_version: i64, migration: &str) -> Result<()> {
    database
        .connection
        .execute("BEGIN IMMEDIATE", [])
        .map_err(crate::errors::map_sqlite_error)
        .with_context(|| format!("failed to begin migration {target_version}"))?;

    let result = (|| -> Result<()> {
        if target_version == ENTERPRISE_MIGRATION_VERSION {
            apply_enterprise_migration(database)?;
        } else {
            database
                .connection
                .execute_batch(migration)
                .with_context(|| format!("failed to apply migration {target_version}"))?;
        }
        database
            .connection
            .pragma_update(None, "user_version", target_version)
            .with_context(|| format!("failed to set schema version {target_version}"))?;
        Ok(())
    })();

    match result {
        Ok(()) => database
            .connection
            .execute("COMMIT", [])
            .map(|_| ())
            .map_err(crate::errors::map_sqlite_error)
            .with_context(|| format!("failed to commit migration {target_version}")),
        Err(error) => {
            let _ = database.connection.execute("ROLLBACK", []);
            Err(error)
        }
    }
}

fn apply_enterprise_migration(database: &Database) -> Result<()> {
    let connection = &database.connection;
    add_column_if_missing(
        connection,
        "audit_log",
        "prev_hash",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        connection,
        "audit_log",
        "entry_hash",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        connection,
        "commands",
        "is_legal_hold",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(connection, "commands", "environment_tier", "TEXT")?;

    connection.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS legal_holds (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    label       TEXT NOT NULL,
    session_id  TEXT,
    command_id  INTEGER,
    tag         TEXT,
    git_repo    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    reason      TEXT
);

CREATE INDEX IF NOT EXISTS idx_legal_holds_session ON legal_holds(session_id);
CREATE INDEX IF NOT EXISTS idx_legal_holds_command ON legal_holds(command_id);

CREATE TABLE IF NOT EXISTS runbooks (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT NOT NULL UNIQUE,
    description       TEXT,
    source_session_id TEXT,
    created_at        TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at        TEXT
);

CREATE TABLE IF NOT EXISTS runbook_steps (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    runbook_id  INTEGER NOT NULL REFERENCES runbooks(id) ON DELETE CASCADE,
    step_order  INTEGER NOT NULL,
    command     TEXT NOT NULL,
    cwd         TEXT,
    notes       TEXT
);

CREATE INDEX IF NOT EXISTS idx_runbook_steps_runbook ON runbook_steps(runbook_id);

CREATE TABLE IF NOT EXISTS purge_audit (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    action      TEXT NOT NULL,
    target      TEXT,
    count       INTEGER NOT NULL DEFAULT 0,
    username    TEXT,
    hostname    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
"#,
    )?;
    Ok(())
}

fn add_column_if_missing(
    connection: &rusqlite::Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    if column_exists(connection, table, column)? {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    connection
        .execute(&sql, [])
        .with_context(|| format!("failed to add column {column} to {table}"))?;
    Ok(())
}

fn column_exists(connection: &rusqlite::Connection, table: &str, column: &str) -> Result<bool> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&sql)
        .with_context(|| format!("failed to inspect columns for {table}"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names.iter().any(|name| name == column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_migration_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("history.db");
        let database = Database::open_path(path).expect("open");

        apply_enterprise_migration(&database).expect("first apply");
        apply_enterprise_migration(&database).expect("second apply");
        assert!(column_exists(&database.connection, "audit_log", "prev_hash").expect("column"));
    }

    #[test]
    fn dedupe_lookup_index_exists_after_migration() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let database =
            Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

        let index_count: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_commands_dedupe_lookup'",
                [],
                |row| row.get(0),
            )
            .expect("index query");
        assert_eq!(index_count, 1);
        assert_eq!(
            schema_version(&database).expect("version"),
            EXPECTED_SCHEMA_VERSION
        );
    }
}
