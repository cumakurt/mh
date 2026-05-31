use std::env;

use anyhow::{Result, bail};

use crate::cli::LastArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::SearchFilters;
use crate::output;

pub fn run(args: LastArgs) -> Result<()> {
    if [args.json, args.markdown, args.plain]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("--json, --markdown, and --plain cannot be used together");
    }

    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let filters = SearchFilters {
        query: None,
        cwd: args.cwd,
        failed: args.failed,
        success: false,
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: args.tag,
        category: args.category,
        pinned: args.pinned,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit: args.limit.unwrap_or(config.display.default_limit),
        session_id: if args.session {
            env::var("MH_SESSION_ID").ok()
        } else {
            None
        },
        git_repo: args.git_repo,
        git_branch: args.git_branch,
        git_commit: args.git_commit,
        environment: args.env,
    };
    let rows = database.search_commands(&filters)?;
    output::print_rows_with_formats(&rows, args.json, args.plain, false, args.markdown)
}
