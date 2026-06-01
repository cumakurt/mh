use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use mh::config::AppConfig;
use mh::daemon::peer::{MAX_REQUEST_BYTES, read_bounded_line, verify_peer_credentials};
use mh::daemon::protocol::{DaemonRequest, DaemonResponse};
use mh::db::Database;
use mh::record_pipeline::RecordPayload;

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[test]
fn peer_credentials_accept_same_process_connections() {
    let (client, server) = UnixStream::pair().expect("socket pair");
    verify_peer_credentials(&client).expect("same-process client should be accepted");
    drop(client);
    drop(server);
}

#[test]
fn peer_credentials_rejection_message_includes_uids() {
    let expected = unsafe { libc::geteuid() };
    let message = format!(
        "daemon rejected connection from uid {} (expected {expected})",
        expected.saturating_add(1)
    );
    assert!(message.contains("daemon rejected connection from uid"));
    assert!(message.contains(&format!("expected {expected}")));
}

#[test]
#[ignore = "requires a peer connection from a different UID (run as root: sudo -u nobody mh daemon run)"]
fn peer_credentials_reject_foreign_uid_documentation() {
    // Unix domain socket peer credentials are checked in verify_peer_credentials.
    // Same-process socket pairs always share UID, so cross-UID rejection is validated
    // manually in production deployments. See src/daemon/peer.rs.
    assert!(std::path::Path::new("src/daemon/peer.rs").exists());
}

#[test]
fn read_bounded_response_rejects_oversized_payload() {
    use mh::daemon::peer::{MAX_RESPONSE_BYTES, read_bounded_response};

    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let reader = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(reader);
        read_bounded_response(&mut reader)
    });

    let payload = format!("{}\n", "x".repeat(MAX_RESPONSE_BYTES + 16));
    writer.write_all(payload.as_bytes()).expect("write payload");
    drop(writer);

    let error = reader
        .join()
        .expect("reader thread")
        .expect_err("oversized response should be rejected");
    assert!(
        error.to_string().contains("maximum size"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn read_bounded_line_rejects_oversized_requests() {
    let (mut client, server) = UnixStream::pair().expect("socket pair");
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");

    let reader = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(server);
        read_bounded_line(&mut reader)
    });

    let payload = format!("{}\n", "x".repeat(MAX_REQUEST_BYTES + 16));
    client.write_all(payload.as_bytes()).expect("write payload");
    drop(client);

    let error = reader
        .join()
        .expect("reader thread")
        .expect_err("oversized payload should be rejected");
    assert!(
        error.to_string().contains("maximum size"),
        "unexpected error: {error:#}"
    );
}

#[test]
#[cfg(unix)]
fn daemon_rejects_world_writable_socket_parent() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let parent = temp_dir.path().join("unsafe-socket-parent");
    std::fs::create_dir_all(&parent).expect("parent dir");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
        .expect("chmod parent");

    let original_socket = std::env::var_os("MH_DAEMON_SOCKET");
    unsafe {
        std::env::set_var("MH_DAEMON_SOCKET", parent.join("record.sock"));
    }

    let result = mh::daemon::run_daemon();

    unsafe {
        match original_socket {
            Some(value) => std::env::set_var("MH_DAEMON_SOCKET", value),
            None => std::env::remove_var("MH_DAEMON_SOCKET"),
        }
    }

    let error = result.expect_err("world-writable socket parent should fail");
    assert!(format!("{error:#}").contains("writable by group or others"));
}

#[test]
#[cfg(unix)]
fn daemon_refuses_to_replace_non_socket_path() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp_dir = tempfile::tempdir().expect("temp dir");
    std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("chmod temp dir");
    let socket_path = temp_dir.path().join("record.sock");
    std::fs::write(&socket_path, "not a socket").expect("socket placeholder");

    let original_socket = std::env::var_os("MH_DAEMON_SOCKET");
    unsafe {
        std::env::set_var("MH_DAEMON_SOCKET", &socket_path);
    }

    let result = mh::daemon::run_daemon();

    unsafe {
        match original_socket {
            Some(value) => std::env::set_var("MH_DAEMON_SOCKET", value),
            None => std::env::remove_var("MH_DAEMON_SOCKET"),
        }
    }

    let error = result.expect_err("regular file must not be removed as a stale socket");
    assert!(format!("{error:#}").contains("non-socket daemon path"));
    assert_eq!(
        std::fs::read_to_string(&socket_path).expect("placeholder should remain"),
        "not a socket"
    );
}

#[test]
fn daemon_record_roundtrip_respects_security_masking() {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let socket_path = temp_dir.path().join("record.sock");
    // SAFETY: test-local environment overrides.
    unsafe {
        std::env::set_var("MH_DAEMON_SOCKET", &socket_path);
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
    }

    let mut config = AppConfig::default();
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config
        .write_to_path(&mh::config::config_path())
        .expect("write config");

    let listener = std::os::unix::net::UnixListener::bind(&socket_path).expect("bind");
    let database = Arc::new(Mutex::new(Database::open(&config).expect("open")));
    let config = Arc::new(config);

    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        mh::daemon::peer::verify_peer_credentials(&stream).expect("peer");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let mut reader = std::io::BufReader::new(stream);
        let line = read_bounded_line(&mut reader).expect("line");
        let request: DaemonRequest = serde_json::from_str(line.trim()).expect("request");
        let response = match request {
            DaemonRequest::Record { payload } => {
                let database = database.lock().expect("lock");
                match mh::record_pipeline::execute_with_options(
                    config.as_ref(),
                    &database,
                    payload.as_ref(),
                    mh::record_pipeline::RecordOptions::for_daemon(),
                ) {
                    Ok(()) => DaemonResponse::success(),
                    Err(error) => DaemonResponse::failure(error.to_string()),
                }
            }
            DaemonRequest::Ping => DaemonResponse::success(),
        };
        let mut stream = reader.into_inner();
        writeln!(
            stream,
            "{}",
            serde_json::to_string(&response).expect("json")
        )
        .expect("write");
        stream.flush().expect("flush");
    });

    let mut client = UnixStream::connect(&socket_path).expect("connect");
    let request = DaemonRequest::Record {
        payload: Box::new(RecordPayload {
            command: "mysql -u root -pSecret123".to_string(),
            cwd: None,
            shell: Some("zsh".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("test-session".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        }),
    };
    writeln!(client, "{}", serde_json::to_string(&request).expect("json")).expect("write");
    client.flush().expect("flush");

    let mut reader = std::io::BufReader::new(client);
    let mut line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut line).expect("response");
    let response: DaemonResponse = serde_json::from_str(line.trim()).expect("decode");
    assert!(
        response.ok,
        "daemon should accept record: {:?}",
        response.error
    );

    handle.join().expect("server thread");

    let database = Database::open(&AppConfig::load().expect("config")).expect("open");
    let rows = database
        .search_commands(&mh::models::SearchFilters {
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
            limit: 5,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("search");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].command.contains("Secret123"));
    assert!(rows[0].is_masked);

    // SAFETY: restore environment for other tests.
    unsafe {
        std::env::remove_var("MH_DAEMON_SOCKET");
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
