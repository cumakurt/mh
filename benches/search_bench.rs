use std::hint::black_box;
use std::time::Instant;

use mh::config::AppConfig;
use mh::db::Database;
use mh::models::{CommandRecord, SearchFilters};

fn bench_size() -> usize {
    std::env::var("MH_BENCH_SEARCH_SIZE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000)
}

fn empty_filters() -> SearchFilters {
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
        limit: 50,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    }
}

fn main() {
    let size = bench_size();
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("search.db")
        .to_string_lossy()
        .to_string();
    let database = Database::open(&config).expect("open");

    let insert_start = Instant::now();
    for index in 0..size {
        let record = CommandRecord {
            command: format!("echo search-bench-{index}"),
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
        database.insert_command(&record).expect("insert");
    }
    let insert_elapsed = insert_start.elapsed();
    println!("search bench: inserted {size} rows in {insert_elapsed:?}");

    let mut fts_filters = empty_filters();
    fts_filters.query = Some("search-bench-42".to_string());
    fts_filters.fts = true;
    fts_filters.limit = 20;

    let fts_start = Instant::now();
    for _ in 0..50 {
        black_box(database.search_commands(&fts_filters).expect("fts search"));
    }
    let fts_elapsed = fts_start.elapsed();
    let fts_ms = fts_elapsed.as_secs_f64() * 1000.0 / 50.0;
    println!("search bench: FTS search avg {fts_ms:.2} ms/op (50 iterations)");

    let last_filters = empty_filters();
    let last_start = Instant::now();
    for _ in 0..50 {
        black_box(
            database
                .search_commands(&last_filters)
                .expect("last search"),
        );
    }
    let last_elapsed = last_start.elapsed();
    let last_ms = last_elapsed.as_secs_f64() * 1000.0 / 50.0;
    println!("search bench: last-50 avg {last_ms:.2} ms/op (50 iterations)");

    let mut fuzzy_filters = empty_filters();
    fuzzy_filters.query = Some("dcl".to_string());
    fuzzy_filters.fuzzy = true;
    fuzzy_filters.limit = 20;

    let fuzzy_start = Instant::now();
    for _ in 0..50 {
        black_box(
            database
                .search_commands(&fuzzy_filters)
                .expect("fuzzy search"),
        );
    }
    let fuzzy_elapsed = fuzzy_start.elapsed();
    let fuzzy_ms = fuzzy_elapsed.as_secs_f64() * 1000.0 / 50.0;
    println!("search bench: fuzzy search avg {fuzzy_ms:.2} ms/op (50 iterations)");

    if std::env::var("MH_BENCH_ASSERT").is_ok() {
        let max_fts_ms = env_limit("MH_BENCH_MAX_FTS_MS", 50.0);
        let max_last_ms = env_limit("MH_BENCH_MAX_LAST_MS", 20.0);
        let max_fuzzy_ms = env_limit("MH_BENCH_MAX_FUZZY_MS", 100.0);
        assert!(
            fts_ms <= max_fts_ms,
            "FTS search too slow: {fts_ms:.2} ms/op (max {max_fts_ms})"
        );
        assert!(
            last_ms <= max_last_ms,
            "last search too slow: {last_ms:.2} ms/op (max {max_last_ms})"
        );
        assert!(
            fuzzy_ms <= max_fuzzy_ms,
            "fuzzy search too slow: {fuzzy_ms:.2} ms/op (max {max_fuzzy_ms})"
        );
    }
}

fn env_limit(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
