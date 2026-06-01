use anyhow::{Result, bail};

use crate::cli::SearchArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::SearchFilters;
use crate::output;

pub fn run(args: SearchArgs) -> Result<()> {
    if args.failed && args.success {
        bail!("--failed and --success cannot be used together");
    }
    if [args.json, args.csv, args.plain]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("--json, --csv, and --plain cannot be used together");
    }
    if [args.regex, args.fuzzy, args.fts, args.semantic]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("--regex, --fuzzy, --fts, and --semantic cannot be used together");
    }

    let config = AppConfig::load()?;
    let limit = args.limit.unwrap_or(config.display.default_limit);
    let database = Database::open(&config)?;

    const LARGE_HISTORY_THRESHOLD: i64 = 10_000;
    if (args.fuzzy || args.regex)
        && args.after.is_none()
        && args.before.is_none()
        && database.count_commands()? > LARGE_HISTORY_THRESHOLD
    {
        bail!("fuzzy/regex search on large history requires --after or --before to bound the scan");
    }
    if args.semantic {
        let query = args
            .query
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("--semantic requires a query"))?;
        let mut plan = crate::semantic_search::build_plan(query, limit);
        override_semantic_filters(&mut plan.filters, &args);
        let rows = database.search_commands(&plan.filters)?;
        let rows = crate::semantic_search::rank_rows(rows, &plan, limit);
        return output::print_rows_with_formats(&rows, args.json, args.plain, args.csv, false);
    }

    let filters = SearchFilters {
        query: args.query,
        cwd: args.cwd,
        failed: args.failed,
        success: args.success,
        user: args.user,
        shell: args.shell,
        after: args.after,
        before: args.before,
        regex: args.regex,
        fuzzy: args.fuzzy,
        fts: args.fts,
        tag: args.tag,
        category: args.category,
        pinned: args.pinned,
        duration_gt: args.duration_gt,
        duration_lt: args.duration_lt,
        hostname: args.hostname,
        ssh: args.ssh,
        root: args.root,
        limit,
        session_id: None,
        git_repo: args.git_repo,
        git_branch: args.git_branch,
        git_commit: args.git_commit,
        environment: args.env,
    };
    let rows = database.search_commands(&filters)?;
    output::print_rows_with_formats(&rows, args.json, args.plain, args.csv, false)
}

fn override_semantic_filters(filters: &mut SearchFilters, args: &SearchArgs) {
    if args.cwd.is_some() {
        filters.cwd = args.cwd.clone();
    }
    if args.failed {
        filters.failed = true;
    }
    if args.success {
        filters.success = true;
    }
    if args.user.is_some() {
        filters.user = args.user.clone();
    }
    if args.shell.is_some() {
        filters.shell = args.shell.clone();
    }
    if args.after.is_some() {
        filters.after = args.after.clone();
    }
    if args.before.is_some() {
        filters.before = args.before.clone();
    }
    if args.tag.is_some() {
        filters.tag = args.tag.clone();
    }
    if args.category.is_some() {
        filters.category = args.category.clone();
    }
    if args.pinned {
        filters.pinned = true;
    }
    if args.duration_gt.is_some() {
        filters.duration_gt = args.duration_gt;
    }
    if args.duration_lt.is_some() {
        filters.duration_lt = args.duration_lt;
    }
    if args.hostname.is_some() {
        filters.hostname = args.hostname.clone();
    }
    if args.ssh {
        filters.ssh = true;
    }
    if args.root {
        filters.root = true;
    }
    if args.git_repo.is_some() {
        filters.git_repo = args.git_repo.clone();
    }
    if args.git_branch.is_some() {
        filters.git_branch = args.git_branch.clone();
    }
    if args.git_commit.is_some() {
        filters.git_commit = args.git_commit.clone();
    }
    if args.env.is_some() {
        filters.environment = args.env.clone();
    }
}
