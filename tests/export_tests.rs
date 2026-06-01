use mh::config::AppConfig;
use mh::db::Database;

#[test]
fn sqlite_export_without_audit_clears_audit_table() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let export_path = temp_dir.path().join("export.db");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open");
    database
        .insert_audit_log("risky", "secret-cmd", "test", Some("u"), Some("h"))
        .expect("audit");

    database.checkpoint_wal().expect("checkpoint");
    std::fs::copy(database.path(), &export_path).expect("copy");
    let connection = rusqlite::Connection::open(&export_path).expect("open export");
    connection
        .execute("DELETE FROM audit_log", [])
        .expect("clear audit");

    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

#[test]
fn sanitize_redacts_command_export_rows() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open");
    mh::record_pipeline::execute(
        &config,
        &database,
        &mh::record_pipeline::RecordPayload {
            command: "mysql -u root -pSecret123".to_string(),
            cwd: None,
            shell: None,
            exit_code: Some(0),
            duration_ms: None,
            started_at: None,
            finished_at: None,
            session_id: None,
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("record");

    let mut rows = database
        .search_commands(&mh::models::SearchFilters {
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
    mh::security::redact_for_audit(&rows[0].command, &config).expect("redact");
    for row in &mut rows {
        row.command = mh::security::redact_for_audit(&row.command, &config).expect("redact");
    }
    assert!(!rows[0].command.contains("Secret123"));
}

#[test]
#[cfg(unix)]
fn export_refuses_symlink_destination() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open");
    mh::record_pipeline::execute(
        &config,
        &database,
        &mh::record_pipeline::RecordPayload {
            command: "echo export-test".to_string(),
            cwd: None,
            shell: None,
            exit_code: Some(0),
            duration_ms: None,
            started_at: None,
            finished_at: None,
            session_id: None,
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("record");

    let export_path = temp_dir.path().join("history.json");
    let symlink_path = temp_dir.path().join("link.json");
    std::os::unix::fs::symlink(&export_path, &symlink_path).expect("symlink");

    let result = mh::commands::export::run(mh::cli::ExportArgs {
        json: Some(symlink_path.to_string_lossy().to_string()),
        csv: None,
        markdown: None,
        compressed: None,
        sqlite: None,
        after: None,
        before: None,
        tag: None,
        category: None,
        include_secrets: false,
        sanitize: true,
        without_audit: false,
        sanitize_audit: false,
    });
    assert!(result.is_err(), "export to symlink destination must fail");
    let message = format!("{:#}", result.expect_err("error"));
    assert!(
        message.contains("symlink") || message.contains("refusing"),
        "unexpected error: {message}"
    );
}
