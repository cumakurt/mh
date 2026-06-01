use std::sync::{Arc, Barrier};
use std::thread;

use chrono::{SecondsFormat, Utc};

use mh::config::AppConfig;
use mh::db::Database;
use mh::models::CommandRecord;

fn sample_record(command: &str) -> CommandRecord {
    CommandRecord {
        command: command.to_string(),
        command_hash: "hash".to_string(),
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
        category: None,
        env_context: None,
        is_pinned: false,
        is_masked: false,
        tags: Vec::new(),
        environment_tier: None,
    }
}

#[test]
fn duplicate_insert_is_skipped_inside_transaction() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.history.dedupe_window_seconds = 3600;

    let database = Database::open(&config).expect("open");
    let record = sample_record("echo dedupe-test");

    let first = database
        .insert_command_unless_recent_duplicate(&record, config.history.dedupe_window_seconds)
        .expect("first insert");
    assert!(first.is_some());

    let second = database
        .insert_command_unless_recent_duplicate(&record, config.history.dedupe_window_seconds)
        .expect("second insert");
    assert!(second.is_none());

    let count = database.count_commands().expect("count");
    assert_eq!(count, 1);
}

#[test]
fn concurrent_duplicate_inserts_store_single_row() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    let db_path = temp_dir.path().join("history.db");
    config.database.path = db_path.to_string_lossy().to_string();
    config.history.dedupe_window_seconds = 3600;

    Database::open(&config).expect("initialize database");
    let record = sample_record("echo concurrent-dedupe");
    let barrier = Arc::new(Barrier::new(8));
    let window = config.history.dedupe_window_seconds;

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let db_path = db_path.clone();
            let record = record.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let database = Database::open_path(db_path).expect("open");
                database.insert_command_unless_recent_duplicate(&record, window)
            })
        })
        .collect();

    let mut inserted = 0usize;
    for handle in handles {
        let result = handle.join().expect("thread join");
        if result.expect("insert").is_some() {
            inserted += 1;
        }
    }

    assert_eq!(inserted, 1, "exactly one concurrent insert should win");
    let database = Database::open_path(db_path).expect("reopen");
    assert_eq!(database.count_commands().expect("count"), 1);
}
