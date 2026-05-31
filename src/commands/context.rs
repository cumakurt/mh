use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL};

use crate::cli::{
    ContextArgs, ContextBranchArgs, ContextCommand, ContextHistoryArgs, ContextListArgs,
};
use crate::config::AppConfig;
use crate::db::Database;
use crate::git_detect;
use crate::models::{SearchFilters, StatEntry};
use crate::output;
use crate::output::styling::Styler;

pub fn run(args: ContextArgs) -> Result<()> {
    match args.command {
        None => show_current_context(),
        Some(ContextCommand::Repos(list_args)) => list_repos(list_args),
        Some(ContextCommand::Branches(branch_args)) => list_branches(branch_args),
        Some(ContextCommand::History(history_args)) => show_history(history_args),
    }
}

fn show_current_context() -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);

    match git_detect::detect_git_context_from_env() {
        Some(context) => {
            println!(
                "{}",
                styler.label_value("Repository", styler.accent(context.repo))
            );
            if let Some(branch) = context.branch {
                println!("{}", styler.label_value("Branch", styler.success(branch)));
            }
            if let Some(commit) = context.commit {
                println!("{}", styler.label_value("Commit", commit));
            }
        }
        None => println!("{}", styler.warning("Not inside a git repository")),
    }
    Ok(())
}

fn list_repos(args: ContextListArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let limit = args.limit.unwrap_or(20);
    let entries = database.distinct_git_repos(limit)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    print_stat_entries(&Styler::from_config(&config), "Git repositories", &entries);
    Ok(())
}

fn list_branches(args: ContextBranchArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let limit = args.limit.unwrap_or(20);
    let entries = database.distinct_git_branches(args.repo.as_deref(), limit)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    let title = if let Some(repo) = args.repo {
        format!("Branches in {repo}")
    } else {
        "Git branches".to_string()
    };
    print_stat_entries(&Styler::from_config(&config), &title, &entries);
    Ok(())
}

fn show_history(args: ContextHistoryArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let limit = args.limit.unwrap_or(config.display.default_limit);

    let filters = SearchFilters {
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
        pinned: false,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit,
        session_id: None,
        git_repo: args.repo,
        git_branch: args.branch,
        git_commit: args.commit,
        environment: None,
    };

    let rows = database.search_commands(&filters)?;
    output::print_rows_with_formats(&rows, args.json, args.plain, false, false)
}

fn print_stat_entries(styler: &Styler, title: &str, entries: &[StatEntry]) {
    if entries.is_empty() {
        println!("{}", styler.warning(format!("No {title} found")));
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        styler.cell("Label", Some(comfy_table::Color::Cyan)),
        styler.cell("Commands", Some(comfy_table::Color::Cyan)),
    ]);

    for entry in entries {
        table.add_row(vec![
            styler.cell(&entry.label, None),
            styler.cell(entry.count, Some(comfy_table::Color::Green)),
        ]);
    }

    println!("{}:", styler.section_title(title));
    println!("{table}");
}
