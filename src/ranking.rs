//! Context-aware ranking for history pickers (cwd, success, recency).

use std::env;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::models::CommandRow;

#[derive(Debug, Clone)]
pub struct RankContext {
    pub cwd: String,
    pub hostname: String,
}

impl RankContext {
    pub fn from_env() -> Self {
        Self {
            cwd: env::current_dir()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            hostname: hostname::get()
                .ok()
                .and_then(|value| value.into_string().ok())
                .unwrap_or_default(),
        }
    }
}

/// Higher is better. Used alone or added to fuzzy match scores.
pub fn context_score(row: &CommandRow, ctx: &RankContext) -> i64 {
    let mut score = 0_i64;

    if row.is_pinned {
        score += 10_000;
    }

    if let Some(row_cwd) = row.cwd.as_deref().filter(|value| !value.is_empty()) {
        if row_cwd == ctx.cwd {
            score += 5_000;
        } else if ctx.cwd.starts_with(row_cwd) || row_cwd.starts_with(&ctx.cwd) {
            score += 2_500;
        } else if Path::new(row_cwd).file_name() == Path::new(&ctx.cwd).file_name() {
            score += 800;
        }
    }

    if let Some(row_host) = row.hostname.as_deref().filter(|value| !value.is_empty())
        && row_host == ctx.hostname
    {
        score += 1_200;
    }

    match row.exit_code {
        Some(0) => score += 1_500,
        Some(_) => score -= 800,
        None => {}
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(&row.started_at) {
        let age_hours = (Utc::now() - parsed.with_timezone(&Utc)).num_hours().max(0);
        score += (168_i64 - age_hours.min(168)) * 4;
    }

    score
}

pub fn sort_by_context(rows: &mut [CommandRow], ctx: &RankContext) {
    rows.sort_by(|left, right| {
        context_score(right, ctx)
            .cmp(&context_score(left, ctx))
            .then_with(|| right.started_at.cmp(&left.started_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

pub fn rank_indices(
    rows: &[CommandRow],
    ctx: &RankContext,
    fuzzy_scores: &[(i64, usize)],
) -> Vec<usize> {
    let mut ranked = fuzzy_scores.to_vec();
    ranked.sort_by(|(score_a, index_a), (score_b, index_b)| {
        let total_a = *score_a + context_score(&rows[*index_a], ctx);
        let total_b = *score_b + context_score(&rows[*index_b], ctx);
        total_b.cmp(&total_a).then_with(|| index_a.cmp(index_b))
    });
    ranked.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cwd: &str, exit_code: Option<i32>) -> CommandRow {
        CommandRow {
            id: 1,
            command: "git status".to_string(),
            cwd: Some(cwd.to_string()),
            shell: None,
            username: None,
            hostname: None,
            exit_code,
            duration_ms: None,
            started_at: Utc::now().to_rfc3339(),
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            category: None,
            tags: Vec::new(),
            is_pinned: false,
            is_masked: false,
            environment_tier: None,
        }
    }

    #[test]
    fn prefers_same_cwd_and_success() {
        let ctx = RankContext {
            cwd: "/srv/api".to_string(),
            hostname: "dev".to_string(),
        };
        let good = row("/srv/api", Some(0));
        let bad = row("/srv/api", Some(1));
        let other = row("/tmp", Some(0));
        assert!(context_score(&good, &ctx) > context_score(&bad, &ctx));
        assert!(context_score(&good, &ctx) > context_score(&other, &ctx));
    }
}
