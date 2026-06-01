use std::path::Path;

use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::types::Value;

use crate::config;
use crate::models::{AuditRow, CommandRow, SnippetRow, StatsPeriod};

pub(crate) const COMMAND_ROW_SELECT_JOIN: &str =
    "SELECT c.id, c.command, c.cwd, c.shell, c.username, c.hostname, c.exit_code,
                    c.duration_ms, c.started_at, c.session_id, c.git_repo, c.git_branch, c.git_commit,
                    c.category, c.is_pinned, c.is_masked, c.environment_tier,
                    COALESCE(group_concat(t.tag, ','), '')";

pub(crate) fn map_audit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditRow> {
    Ok(AuditRow {
        id: row.get(0)?,
        event_type: row.get(1)?,
        raw_command: row.get(2)?,
        reason: row.get(3)?,
        username: row.get(4)?,
        hostname: row.get(5)?,
        created_at: row.get(6)?,
        prev_hash: row.get(7)?,
        entry_hash: row.get(8)?,
    })
}

pub(crate) fn map_command_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CommandRow> {
    let raw_tags: String = row.get(17)?;
    Ok(CommandRow {
        id: row.get(0)?,
        command: row.get(1)?,
        cwd: row.get(2)?,
        shell: row.get(3)?,
        username: row.get(4)?,
        hostname: row.get(5)?,
        exit_code: row.get(6)?,
        duration_ms: row.get(7)?,
        started_at: row.get(8)?,
        session_id: row.get(9)?,
        git_repo: row.get(10)?,
        git_branch: row.get(11)?,
        git_commit: row.get(12)?,
        category: row.get(13)?,
        tags: split_tags(&raw_tags),
        is_pinned: row.get::<_, i64>(14)? != 0,
        is_masked: row.get::<_, i64>(15)? != 0,
        environment_tier: row.get(16)?,
    })
}

pub(crate) fn map_snippet_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnippetRow> {
    Ok(SnippetRow {
        id: row.get(0)?,
        name: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        tags: row.get(4)?,
        use_count: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub(crate) fn row_search_text(row: &CommandRow) -> String {
    let mut text = row.command.clone();
    if let Some(cwd) = &row.cwd {
        text.push(' ');
        text.push_str(cwd);
    }
    if let Some(category) = &row.category {
        text.push(' ');
        text.push_str(category);
    }
    for tag in &row.tags {
        text.push(' ');
        text.push_str(tag);
    }
    text
}

pub(crate) fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter_map(sanitize_fts_token)
        .map(|token| format!("\"{token}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn sanitize_fts_token(token: &str) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    let mut sanitized = String::with_capacity(token.len());
    for ch in token.chars() {
        if matches!(ch, '"' | '*' | ':' | '(' | ')' | '^' | '-' | '+' | '~') {
            continue;
        }
        sanitized.push(ch);
    }
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized.replace('"', "\"\""))
    }
}

pub(crate) fn split_tags(raw_tags: &str) -> Vec<String> {
    raw_tags
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !normalized.iter().any(|existing| existing == tag) {
            normalized.push(tag.to_string());
        }
    }
    normalized
}

pub(crate) fn normalize_date_bound(value: &str, start: bool) -> anyhow::Result<String> {
    crate::timestamp::normalize_date_bound(value, start)
}

pub(crate) fn period_where(period: StatsPeriod) -> (String, Vec<Value>) {
    match period {
        StatsPeriod::All => (String::new(), Vec::new()),
        StatsPeriod::Today => {
            let start = crate::timestamp::today_start_utc().unwrap_or_else(|_| Utc::now());
            (
                "WHERE started_at >= ?".to_string(),
                vec![Value::Text(start.to_rfc3339())],
            )
        }
        StatsPeriod::Week => {
            let start = Utc::now() - Duration::days(7);
            (
                "WHERE started_at >= ?".to_string(),
                vec![Value::Text(start.to_rfc3339())],
            )
        }
        StatsPeriod::Month => {
            let start = Utc::now() - Duration::days(30);
            (
                "WHERE started_at >= ?".to_string(),
                vec![Value::Text(start.to_rfc3339())],
            )
        }
    }
}

pub(crate) fn restrict_wal_sidecar_permissions(db_path: &Path) -> Result<()> {
    let wal = db_path.with_extension("db-wal");
    let shm = db_path.with_extension("db-shm");
    for path in [wal, shm] {
        if path.exists() {
            config::restrict_file_permissions(&path)?;
        }
    }
    Ok(())
}

pub(crate) fn combine_where(where_sql: &str, extra_condition: &str) -> String {
    if where_sql.trim().is_empty() {
        format!("WHERE {extra_condition}")
    } else {
        format!("{where_sql} AND {extra_condition}")
    }
}

pub(crate) fn escape_like(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| {
            if matches!(ch, '%' | '_' | '\\') {
                vec!['\\', ch]
            } else {
                vec![ch]
            }
        })
        .collect()
}

pub(crate) fn sqlite<T>(result: rusqlite::Result<T>) -> Result<T> {
    result.map_err(crate::errors::map_sqlite_error)
}
