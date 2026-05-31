use chrono::{SecondsFormat, Utc};
use mh::audit_chain;
use mh::break_glass;
use mh::config::AppConfig;
use mh::db::Database;
use mh::environment::{self, EnvironmentTier};
use mh::execution_policy::environment_tier_for_command;
use mh::models::CommandRecord;
use mh::policy::{PolicyAction, PolicyEngine};

fn isolated_config(temp_dir: &tempfile::TempDir) -> AppConfig {
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config
}

#[test]
fn policy_denies_critical_in_production() {
    let config = AppConfig::default();
    let engine = PolicyEngine::from_config(&config).expect("policy engine");
    let decision = engine.evaluate("rm -rf /", Some("prod-web"), Some("production"));
    assert_eq!(decision.action, PolicyAction::Deny);
}

#[test]
fn audit_hash_chain_links_entries() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = isolated_config(&temp_dir);
    let database = Database::open(&config).expect("database");

    database
        .insert_audit_log("skipped", "secret", "masked", Some("user"), Some("host"))
        .expect("first audit");
    let second = database
        .insert_audit_log("risky", "rm -rf /", "critical", Some("user"), Some("host"))
        .expect("second audit");

    assert!(!second.prev_hash.as_deref().unwrap_or("").is_empty());
    database.verify_audit_chain().expect("valid chain");
}

#[test]
fn legal_hold_blocks_retention_purge() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = isolated_config(&temp_dir);
    let database = Database::open(&config).expect("database");

    let record = CommandRecord {
        command: "old command".to_string(),
        command_hash: "hash-old".to_string(),
        cwd: None,
        shell: None,
        username: Some("tester".to_string()),
        hostname: None,
        exit_code: Some(0),
        duration_ms: None,
        started_at: "2020-01-01T00:00:00Z".to_string(),
        finished_at: None,
        session_id: Some("session-hold".to_string()),
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
        environment_tier: None,
    };
    database.insert_command(&record).expect("insert");

    database
        .add_legal_hold(
            "incident-1",
            Some("session-hold"),
            None,
            None,
            None,
            Some("investigation"),
        )
        .expect("legal hold");

    let deleted = database.retention_purge(30, true).expect("purge");
    assert_eq!(deleted, 0);
    assert_eq!(database.count_commands().expect("count"), 1);
}

#[test]
fn runbook_is_created_from_session() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = isolated_config(&temp_dir);
    let database = Database::open(&config).expect("database");
    let session = "session-runbook";

    for command in ["git pull", "cargo test", "cargo build"] {
        database
            .insert_command(&CommandRecord {
                command: command.to_string(),
                command_hash: format!("hash-{command}"),
                cwd: Some("/tmp/project".to_string()),
                shell: None,
                username: None,
                hostname: None,
                exit_code: Some(0),
                duration_ms: None,
                started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                finished_at: None,
                session_id: Some(session.to_string()),
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
                environment_tier: None,
            })
            .expect("insert");
    }

    database
        .create_runbook_from_session("deploy", Some("Deploy flow"), session)
        .expect("runbook");

    let steps = database.runbook_steps("deploy").expect("steps");
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].command, "git pull");
}

#[test]
fn environment_classifies_prod_host() {
    let config = AppConfig::default();
    assert_eq!(
        environment::classify(&config, Some("prod-web-01"), None, None),
        EnvironmentTier::Production
    );
}

#[test]
fn break_glass_state_expires() {
    let state = break_glass::BreakGlassState {
        reason: "incident".to_string(),
        expires_at: "2020-01-01T00:00:00Z".to_string(),
        activated_at: "2020-01-01T00:00:00Z".to_string(),
    };
    assert!(state.is_expired());
}

#[test]
fn timeline_returns_session_commands() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = isolated_config(&temp_dir);
    let database = Database::open(&config).expect("database");
    let session = "timeline-session";

    database
        .insert_command(&CommandRecord {
            command: "echo one".to_string(),
            command_hash: "hash-one".to_string(),
            cwd: None,
            shell: None,
            username: None,
            hostname: None,
            exit_code: Some(0),
            duration_ms: None,
            started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            finished_at: None,
            session_id: Some(session.to_string()),
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
            environment_tier: Some("development".to_string()),
        })
        .expect("insert");

    let timeline = database.session_timeline(session).expect("timeline");
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].command, "echo one");
}

#[test]
fn stored_environment_tier_enforces_production_replay_policy() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = isolated_config(&temp_dir);
    let database = Database::open(&config).expect("database");

    let record = CommandRecord {
        command: "rm -rf /".to_string(),
        command_hash: "hash-critical".to_string(),
        cwd: Some("/srv".to_string()),
        shell: None,
        username: Some("ops".to_string()),
        hostname: Some("prod-web-01".to_string()),
        exit_code: Some(0),
        duration_ms: None,
        started_at: "2026-05-31T12:00:00Z".to_string(),
        finished_at: None,
        session_id: Some("session-replay".to_string()),
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
    };
    let id = database.insert_command(&record).expect("insert");
    let row = database.get_command(id).expect("load");

    let tier = environment_tier_for_command(
        &config,
        row.environment_tier.as_deref(),
        row.hostname.as_deref(),
        row.cwd.as_deref(),
        row.git_repo.as_deref(),
    );
    let engine = PolicyEngine::from_config(&config).expect("policy engine");
    let decision = engine.evaluate(&row.command, row.hostname.as_deref(), tier.as_deref());
    assert_eq!(decision.action, PolicyAction::Deny);
}

#[test]
fn hash_chain_detects_tampering() {
    let rows = vec![mh::models::AuditRow {
        id: 1,
        event_type: "risky".to_string(),
        raw_command: Some("rm -rf /".to_string()),
        reason: Some("critical".to_string()),
        username: None,
        hostname: None,
        created_at: "t1".to_string(),
        prev_hash: Some(String::new()),
        entry_hash: Some("deadbeef".to_string()),
    }];
    assert!(audit_chain::verify_chain(&rows).is_err());
}
