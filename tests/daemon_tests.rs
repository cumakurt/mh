use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

use mh::daemon::{DaemonRequest, DaemonResponse};
use mh::record_pipeline::RecordPayload;
#[test]
fn daemon_ping_and_record_roundtrip() {
    let dir = mh::config::private_tempdir().expect("temp dir");
    let config_dir = dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config.toml");
    let db_path = dir.path().join("history.db");

    let config_body = format!(
        r#"
[database]
path = "{}"

[history]
max_entries = 1000
"#,
        db_path.display()
    );
    fs::write(&config_path, config_body).expect("write config");

    let saved = [
        ("XDG_RUNTIME_DIR", std::env::var("XDG_RUNTIME_DIR").ok()),
        ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
        ("MH_CONFIG", std::env::var("MH_CONFIG").ok()),
        (
            "MH_CONFIG_NO_CACHE",
            std::env::var("MH_CONFIG_NO_CACHE").ok(),
        ),
    ];
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("MH_CONFIG", &config_path);
        std::env::set_var("MH_CONFIG_NO_CACHE", "1");
    }

    let handle = thread::spawn(|| mh::daemon::run_daemon().expect("daemon run"));
    let socket = mh::daemon::record_socket_path();

    for _ in 0..80 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(socket.exists(), "daemon socket was not created");

    let ping = exchange(&socket, &DaemonRequest::Ping).expect("ping");
    assert!(ping.ok);

    let record = exchange(
        &socket,
        &DaemonRequest::Record {
            payload: Box::new(RecordPayload {
                command: "echo daemon-test".to_string(),
                cwd: Some(dir.path().to_string_lossy().to_string()),
                shell: Some("test".to_string()),
                exit_code: Some(0),
                duration_ms: Some(1),
                started_at: None,
                finished_at: None,
                session_id: Some("test-session".to_string()),
                tty: None,
                tags: None,
                env_context: None,
            }),
        },
    )
    .expect("record");
    assert!(record.ok, "record failed: {:?}", record.error);

    assert!(
        db_path.exists(),
        "daemon should write to configured database"
    );
    let config = mh::config::AppConfig::load().expect("load config");
    let database = mh::db::Database::open(&config).expect("open db");
    assert_eq!(
        database.count_commands().expect("count"),
        1,
        "record should persist in isolated database"
    );

    if let Ok(pid_contents) = fs::read_to_string(mh::daemon::record_pid_path())
        && let Ok(pid) = pid_contents.trim().parse::<i32>()
    {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
    let _ = handle.join();

    unsafe {
        for (key, value) in saved {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn exchange(socket: &std::path::Path, request: &DaemonRequest) -> anyhow::Result<DaemonResponse> {
    let mut stream = UnixStream::connect(socket)?;
    let body = serde_json::to_string(request)?;
    writeln!(stream, "{body}")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(serde_json::from_str(line.trim())?)
}
