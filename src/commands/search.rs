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
    if [args.regex, args.fuzzy, args.fts]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("--regex, --fuzzy, and --fts cannot be used together");
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
