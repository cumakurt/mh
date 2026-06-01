use std::hint::black_box;

use mh::config::AppConfig;
use mh::db::Database;
use mh::record_pipeline::{RecordOptions, RecordPayload, execute_with_options};

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
        let payload = RecordPayload {
            command: format!("echo pipeline-bench-{index}"),
            cwd: Some("/tmp".to_string()),
            shell: Some("zsh".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5),
            started_at: None,
            finished_at: None,
            session_id: Some("bench-session".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        };
        execute_with_options(&config, &database, &payload, RecordOptions::default())
            .expect("record");
        black_box(());
    }
    let elapsed = start.elapsed();
    let per_op_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    println!("record pipeline bench: {iterations} records in {elapsed:?} ({per_op_ms:.2} ms/op)");
}
