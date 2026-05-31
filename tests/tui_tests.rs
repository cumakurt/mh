use clap::Parser;
use mh::cli::Cli;
use mh::config::AppConfig;
use mh::db::Database;
use mh::models::SearchFilters;
use mh::output;

#[test]
fn parses_tui_command() {
    let cli = Cli::try_parse_from(["mh", "tui", "--limit", "25"]).expect("parse tui");
    match cli.command {
        mh::cli::Command::Tui(args) => assert_eq!(args.limit, 25),
        _ => panic!("expected tui command"),
    }
}

#[test]
fn table_fallback_renders_rows_without_terminal() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open database");
    let rows = database
        .search_commands(&SearchFilters {
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
            limit: 10,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("search");

    output::print_rows(&rows, false, false).expect("table fallback output");
}
