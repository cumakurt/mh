use std::hint::black_box;

use mh::config::AppConfig;
use mh::db::Database;
use mh::models::CommandRecord;

fn main() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("bench.db")
        .to_string_lossy()
        .to_string();
    let database = Database::open(&config).expect("open");

    let iterations = 200usize;
    let start = std::time::Instant::now();
    for index in 0..iterations {
        let record = CommandRecord {
            command: format!("echo bench-{index}"),
            command_hash: format!("hash-{index}"),
            cwd: Some("/tmp".to_string()),
            shell: Some("zsh".to_string()),
            username: Some("bench".to_string()),
            hostname: Some("localhost".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            session_id: Some("bench-session".to_string()),
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
        black_box(database.insert_command(&record).expect("insert"));
    }
    let elapsed = start.elapsed();
    let per_op_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    println!("record bench: {iterations} inserts in {elapsed:?} ({per_op_ms:.2} ms/op)");
}
