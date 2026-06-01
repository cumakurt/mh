use std::io::Write;

use mh::cli::ImportArgs;
use mh::commands::import_history;

#[test]
fn import_rejects_invalid_csv_timestamp() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let csv_path = temp_dir.path().join("bad.csv");
    let mut file = std::fs::File::create(&csv_path).expect("create csv");
    writeln!(
        file,
        "id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags"
    )
    .expect("header");
    writeln!(file, "1,not-a-date,0,5,/tmp,zsh,,echo hello,").expect("row");

    let error = import_history::run(ImportArgs {
        file: csv_path.to_string_lossy().to_string(),
        merge: false,
        dry_run: true,
    })
    .expect_err("invalid timestamp should fail");

    assert!(error.to_string().contains("invalid import timestamp"));
}
