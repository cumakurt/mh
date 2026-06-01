mod audit;
mod commands;
mod enterprise;
mod helpers;
mod history;
mod migrations;
mod search;
mod snippets;
mod stats;
mod tags;
mod vault;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::{self, AppConfig};
use crate::errors::MhError;
pub use migrations::EXPECTED_SCHEMA_VERSION;

pub struct Database {
    pub(crate) connection: Connection,
    pub(crate) path: PathBuf,
}

const DEFAULT_BUSY_TIMEOUT_MS: i64 = 5_000;
const RECORD_BUSY_TIMEOUT_MS: i64 = 20;

impl Database {
    pub fn open(config: &AppConfig) -> Result<Self> {
        let path = config.database_path()?;
        let database = Self::open_path(path)?;
        database.maybe_incremental_vacuum(config);
        Ok(database)
    }

    pub fn open_for_record(config: &AppConfig) -> Result<Self> {
        let path = config.database_path()?;
        let database = Self::open_path_with_busy_timeout(path, RECORD_BUSY_TIMEOUT_MS)?;
        database.maybe_incremental_vacuum(config);
        Ok(database)
    }

    pub fn open_path(path: PathBuf) -> Result<Self> {
        Self::open_path_with_busy_timeout(path, DEFAULT_BUSY_TIMEOUT_MS)
    }

    fn open_path_with_busy_timeout(path: PathBuf, busy_timeout_ms: i64) -> Result<Self> {
        if path.exists() && path.is_dir() {
            anyhow::bail!(
                "database path {} is a directory, not a file",
                path.display()
            );
        }
        if let Some(parent) = path.parent() {
            config::ensure_secure_data_directory(parent, "database parent")?;
        }
        if path.exists() {
            config::ensure_not_symlink(&path)?;
        }

        let connection = Connection::open(&path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "busy_timeout", busy_timeout_ms)?;
        connection.pragma_update(None, "cache_size", -64_000)?;
        connection.pragma_update(None, "wal_autocheckpoint", 1_000)?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        config::restrict_file_permissions(&path)?;
        helpers::restrict_wal_sidecar_permissions(&path)?;

        let database = Self { connection, path };
        database.run_migrations()?;
        database.verify_schema_version()?;
        Ok(database)
    }

    fn maybe_incremental_vacuum(&self, config: &AppConfig) {
        if config.database.auto_vacuum {
            let _ = self.connection.execute("PRAGMA incremental_vacuum", []);
        }
    }

    fn verify_schema_version(&self) -> Result<()> {
        let version = self.schema_version()?;
        if version > EXPECTED_SCHEMA_VERSION {
            return Err(MhError::Config(format!(
                "database schema {version} is newer than this mh build ({EXPECTED_SCHEMA_VERSION}); upgrade the mh binary"
            ))
            .into());
        }
        if version < EXPECTED_SCHEMA_VERSION {
            return Err(MhError::SchemaOutdated {
                found: version,
                expected: EXPECTED_SCHEMA_VERSION,
            }
            .into());
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn checkpoint_wal(&self) -> Result<()> {
        self.connection
            .query_row("PRAGMA wal_checkpoint(FULL)", [], |_row| Ok(()))
            .context("failed to checkpoint WAL")?;
        Ok(())
    }

    pub fn count_commands(&self) -> Result<i64> {
        self.connection
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .context("failed to count commands")
    }

    pub fn integrity_check(&self) -> Result<String> {
        self.connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("failed to run database integrity check")
    }

    pub fn schema_version(&self) -> Result<i64> {
        migrations::schema_version(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::MhError;

    #[test]
    fn verify_schema_version_rejects_outdated_version() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let database =
            Database::open_path(temp_dir.path().join("history.db")).expect("database should open");
        database
            .connection
            .pragma_update(None, "user_version", 1)
            .expect("downgrade schema version");

        match database.verify_schema_version() {
            Err(error) => assert!(
                error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<MhError>(),
                        Some(MhError::SchemaOutdated {
                            found: 1,
                            expected
                        }) if *expected == EXPECTED_SCHEMA_VERSION
                    )
                }),
                "expected SchemaOutdated, got: {error:#}"
            ),
            Ok(()) => panic!("outdated schema should be rejected"),
        }
    }
}
