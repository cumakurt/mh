use anyhow::{Context, Result};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::Database;
use super::helpers::*;
use crate::models::*;

impl Database {
    pub(super) fn count_with_where(
        &self,
        where_sql: &str,
        values: &[Value],
        expression: &str,
    ) -> Result<i64> {
        let sql = format!("SELECT COALESCE({expression}, 0) FROM commands {where_sql}");
        self.connection
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
            .with_context(|| format!("failed to evaluate statistic: {expression}"))
    }

    pub(super) fn optional_i64_with_where(
        &self,
        where_sql: &str,
        values: &[Value],
        expression: &str,
        extra_condition: &str,
    ) -> Result<Option<i64>> {
        let sql = format!(
            "SELECT {expression} FROM commands {}",
            combine_where(where_sql, extra_condition)
        );
        self.connection
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
            .with_context(|| format!("failed to evaluate statistic: {expression}"))
    }

    pub(super) fn optional_f64_with_where(
        &self,
        where_sql: &str,
        values: &[Value],
        expression: &str,
        extra_condition: &str,
    ) -> Result<Option<f64>> {
        let sql = format!(
            "SELECT {expression} FROM commands {}",
            combine_where(where_sql, extra_condition)
        );
        self.connection
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))
            .with_context(|| format!("failed to evaluate statistic: {expression}"))
    }

    pub(super) fn top_entries(
        &self,
        column: &str,
        where_sql: &str,
        values: &[Value],
        limit: usize,
    ) -> Result<Vec<StatEntry>> {
        let sql = format!(
            "SELECT {column}, COUNT(*)
             FROM commands {}
             GROUP BY {column}
             ORDER BY COUNT(*) DESC, {column} ASC
             LIMIT ?",
            combine_where(
                where_sql,
                &format!("{column} IS NOT NULL AND {column} != ''")
            )
        );
        let mut query_values = values.to_vec();
        query_values.push(Value::Integer(limit as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let entries = statement
            .query_map(params_from_iter(query_values.iter()), |row| {
                Ok(StatEntry {
                    label: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn stats_summary(&self, period: StatsPeriod, top: usize) -> Result<StatsSummary> {
        let top = top.max(1);
        let (where_sql, values) = period_where(period);

        let total_commands = self.count_with_where(&where_sql, &values, "COUNT(*)")?;
        let successful_commands = self.count_with_where(
            &where_sql,
            &values,
            "SUM(CASE WHEN exit_code = 0 THEN 1 ELSE 0 END)",
        )?;
        let failed_commands = self.count_with_where(
            &where_sql,
            &values,
            "SUM(CASE WHEN exit_code IS NOT NULL AND exit_code != 0 THEN 1 ELSE 0 END)",
        )?;
        let average_duration_ms = self.optional_f64_with_where(
            &where_sql,
            &values,
            "AVG(duration_ms)",
            "duration_ms IS NOT NULL",
        )?;
        let longest_duration_ms = self.optional_i64_with_where(
            &where_sql,
            &values,
            "MAX(duration_ms)",
            "duration_ms IS NOT NULL",
        )?;

        Ok(StatsSummary {
            period,
            total_commands,
            successful_commands,
            failed_commands,
            average_duration_ms,
            longest_duration_ms,
            top_commands: self.top_entries("command", &where_sql, &values, top)?,
            top_directories: self.top_entries("cwd", &where_sql, &values, top)?,
            error_prone_commands: self.top_error_prone_commands(&where_sql, &values, top)?,
            shell_counts: self.top_entries("shell", &where_sql, &values, top)?,
            category_counts: self.top_entries("category", &where_sql, &values, top)?,
            peak_hour: self.peak_hour(period)?,
        })
    }

    fn top_error_prone_commands(
        &self,
        where_sql: &str,
        values: &[Value],
        limit: usize,
    ) -> Result<Vec<StatEntry>> {
        let sql = format!(
            "SELECT command,
                    SUM(CASE WHEN exit_code IS NOT NULL AND exit_code != 0 THEN 1 ELSE 0 END) AS fail_count
             FROM commands {where_sql}
             GROUP BY command
             HAVING fail_count > 0
             ORDER BY fail_count DESC, command ASC
             LIMIT ?"
        );
        let mut query_values = values.to_vec();
        query_values.push(Value::Integer(limit as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let entries = statement
            .query_map(params_from_iter(query_values.iter()), |row| {
                Ok(StatEntry {
                    label: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    fn peak_hour(&self, period: StatsPeriod) -> Result<Option<String>> {
        let heatmap = self.hourly_activity(period)?;
        Ok(heatmap
            .into_iter()
            .max_by_key(|entry| entry.count)
            .map(|entry| format!("{}:00", entry.label)))
    }

    pub fn enforce_max_entries(&self, max_entries: usize, auto_vacuum: bool) -> Result<usize> {
        let count = self.count_commands()?;
        let max_entries = max_entries as i64;
        if count <= max_entries {
            return Ok(0);
        }

        let excess = count - max_entries;
        self.delete_oldest_commands(excess, auto_vacuum)
    }

    /// Runs retention when due; avoids a full `COUNT(*)` on every insert.
    pub fn maybe_enforce_max_entries(
        &self,
        max_entries: usize,
        auto_vacuum: bool,
        last_insert_id: i64,
    ) -> Result<usize> {
        if max_entries == 0 {
            return Ok(0);
        }

        const CHECK_INTERVAL: i64 = 250;
        let near_limit = max_entries < 10_000;
        let interval_due = last_insert_id > 0 && last_insert_id % CHECK_INTERVAL == 0;
        let possibly_over = last_insert_id as usize > max_entries;
        if !near_limit && !interval_due && !possibly_over {
            return Ok(0);
        }

        self.enforce_max_entries(max_entries, auto_vacuum)
    }

    fn delete_oldest_commands(&self, excess: i64, auto_vacuum: bool) -> Result<usize> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM commands
             WHERE is_pinned = 0 AND is_legal_hold = 0
             ORDER BY started_at ASC, id ASC
             LIMIT ?",
        )?;
        let ids = statement
            .query_map(rusqlite::params![excess], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        let deleted = self.delete_command_ids(&ids)?;

        if auto_vacuum && deleted > 0 {
            let _ = self.connection.execute("PRAGMA incremental_vacuum", []);
        }

        Ok(deleted)
    }

    pub fn hourly_activity(&self, period: StatsPeriod) -> Result<Vec<StatEntry>> {
        let (where_sql, values) = period_where(period);
        let sql = format!(
            "SELECT strftime('%H', started_at) AS hour, COUNT(*)
             FROM commands {where_sql}
             GROUP BY hour
             ORDER BY hour ASC"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let hour: Option<String> = row.get(0)?;
                Ok(StatEntry {
                    label: hour.unwrap_or_else(|| "--".to_string()),
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
