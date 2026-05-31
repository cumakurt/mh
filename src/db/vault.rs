use anyhow::{Context, Result};
use rusqlite::params;

use super::Database;
use crate::models::*;

impl Database {
    pub fn add_vault_entry(
        &self,
        encrypted_data: &[u8],
        nonce: &[u8],
        label: Option<&str>,
    ) -> Result<i64> {
        self.connection.execute(
            "INSERT INTO vault(encrypted_data, nonce, label) VALUES (?1, ?2, ?3)",
            params![encrypted_data, nonce, label],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn list_vault_entries(&self) -> Result<Vec<VaultRow>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, label, created_at FROM vault ORDER BY id DESC")?;
        let rows = statement
            .query_map([], |row| {
                Ok(VaultRow {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_vault_entry(&self, id: i64) -> Result<EncryptedVaultRow> {
        self.connection
            .query_row(
                "SELECT id, encrypted_data, nonce, label, created_at FROM vault WHERE id = ?1",
                params![id],
                |row| {
                    Ok(EncryptedVaultRow {
                        id: row.get(0)?,
                        encrypted_data: row.get(1)?,
                        nonce: row.get(2)?,
                        label: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .with_context(|| format!("vault entry id {id} does not exist"))
    }

    pub fn delete_vault_entry(&self, id: i64) -> Result<usize> {
        Ok(self
            .connection
            .execute("DELETE FROM vault WHERE id = ?1", params![id])?)
    }

}
