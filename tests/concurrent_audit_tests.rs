use std::sync::{Arc, Barrier};
use std::thread;

use mh::config::AppConfig;
use mh::db::Database;

#[test]
fn concurrent_audit_inserts_preserve_hash_chain() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    Database::open(&config).expect("bootstrap database");

    let config = Arc::new(config);
    let workers = 8usize;
    let barrier = Arc::new(Barrier::new(workers));

    let handles: Vec<_> = (0..workers)
        .map(|index| {
            let config = config.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                let database = Database::open(&config).expect("open");
                database
                    .insert_audit_log(
                        "risky",
                        &format!("command-{index}"),
                        "test",
                        Some("user"),
                        Some("host"),
                    )
                    .expect("audit insert");
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("worker");
    }

    let database = Database::open(&config).expect("verify");
    database.verify_audit_chain().expect("valid chain");
}
