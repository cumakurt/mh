use mh::config::AppConfig;
use mh::db::Database;
use mh::models::CommandRecord;

#[test]
fn maybe_enforce_skips_count_when_not_due() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.history.max_entries = 100_000;

    let database = Database::open(&config).expect("open database");
    insert_row(&database, "first");
    insert_row(&database, "second");

    let skipped = database
        .maybe_enforce_max_entries(100_000, false, 2)
        .expect("maybe enforce");
    assert_eq!(skipped, 0);
    assert_eq!(database.count_commands().expect("count"), 2);
}

#[test]
fn maybe_enforce_runs_when_near_limit() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.history.max_entries = 2;

    let database = Database::open(&config).expect("open database");
    insert_row(&database, "first");
    insert_row(&database, "second");
    insert_row(&database, "third");

    let deleted = database
        .maybe_enforce_max_entries(2, false, 3)
        .expect("maybe enforce");
    assert_eq!(deleted, 1);
    assert_eq!(database.count_commands().expect("count"), 2);
}

fn insert_row(database: &Database, command: &str) {
    database
        .insert_command(&CommandRecord {
            command: command.to_string(),
            command_hash: format!("hash-{command}"),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            username: Some("tester".to_string()),
            hostname: Some("localhost".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            is_ssh: false,
            is_root: false,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            category: None,
            env_context: None,
            is_pinned: false,
            is_masked: false,
            tags: vec![],
            environment_tier: None,
        })
        .expect("insert");
}
