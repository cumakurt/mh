use mh::config::{AppConfig, PolicyRuleConfig};
use mh::db::Database;
use mh::errors::MhError;
use mh::record_pipeline::{RecordPayload, execute};

#[test]
fn pipeline_skips_empty_command() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open database");
    let before = database.count_commands().expect("count");

    execute(
        &config,
        &database,
        &RecordPayload {
            command: "   ".to_string(),
            cwd: None,
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: None,
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("execute");

    let after = database.count_commands().expect("count");
    assert_eq!(before, after);
}

#[test]
fn rejects_invalid_started_at_timestamp() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open database");
    let result = execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo test".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: Some("not-a-date".to_string()),
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    );
    assert!(result.is_err());
    assert!(
        result
            .expect_err("invalid timestamp")
            .to_string()
            .contains("started_at")
    );
}

#[test]
fn pipeline_masks_mysql_password() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open database");
    execute(
        &config,
        &database,
        &RecordPayload {
            command: "mysql -u root -pSecret123".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("execute");

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
    assert!(rows[0].is_masked);
}

#[test]
fn pipeline_returns_policy_denied_for_matching_rule() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.policy.rules = vec![PolicyRuleConfig {
        id: "test-deny".to_string(),
        action: "deny".to_string(),
        risk_level: None,
        pattern: Some("^echo blocked$".to_string()),
        environment: None,
        hostname_pattern: None,
        message: "blocked by test policy".to_string(),
    }];
    mh::record_engines::invalidate_cache();

    let database = Database::open(&config).expect("open database");
    let result = execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo blocked".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    );

    match result {
        Err(error) => assert!(
            error.chain().any(|cause| matches!(
                cause.downcast_ref::<MhError>(),
                Some(MhError::PolicyDenied(_))
            )),
            "expected PolicyDenied, got: {error:#}"
        ),
        Ok(()) => panic!("policy deny should return an error"),
    }
    assert_eq!(database.count_commands().expect("count"), 0);
}

#[test]
fn pipeline_clamps_negative_duration_to_zero() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    let database = Database::open(&config).expect("open database");
    execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo duration".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(-25),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("execute");

    let row = database.get_command(1).expect("command");
    assert_eq!(row.duration_ms, Some(0));
}

#[test]
fn pipeline_masks_critical_secret_commands_end_to_end() {
    let cases = [
        ("mysql -u root -pSecret123", "Secret123"),
        (
            r#"curl -H "Authorization: Bearer abc123" https://api.example.com"#,
            "abc123",
        ),
        ("export AWS_SECRET_ACCESS_KEY=xxxx", "xxxx"),
        ("sshpass -p password ssh root@1.1.1.1", "password"),
        ("docker login -u user -p password", "password"),
        ("kubectl config set-credentials user --token=abc", "abc"),
    ];

    for (index, (command, secret)) in cases.iter().enumerate() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let mut config = AppConfig::default();
        config.database.path = temp_dir
            .path()
            .join(format!("history-{index}.db"))
            .to_string_lossy()
            .to_string();
        mh::record_engines::invalidate_cache();

        let database = Database::open(&config).expect("open database");
        execute(
            &config,
            &database,
            &RecordPayload {
                command: (*command).to_string(),
                cwd: Some("/tmp".to_string()),
                shell: Some("test".to_string()),
                exit_code: Some(0),
                duration_ms: Some(1),
                started_at: None,
                finished_at: None,
                session_id: Some(format!("sess-{index}")),
                tty: None,
                tags: None,
                env_context: None,
            },
        )
        .expect("execute");

        let row = database.get_command(1).expect("stored command");
        assert!(
            !row.command.contains(secret),
            "secret leaked for command: {command}"
        );
        assert!(row.is_masked, "expected masked flag for: {command}");
    }
}
