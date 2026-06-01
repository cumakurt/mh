use mh::config::AppConfig;
use mh::db::Database;
use mh::models::CommandRecord;

#[test]
fn incident_bundle_redacts_and_marks_risk() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    let database = Database::open(&config).expect("database");
    database
        .insert_command(&CommandRecord {
            command: "rm -rf /tmp/sandbox".to_string(),
            command_hash: "hash-1".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("zsh".to_string()),
            username: Some("user".to_string()),
            hostname: Some("host".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: "2026-06-01T00:00:00Z".to_string(),
            finished_at: None,
            session_id: Some("session-1".to_string()),
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
            tags: Vec::new(),
            environment_tier: Some("production".to_string()),
        })
        .expect("insert");
    database
        .insert_audit_log(
            "risky",
            "mysql -pSecret123",
            "token=Secret456",
            Some("u"),
            Some("h"),
        )
        .expect("audit");

    let bundle =
        mh::incident_bundle::build(&config, &database, "session-1", false).expect("bundle");

    assert_eq!(bundle.command_count, 1);
    assert_eq!(bundle.risky_commands.len(), 1);
    assert!(bundle.audit_chain_verified);
    assert!(
        !serde_json::to_string(&bundle)
            .expect("json")
            .contains("Secret123")
    );
    assert!(
        !serde_json::to_string(&bundle)
            .expect("json")
            .contains("Secret456")
    );
}
