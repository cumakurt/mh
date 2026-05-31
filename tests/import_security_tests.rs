use mh::commands::import_history;
use mh::config::AppConfig;
use mh::db::Database;
use mh::security::{SecurityAction, process_command};

#[test]
fn import_skips_plaintext_secrets_when_skip_enabled() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.security.skip_secret_commands = true;

    let decision =
        process_command("export AWS_SECRET_ACCESS_KEY=xxxx", &config).expect("security should run");
    assert!(matches!(decision.action, SecurityAction::Skipped(_)));

    let database = Database::open(&config).expect("database");
    assert_eq!(database.count_commands().expect("count"), 0);
}

#[test]
fn import_rejects_invalid_csv_column_count() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let csv_path = temp_dir.path().join("bad.csv");
    std::fs::write(&csv_path, "id,started_at\n1,2026-01-01T00:00:00Z\n").expect("write csv");

    let result = import_history::run(mh::cli::ImportArgs {
        file: csv_path.to_string_lossy().to_string(),
        merge: false,
        dry_run: true,
    });
    assert!(result.is_err());
    assert!(
        result
            .expect_err("invalid csv")
            .to_string()
            .contains("expected 9 columns")
    );
}

#[test]
fn import_rejects_oversized_file() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let json_path = temp_dir.path().join("huge.json");
    let oversized = vec![b' '; 64 * 1024 * 1024 + 1];
    std::fs::write(&json_path, oversized).expect("write file");

    let result = import_history::run(mh::cli::ImportArgs {
        file: json_path.to_string_lossy().to_string(),
        merge: false,
        dry_run: true,
    });
    assert!(result.is_err());
    assert!(
        result
            .expect_err("oversized import")
            .to_string()
            .contains("maximum size")
    );
}

#[test]
fn import_masks_mysql_password() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let json_path = temp_dir.path().join("history.json");
    let payload = r#"[{"id":1,"started_at":"2026-01-01T00:00:00Z","exit_code":0,"duration_ms":1,"cwd":"/tmp","shell":"bash","category":null,"command":"mysql -u root -pSecret123","tags":[],"is_pinned":false,"is_masked":false}]"#;
    std::fs::write(&json_path, payload).expect("write json");

    let config_path = temp_dir.path().join("config.toml");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.write_to_path(&config_path).expect("write config");
    unsafe {
        std::env::set_var("MH_CONFIG_NO_CACHE", "1");
        std::env::set_var("MH_CONFIG", config_path.to_string_lossy().to_string());
    }

    import_history::run(mh::cli::ImportArgs {
        file: json_path.to_string_lossy().to_string(),
        merge: false,
        dry_run: false,
    })
    .expect("import");

    let database = Database::open(&config).expect("database");
    let rows = database
        .search_commands(&mh::models::SearchFilters {
            query: Some("mysql".to_string()),
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
            limit: 5,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("search");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].command.contains("Secret123"));
}
