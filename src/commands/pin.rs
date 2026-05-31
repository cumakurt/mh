use anyhow::Result;

use crate::cli::{PinArgs, PinnedArgs};
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::SearchFilters;
use crate::output;

pub fn run(args: PinArgs, pinned: bool) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let updated = database.set_pinned(&args.ids, pinned)?;
    let action = if pinned { "Pinned" } else { "Unpinned" };
    println!(
        "{action} {updated} command{}",
        if updated == 1 { "" } else { "s" }
    );
    Ok(())
}

pub fn run_pinned(args: PinnedArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let rows = database.search_commands(&SearchFilters {
        query: None,
        cwd: None,
        failed: false,
        success: false,
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: None,
        category: None,
        pinned: true,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit: args.limit.unwrap_or(config.display.default_limit),
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    })?;
    output::print_rows(&rows, args.json, args.plain)
}
