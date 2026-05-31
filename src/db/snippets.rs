use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::Database;
use super::helpers::*;
use crate::models::*;

impl Database {
    pub fn save_snippet(
        &self,
        name: &str,
        command: &str,
        description: Option<&str>,
        tags: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO snippets(name, command, description, tags, updated_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
             ON CONFLICT(name) DO UPDATE SET
                command = excluded.command,
                description = excluded.description,
                tags = excluded.tags,
                updated_at = CURRENT_TIMESTAMP",
            params![name, command, description, tags],
        )?;
        Ok(())
    }

    pub fn list_snippets(&self) -> Result<Vec<SnippetRow>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, command, description, tags, use_count, created_at, updated_at
             FROM snippets
             ORDER BY name ASC",
        )?;
        let rows = statement
            .query_map([], map_snippet_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_snippet(&self, name: &str) -> Result<SnippetRow> {
        self.connection
            .query_row(
                "SELECT id, name, command, description, tags, use_count, created_at, updated_at
                 FROM snippets
                 WHERE name = ?1",
                params![name],
                map_snippet_row,
            )
            .with_context(|| format!("snippet not found: {name}"))
    }

    pub fn delete_snippet(&self, name: &str) -> Result<usize> {
        Ok(self
            .connection
            .execute("DELETE FROM snippets WHERE name = ?1", params![name])?)
    }

    pub fn increment_snippet_use(&self, name: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE snippets SET use_count = use_count + 1, updated_at = CURRENT_TIMESTAMP WHERE name = ?1",
            params![name],
        )?;
        Ok(())
    }

    pub fn distinct_commands_by_column(&self, column: &str, value: &str) -> Result<Vec<String>> {
        let column = match column {
            "session_id" => "session_id",
            "hostname" => "hostname",
            _ => bail!("unsupported diff column: {column}"),
        };
        let sql = format!(
            "SELECT DISTINCT command FROM commands WHERE {column} = ?1 ORDER BY command ASC"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement
            .query_map(params![value], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn distinct_commands_between(&self, after: &str, before: &str) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT command FROM commands
             WHERE started_at >= ?1 AND started_at <= ?2
             ORDER BY command ASC",
        )?;
        let rows = statement
            .query_map(params![after, before], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }


}
