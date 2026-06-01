use chrono::{SecondsFormat, Utc};
use mh::db::Database;
use mh::models::{CommandRecord, SearchFilters, StatsPeriod};

#[test]
fn inserts_and_searches_commands() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let record = CommandRecord {
        command: "docker ps -a".to_string(),
        command_hash: "hash".to_string(),
        cwd: Some("/tmp/project".to_string()),
        shell: Some("zsh".to_string()),
        username: Some("tester".to_string()),
        hostname: Some("host".to_string()),
        exit_code: Some(0),
        duration_ms: Some(42),
        started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        finished_at: None,
        session_id: Some("session".to_string()),
        tty: None,
        is_ssh: false,
        is_root: false,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment_tier: None,
        category: Some("docker".to_string()),
        env_context: None,
        is_pinned: false,
        is_masked: false,
        tags: vec!["test".to_string()],
    };

    let id = database
        .insert_command(&record)
        .expect("command should be inserted");

    let rows = database
        .search_commands(&SearchFilters {
            query: Some("docker".to_string()),
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
        .expect("search should succeed");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].command, "docker ps -a");
}

#[test]
fn searches_with_fuzzy_and_fts_modes() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let docker_id = database
        .insert_command(&sample_record(
            "docker container list",
            Some(0),
            None,
            Some("docker"),
        ))
        .expect("docker command should be inserted");
    database
        .insert_command(&sample_record("git status", Some(0), None, Some("git")))
        .expect("git command should be inserted");

    let fuzzy_rows = database
        .search_commands(&SearchFilters {
            query: Some("dcl".to_string()),
            cwd: None,
            failed: false,
            success: false,
            user: None,
            shell: None,
            after: None,
            before: None,
            regex: false,
            fuzzy: true,
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
        .expect("fuzzy search should succeed");
    assert_eq!(fuzzy_rows[0].id, docker_id);

    let fts_rows = database
        .search_commands(&SearchFilters {
            query: Some("container".to_string()),
            cwd: None,
            failed: false,
            success: false,
            user: None,
            shell: None,
            after: None,
            before: None,
            regex: false,
            fuzzy: false,
            fts: true,
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
        .expect("fts search should succeed");
    assert_eq!(fts_rows.len(), 1);
    assert_eq!(fts_rows[0].id, docker_id);
}

#[test]
fn detects_recent_duplicates() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let record = CommandRecord {
        command: "ls -la".to_string(),
        command_hash: "hash".to_string(),
        cwd: Some("/tmp".to_string()),
        shell: None,
        username: None,
        hostname: None,
        exit_code: Some(0),
        duration_ms: None,
        started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        finished_at: None,
        session_id: None,
        tty: None,
        is_ssh: false,
        is_root: false,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment_tier: None,
        category: None,
        env_context: None,
        is_pinned: false,
        is_masked: false,
        tags: Vec::new(),
    };

    database
        .insert_command(&record)
        .expect("command should be inserted");

    assert!(
        database
            .has_recent_duplicate("ls -la", Some("/tmp"), 60)
            .expect("duplicate check should succeed")
    );
}

#[test]
fn tags_and_pins_filter_results() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let docker_id = database
        .insert_command(&sample_record(
            "docker ps",
            Some(0),
            Some(40),
            Some("docker"),
        ))
        .expect("docker command should be inserted");
    let git_id = database
        .insert_command(&sample_record("git status", Some(1), Some(12), Some("git")))
        .expect("git command should be inserted");

    assert_eq!(
        database
            .add_tags(&[docker_id], &["prod".to_string(), "ops".to_string()])
            .expect("tags should be added"),
        2
    );
    assert_eq!(
        database
            .add_tags(&[docker_id], &["prod".to_string()])
            .expect("duplicate tag should be ignored"),
        0
    );

    let tagged_rows = database
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
            tag: Some("prod".to_string()),
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
        .expect("tag search should succeed");
    assert_eq!(tagged_rows.len(), 1);
    assert_eq!(tagged_rows[0].id, docker_id);
    assert!(tagged_rows[0].tags.contains(&"prod".to_string()));

    database
        .set_pinned(&[git_id], true)
        .expect("pin should succeed");
    let pinned_rows = database
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
            pinned: true,
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
        .expect("pinned search should succeed");
    assert_eq!(pinned_rows.len(), 1);
    assert_eq!(pinned_rows[0].id, git_id);
    assert!(pinned_rows[0].is_pinned);

    assert_eq!(
        database
            .remove_tags(docker_id, &["prod".to_string()])
            .expect("tag removal should succeed"),
        1
    );
}

#[test]
fn summarizes_command_statistics() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    database
        .insert_command(&sample_record("git status", Some(0), Some(10), Some("git")))
        .expect("first command should be inserted");
    database
        .insert_command(&sample_record("git status", Some(1), Some(30), Some("git")))
        .expect("second command should be inserted");
    database
        .insert_command(&sample_record(
            "docker ps",
            Some(0),
            Some(20),
            Some("docker"),
        ))
        .expect("third command should be inserted");

    let summary = database
        .stats_summary(StatsPeriod::All, 5)
        .expect("stats should be generated");

    assert_eq!(summary.total_commands, 3);
    assert_eq!(summary.successful_commands, 2);
    assert_eq!(summary.failed_commands, 1);
    assert_eq!(summary.longest_duration_ms, Some(30));
    assert_eq!(summary.top_commands[0].label, "git status");
    assert_eq!(summary.top_commands[0].count, 2);
    assert!(
        summary
            .category_counts
            .iter()
            .any(|entry| entry.label == "git")
    );
    assert!(
        summary
            .error_prone_commands
            .iter()
            .any(|entry| entry.label == "git status")
    );
    assert!(
        !database
            .hourly_activity(StatsPeriod::All)
            .expect("hourly activity should load")
            .is_empty()
    );
}

#[test]
fn deletes_commands_and_clears_history() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let first_id = database
        .insert_command(&sample_record("ls -la", Some(0), None, None))
        .expect("first command should be inserted");
    database
        .insert_command(&sample_record("pwd", Some(0), None, None))
        .expect("second command should be inserted");

    assert_eq!(
        database
            .delete_command_ids(&[first_id])
            .expect("delete should succeed"),
        1
    );
    assert_eq!(database.count_commands().expect("count should succeed"), 1);

    assert_eq!(
        database
            .clear_history(None, None, false)
            .expect("clear should succeed"),
        1
    );
    assert_eq!(database.count_commands().expect("count should succeed"), 0);
}

#[test]
fn clear_history_allows_new_commands_to_be_stored_and_listed() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    database
        .insert_command(&sample_record("ls -la", Some(0), None, None))
        .expect("initial command should be inserted");
    database
        .clear_history(None, None, false)
        .expect("clear should succeed");

    let id = database
        .insert_command(&sample_record("echo after-clear", Some(0), None, None))
        .expect("command after clear should be inserted");

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
        .expect("search after clear should succeed");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].command, "echo after-clear");
}

#[test]
fn stores_audit_rows_and_snippets() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    database
        .insert_audit_log(
            "masked",
            "curl -H 'Authorization: Bearer token'",
            "secret detected",
            Some("tester"),
            Some("host"),
        )
        .expect("audit row should be inserted");
    let audit_rows = database
        .audit_rows(false, 10)
        .expect("audit rows should load");
    assert_eq!(audit_rows.len(), 1);
    assert_eq!(audit_rows[0].event_type, "masked");

    database
        .save_snippet(
            "ssh-host",
            "ssh {{user}}@{{host}}",
            Some("Connect to a host"),
            Some("ssh"),
        )
        .expect("snippet should be saved");
    let snippet = database
        .get_snippet("ssh-host")
        .expect("snippet should load");
    assert_eq!(snippet.command, "ssh {{user}}@{{host}}");

    database
        .increment_snippet_use("ssh-host")
        .expect("snippet use should increment");
    let snippet = database
        .get_snippet("ssh-host")
        .expect("snippet should reload");
    assert_eq!(snippet.use_count, 1);
    assert_eq!(
        database
            .delete_snippet("ssh-host")
            .expect("snippet should delete"),
        1
    );
}

#[test]
fn enforces_max_entries_and_keeps_pinned_records() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let pinned_id = database
        .insert_command(&sample_record("pinned command", Some(0), None, None))
        .expect("pinned command should be inserted");
    database
        .set_pinned(&[pinned_id], true)
        .expect("pin should succeed");

    for index in 0..3 {
        database
            .insert_command(&sample_record(
                &format!("command-{index}"),
                Some(0),
                None,
                None,
            ))
            .expect("command should be inserted");
    }

    let deleted = database
        .enforce_max_entries(2, false)
        .expect("max entry enforcement should succeed");
    assert_eq!(deleted, 2);
    assert_eq!(database.count_commands().expect("count should succeed"), 2);

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
            pinned: true,
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
        .expect("pinned search should succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, pinned_id);
}

#[test]
fn filters_by_hostname_ssh_and_root_flags() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let mut local = sample_record("local command", Some(0), None, None);
    local.hostname = Some("workstation".to_string());
    database
        .insert_command(&local)
        .expect("local command should be inserted");

    let mut remote = sample_record("ssh command", Some(0), None, None);
    remote.hostname = Some("kali".to_string());
    remote.is_ssh = true;
    database
        .insert_command(&remote)
        .expect("remote command should be inserted");

    let mut root = sample_record("root command", Some(0), None, None);
    root.is_root = true;
    database
        .insert_command(&root)
        .expect("root command should be inserted");

    let hostname_rows = database
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
            hostname: Some("kali".to_string()),
            ssh: false,
            root: false,
            limit: 10,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("hostname search should succeed");
    assert_eq!(hostname_rows.len(), 1);
    assert_eq!(hostname_rows[0].command, "ssh command");

    let ssh_rows = database
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
            ssh: true,
            root: false,
            limit: 10,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("ssh search should succeed");
    assert_eq!(ssh_rows.len(), 1);

    let root_rows = database
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
            root: true,
            limit: 10,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("root search should succeed");
    assert_eq!(root_rows.len(), 1);
    assert_eq!(root_rows[0].command, "root command");
}

#[test]
fn filters_by_git_context() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let mut record = sample_record("git commit", Some(0), None, Some("git"));
    record.git_repo = Some("/tmp/repo".to_string());
    record.git_branch = Some("main".to_string());
    record.git_commit = Some("abc1234".to_string());
    database
        .insert_command(&record)
        .expect("git command should be inserted");
    database
        .insert_command(&sample_record("ls", Some(0), None, None))
        .expect("non-git command should be inserted");

    let branch_rows = database
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
            git_repo: Some("/tmp/repo".to_string()),
            git_branch: Some("main".to_string()),
            git_commit: None,
            environment: None,
        })
        .expect("git branch search should succeed");

    assert_eq!(branch_rows.len(), 1);
    assert_eq!(branch_rows[0].command, "git commit");
    assert_eq!(branch_rows[0].git_commit.as_deref(), Some("abc1234"));
}

fn sample_record(
    command: &str,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
    category: Option<&str>,
) -> CommandRecord {
    CommandRecord {
        command: command.to_string(),
        command_hash: format!("hash-{command}"),
        cwd: Some("/tmp/project".to_string()),
        shell: Some("zsh".to_string()),
        username: Some("tester".to_string()),
        hostname: Some("host".to_string()),
        exit_code,
        duration_ms,
        started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        finished_at: None,
        session_id: Some("session".to_string()),
        tty: None,
        is_ssh: false,
        is_root: false,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment_tier: None,
        category: category.map(ToOwned::to_owned),
        env_context: None,
        is_pinned: false,
        is_masked: false,
        tags: Vec::new(),
    }
}

fn base_filters() -> SearchFilters {
    SearchFilters {
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
    }
}

#[test]
fn literal_search_escapes_like_wildcards() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");
    database
        .insert_command(&sample_record("echo 100% ready", Some(0), None, None))
        .expect("insert percent command");
    database
        .insert_command(&sample_record("git status", Some(0), None, None))
        .expect("insert git command");

    let mut filters = base_filters();
    filters.query = Some("%".to_string());
    let rows = database.search_commands(&filters).expect("literal search");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].command, "echo 100% ready");
}

#[test]
fn cwd_filter_escapes_like_wildcards() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");
    let mut with_underscore = sample_record("ls", Some(0), None, None);
    with_underscore.cwd = Some("/tmp/a_b".to_string());
    let mut without_underscore = sample_record("pwd", Some(0), None, None);
    without_underscore.cwd = Some("/tmp/acb".to_string());
    database
        .insert_command(&with_underscore)
        .expect("insert underscore cwd");
    database
        .insert_command(&without_underscore)
        .expect("insert plain cwd");

    let mut filters = base_filters();
    filters.cwd = Some("_".to_string());
    let rows = database.search_commands(&filters).expect("cwd search");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cwd.as_deref(), Some("/tmp/a_b"));
}

#[test]
fn fts_search_with_only_operators_returns_empty_result() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");
    database
        .insert_command(&sample_record("docker ps", Some(0), None, None))
        .expect("insert command");

    let mut filters = base_filters();
    filters.query = Some("*** -- ++".to_string());
    filters.fts = true;
    let rows = database.search_commands(&filters).expect("fts search");
    assert!(rows.is_empty());
}

#[test]
fn search_rejects_invalid_date_bounds() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let mut filters = base_filters();
    filters.after = Some("yesterday".to_string());
    let error = database
        .search_commands(&filters)
        .expect_err("invalid date should fail");
    assert!(format!("{error:#}").contains("invalid date bound"));
}

#[test]
fn round_trips_environment_tier_on_command_rows() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let record = CommandRecord {
        command: "kubectl get pods".to_string(),
        command_hash: "hash-prod".to_string(),
        cwd: Some("/srv/app".to_string()),
        shell: Some("zsh".to_string()),
        username: Some("ops".to_string()),
        hostname: Some("prod-web-01".to_string()),
        exit_code: Some(0),
        duration_ms: Some(10),
        started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        finished_at: None,
        session_id: Some("session-prod".to_string()),
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

    let id = database
        .insert_command(&record)
        .expect("command should be inserted");
    let row = database.get_command(id).expect("command should load");
    assert_eq!(row.environment_tier.as_deref(), Some("production"));
}

#[test]
fn enables_wal_mode_and_restrictive_permissions() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("history.db");
    Database::open_path(db_path.clone()).expect("database should open");

    let connection =
        rusqlite::Connection::open(&db_path).expect("database should reopen for inspection");
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode should be readable");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&db_path)
            .expect("database metadata should be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
#[cfg(unix)]
fn opening_database_does_not_chmod_existing_parent_directory() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let parent = temp_dir.path().join("db-parent");
    std::fs::create_dir_all(&parent).expect("parent dir");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))
        .expect("chmod parent");

    Database::open_path(parent.join("history.db")).expect("database should open");

    let mode = std::fs::metadata(&parent)
        .expect("parent metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
}

#[test]
#[cfg(unix)]
fn opening_database_rejects_world_writable_parent_directory() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let parent = temp_dir.path().join("unsafe-db-parent");
    std::fs::create_dir_all(&parent).expect("parent dir");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
        .expect("chmod parent");

    let error = match Database::open_path(parent.join("history.db")) {
        Ok(_) => panic!("world-writable database parent should fail"),
        Err(error) => error,
    };
    assert!(format!("{error:#}").contains("writable by group or others"));
}

#[test]
fn exceeds_size_limit_reflects_file_size_on_disk() {
    use std::io::Write;

    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let path = temp_dir.path().join("history.db");
    let database = Database::open_path(path.clone()).expect("database should open");

    assert!(!database.exceeds_size_limit(512).expect("size check"));

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open for append");
    file.write_all(&vec![0_u8; 2 * 1024 * 1024])
        .expect("append padding");

    assert!(database.exceeds_size_limit(1).expect("size check"));
    assert!(!database.exceeds_size_limit(0).expect("zero disables limit"));
}

#[test]
fn fuzzy_search_prefilters_with_like_before_scoring() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    for (command, category) in [
        ("docker compose up -d", "docker"),
        ("kubectl get pods", "k8s"),
        ("git status", "git"),
    ] {
        let record = CommandRecord {
            command: command.to_string(),
            command_hash: format!("hash-{command}"),
            cwd: Some("/tmp".to_string()),
            shell: Some("zsh".to_string()),
            username: Some("tester".to_string()),
            hostname: Some("host".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            finished_at: None,
            session_id: Some("session".to_string()),
            tty: None,
            is_ssh: false,
            is_root: false,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            category: Some(category.to_string()),
            env_context: None,
            is_pinned: false,
            is_masked: false,
            tags: Vec::new(),
            environment_tier: None,
        };
        database.insert_command(&record).expect("insert");
    }

    let rows = database
        .search_commands(&SearchFilters {
            query: Some("doker".to_string()),
            cwd: None,
            failed: false,
            success: false,
            user: None,
            shell: None,
            after: None,
            before: None,
            regex: false,
            fuzzy: true,
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
    assert!(rows[0].command.contains("docker"));
}
