use anyhow::{Context, Result};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use regex::Regex;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use super::Database;
use super::helpers::*;
use crate::models::*;

impl Database {
    pub fn search_commands(&self, filters: &SearchFilters) -> Result<Vec<CommandRow>> {
        if can_use_recent_fast_path(filters) {
            return self.search_recent_fast(filters);
        }

        let mut sql = format!(
            "{COMMAND_ROW_SELECT_JOIN} FROM commands c \
             LEFT JOIN tags t ON t.command_id = c.id WHERE 1 = 1"
        );
        let mut values = Vec::<Value>::new();

        if let Some(query) = filters.query.as_deref().filter(|query| !query.is_empty())
            && !filters.regex
            && !filters.fuzzy
            && !filters.fts
        {
            sql.push_str(" AND c.command LIKE ? ESCAPE '\\'");
            values.push(Value::Text(format!("%{}%", escape_like(query))));
        }

        if let Some(query) = filters.query.as_deref().filter(|query| !query.is_empty())
            && filters.fts
        {
            let normalized_query = fts_query(query);
            if normalized_query.is_empty() {
                return Ok(Vec::new());
            }
            sql.push_str(
                " AND c.id IN (SELECT rowid FROM commands_fts WHERE commands_fts MATCH ?)",
            );
            values.push(Value::Text(normalized_query));
        }

        if let Some(query) = filters.query.as_deref().filter(|query| !query.is_empty())
            && filters.fuzzy
        {
            let char_count = query.chars().count();
            let prefix_len = if char_count <= 3 {
                1
            } else {
                2.min(char_count)
            };
            let prefix: String = query.chars().take(prefix_len).collect();
            if !prefix.is_empty() {
                sql.push_str(" AND c.command LIKE ? ESCAPE '\\'");
                values.push(Value::Text(format!("{}%", escape_like(&prefix))));
            }
        }

        if let Some(cwd) = filters.cwd.as_deref().filter(|cwd| !cwd.is_empty()) {
            sql.push_str(" AND c.cwd LIKE ? ESCAPE '\\'");
            values.push(Value::Text(format!("%{}%", escape_like(cwd))));
        }

        if filters.failed {
            sql.push_str(" AND c.exit_code IS NOT NULL AND c.exit_code != 0");
        }

        if filters.success {
            sql.push_str(" AND c.exit_code = 0");
        }

        if let Some(user) = filters.user.as_deref().filter(|user| !user.is_empty()) {
            sql.push_str(" AND c.username = ?");
            values.push(Value::Text(user.to_string()));
        }

        if let Some(shell) = filters.shell.as_deref().filter(|shell| !shell.is_empty()) {
            sql.push_str(" AND c.shell = ?");
            values.push(Value::Text(shell.to_string()));
        }

        if let Some(after) = filters.after.as_deref().filter(|after| !after.is_empty()) {
            sql.push_str(" AND c.started_at >= ?");
            values.push(Value::Text(normalize_date_bound(after, true)?));
        }

        if let Some(before) = filters
            .before
            .as_deref()
            .filter(|before| !before.is_empty())
        {
            sql.push_str(" AND c.started_at <= ?");
            values.push(Value::Text(normalize_date_bound(before, false)?));
        }

        if let Some(session_id) = filters.session_id.as_deref().filter(|id| !id.is_empty()) {
            sql.push_str(" AND c.session_id = ?");
            values.push(Value::Text(session_id.to_string()));
        }

        if let Some(tag) = filters.tag.as_deref().filter(|tag| !tag.is_empty()) {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM tags tf WHERE tf.command_id = c.id AND tf.tag = ?)",
            );
            values.push(Value::Text(tag.to_string()));
        }

        if let Some(category) = filters
            .category
            .as_deref()
            .filter(|category| !category.is_empty())
        {
            sql.push_str(" AND c.category = ?");
            values.push(Value::Text(category.to_string()));
        }

        if filters.pinned {
            sql.push_str(" AND c.is_pinned = 1");
        }

        if let Some(duration_gt) = filters.duration_gt {
            sql.push_str(" AND c.duration_ms > ?");
            values.push(Value::Integer(duration_gt));
        }

        if let Some(duration_lt) = filters.duration_lt {
            sql.push_str(" AND c.duration_ms < ?");
            values.push(Value::Integer(duration_lt));
        }

        if let Some(hostname) = filters
            .hostname
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND c.hostname = ?");
            values.push(Value::Text(hostname.to_string()));
        }

        if filters.ssh {
            sql.push_str(" AND c.is_ssh = 1");
        }

        if filters.root {
            sql.push_str(" AND c.is_root = 1");
        }

        if let Some(git_repo) = filters
            .git_repo
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND c.git_repo = ?");
            values.push(Value::Text(git_repo.to_string()));
        }

        if let Some(git_branch) = filters
            .git_branch
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND c.git_branch = ?");
            values.push(Value::Text(git_branch.to_string()));
        }

        if let Some(git_commit) = filters
            .git_commit
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND c.git_commit = ?");
            values.push(Value::Text(git_commit.to_string()));
        }

        if let Some(environment) = filters
            .environment
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            sql.push_str(" AND c.environment_tier = ?");
            values.push(Value::Text(environment.to_string()));
        }

        sql.push_str(" GROUP BY c.id ORDER BY c.started_at DESC, c.id DESC LIMIT ?");
        const MAX_OVER_FETCH: usize = 2_000;
        let sql_limit = if filters.regex || filters.fuzzy {
            filters
                .limit
                .saturating_mul(20)
                .max(filters.limit)
                .min(MAX_OVER_FETCH)
        } else {
            filters.limit
        };
        values.push(Value::Integer(sql_limit.min(i64::MAX as usize) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), map_command_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if filters.regex
            && let Some(query) = filters.query.as_deref().filter(|query| !query.is_empty())
        {
            let regex = Regex::new(query)
                .with_context(|| format!("invalid search regex pattern: {query}"))?;
            return Ok(rows
                .into_iter()
                .filter(|row| regex.is_match(&row.command))
                .take(filters.limit)
                .collect());
        }

        if filters.fuzzy
            && let Some(query) = filters.query.as_deref().filter(|query| !query.is_empty())
        {
            let matcher = SkimMatcherV2::default();
            let mut scored = rows
                .into_iter()
                .filter_map(|row| {
                    let haystack = row_search_text(&row);
                    matcher
                        .fuzzy_match(&haystack, query)
                        .map(|score| (score, row))
                })
                .collect::<Vec<_>>();
            scored.sort_by(|left, right| {
                right
                    .0
                    .cmp(&left.0)
                    .then_with(|| right.1.id.cmp(&left.1.id))
            });
            return Ok(scored
                .into_iter()
                .map(|(_, row)| row)
                .take(filters.limit)
                .collect());
        }

        Ok(rows)
    }

    pub fn get_command(&self, command_id: i64) -> Result<CommandRow> {
        let sql = format!(
            "{COMMAND_ROW_SELECT_JOIN} FROM commands c \
             LEFT JOIN tags t ON t.command_id = c.id WHERE c.id = ?1 GROUP BY c.id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        statement
            .query_row(params![command_id], map_command_row)
            .with_context(|| format!("command id {command_id} does not exist"))
    }

    pub fn distinct_git_repos(&self, limit: usize) -> Result<Vec<StatEntry>> {
        self.top_entries("git_repo", "", &[], limit)
    }

    pub fn distinct_git_branches(
        &self,
        git_repo: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StatEntry>> {
        let (where_sql, values) = if let Some(repo) = git_repo.filter(|value| !value.is_empty()) {
            (
                "WHERE git_repo = ?".to_string(),
                vec![Value::Text(repo.to_string())],
            )
        } else {
            (String::new(), Vec::new())
        };
        self.top_entries("git_branch", &where_sql, &values, limit)
    }

    pub fn distinct_environments(&self, limit: usize) -> Result<Vec<StatEntry>> {
        self.top_entries("environment_tier", "", &[], limit)
    }

    pub fn delete_command_ids(&self, command_ids: &[i64]) -> Result<usize> {
        if command_ids.is_empty() {
            return Ok(0);
        }

        let fts_triggers = self.fts_sync_via_triggers()?;
        let tx = self
            .connection
            .unchecked_transaction()
            .context("failed to begin delete transaction")?;

        let mut deleted = 0usize;
        const CHUNK_SIZE: usize = 500;
        for chunk in command_ids.chunks(CHUNK_SIZE) {
            let placeholders = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(", ");
            let params: Vec<rusqlite::types::Value> = chunk
                .iter()
                .map(|id| rusqlite::types::Value::from(*id))
                .collect();

            if !fts_triggers {
                let sql = format!("DELETE FROM commands_fts WHERE rowid IN ({placeholders})");
                tx.execute(&sql, rusqlite::params_from_iter(params.iter()))?;
            }
            let tag_sql = format!("DELETE FROM tags WHERE command_id IN ({placeholders})");
            tx.execute(&tag_sql, rusqlite::params_from_iter(params.iter()))?;
            let cmd_sql = format!("DELETE FROM commands WHERE id IN ({placeholders})");
            deleted += tx.execute(&cmd_sql, rusqlite::params_from_iter(params.iter()))?;
        }

        tx.commit().context("failed to commit delete transaction")?;
        Ok(deleted)
    }
}

fn can_use_recent_fast_path(filters: &SearchFilters) -> bool {
    !filters.regex
        && !filters.fuzzy
        && !filters.fts
        && filters
            .query
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
        && filters.tag.is_none()
}

impl Database {
    fn search_recent_fast(&self, filters: &SearchFilters) -> Result<Vec<CommandRow>> {
        let mut id_sql = String::from("SELECT c.id FROM commands c WHERE 1 = 1");
        let mut values = Vec::<Value>::new();
        append_recent_filters(&mut id_sql, &mut values, filters)?;
        id_sql.push_str(" ORDER BY c.started_at DESC, c.id DESC LIMIT ?");
        values.push(Value::Integer(filters.limit.min(i64::MAX as usize) as i64));

        let mut id_statement = self.connection.prepare(&id_sql)?;
        let ids = id_statement
            .query_map(params_from_iter(values.iter()), |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "{COMMAND_ROW_SELECT_JOIN} FROM commands c \
             LEFT JOIN tags t ON t.command_id = c.id \
             WHERE c.id IN ({placeholders}) \
             GROUP BY c.id ORDER BY c.started_at DESC, c.id DESC"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let id_values = ids.iter().map(|id| Value::Integer(*id)).collect::<Vec<_>>();
        statement
            .query_map(params_from_iter(id_values.iter()), map_command_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to load recent commands")
    }
}

fn append_recent_filters(
    sql: &mut String,
    values: &mut Vec<Value>,
    filters: &SearchFilters,
) -> Result<()> {
    if let Some(cwd) = filters.cwd.as_deref().filter(|cwd| !cwd.is_empty()) {
        sql.push_str(" AND c.cwd LIKE ? ESCAPE '\\'");
        values.push(Value::Text(format!("%{}%", escape_like(cwd))));
    }

    if filters.failed {
        sql.push_str(" AND c.exit_code IS NOT NULL AND c.exit_code != 0");
    }

    if filters.success {
        sql.push_str(" AND c.exit_code = 0");
    }

    if let Some(user) = filters.user.as_deref().filter(|user| !user.is_empty()) {
        sql.push_str(" AND c.username = ?");
        values.push(Value::Text(user.to_string()));
    }

    if let Some(shell) = filters.shell.as_deref().filter(|shell| !shell.is_empty()) {
        sql.push_str(" AND c.shell = ?");
        values.push(Value::Text(shell.to_string()));
    }

    if let Some(after) = filters.after.as_deref().filter(|after| !after.is_empty()) {
        sql.push_str(" AND c.started_at >= ?");
        values.push(Value::Text(normalize_date_bound(after, true)?));
    }

    if let Some(before) = filters
        .before
        .as_deref()
        .filter(|before| !before.is_empty())
    {
        sql.push_str(" AND c.started_at <= ?");
        values.push(Value::Text(normalize_date_bound(before, false)?));
    }

    if let Some(session_id) = filters.session_id.as_deref().filter(|id| !id.is_empty()) {
        sql.push_str(" AND c.session_id = ?");
        values.push(Value::Text(session_id.to_string()));
    }

    if let Some(category) = filters
        .category
        .as_deref()
        .filter(|category| !category.is_empty())
    {
        sql.push_str(" AND c.category = ?");
        values.push(Value::Text(category.to_string()));
    }

    if filters.pinned {
        sql.push_str(" AND c.is_pinned = 1");
    }

    if let Some(duration_gt) = filters.duration_gt {
        sql.push_str(" AND c.duration_ms > ?");
        values.push(Value::Integer(duration_gt));
    }

    if let Some(duration_lt) = filters.duration_lt {
        sql.push_str(" AND c.duration_ms < ?");
        values.push(Value::Integer(duration_lt));
    }

    if let Some(hostname) = filters
        .hostname
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        sql.push_str(" AND c.hostname = ?");
        values.push(Value::Text(hostname.to_string()));
    }

    if filters.ssh {
        sql.push_str(" AND c.is_ssh = 1");
    }

    if filters.root {
        sql.push_str(" AND c.is_root = 1");
    }

    if let Some(git_repo) = filters
        .git_repo
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        sql.push_str(" AND c.git_repo = ?");
        values.push(Value::Text(git_repo.to_string()));
    }

    if let Some(git_branch) = filters
        .git_branch
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        sql.push_str(" AND c.git_branch = ?");
        values.push(Value::Text(git_branch.to_string()));
    }

    if let Some(git_commit) = filters
        .git_commit
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        sql.push_str(" AND c.git_commit = ?");
        values.push(Value::Text(git_commit.to_string()));
    }

    if let Some(environment) = filters
        .environment
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        sql.push_str(" AND c.environment_tier = ?");
        values.push(Value::Text(environment.to_string()));
    }

    Ok(())
}
