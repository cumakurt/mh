use std::fs;
use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json;

use crate::config::{self, AppConfig};
use crate::daemon::peer::{read_bounded_line, verify_peer_credentials};
use crate::daemon::protocol::{DaemonRequest, DaemonResponse};
use crate::daemon::{ensure_socket_parent, record_pid_path, record_socket_path};
use crate::db::Database;
use crate::errors::MhError;
use crate::git_detect;
use crate::record_pipeline;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
const MAX_DAEMON_CONNECTIONS: usize = 32;

struct ConnectionGuard;

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn run_daemon() -> Result<()> {
    let socket_path = record_socket_path();
    ensure_socket_parent(&socket_path)?;
    if socket_path.exists() {
        fs::remove_file(&socket_path)
            .with_context(|| format!("failed to remove stale socket {}", socket_path.display()))?;
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind daemon socket {}", socket_path.display()))?;
    config::restrict_file_permissions(&socket_path)?;
    listener
        .set_nonblocking(true)
        .context("failed to configure nonblocking daemon listener")?;

    write_pid_file()?;
    install_signal_handlers();

    let config = Arc::new(AppConfig::load()?);
    let database = Arc::new(Mutex::new(Database::open(config.as_ref())?));

    eprintln!("mh record daemon listening on {}", socket_path.display());

    while !SHUTDOWN.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                if ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed) >= MAX_DAEMON_CONNECTIONS {
                    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                    let _ = reject_connection(stream, "daemon at connection limit; retry later");
                    continue;
                }
                let db = Arc::clone(&database);
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    let _guard = ConnectionGuard;
                    if let Err(error) = handle_connection(stream, db, config) {
                        eprintln!("mh daemon connection error: {error:#}");
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                return Err(error).context("daemon accept failed");
            }
        }
    }

    for _ in 0..200 {
        if ACTIVE_CONNECTIONS.load(Ordering::Relaxed) == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    cleanup_socket_and_pid()?;
    Ok(())
}

pub fn start_daemon() -> Result<()> {
    if daemon_status()?.running {
        println!("mh record daemon is already running");
        return Ok(());
    }

    let exe = std::env::current_exe().context("failed to resolve mh executable path")?;
    std::process::Command::new(exe)
        .args(["daemon", "run"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to spawn mh daemon")?;

    for _ in 0..40 {
        if daemon_status()?.running {
            println!("mh record daemon started");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "daemon did not become ready; check {} and run `mh daemon run` for foreground diagnostics",
        record_socket_path().display()
    );
}

pub fn stop_daemon() -> Result<()> {
    let status = daemon_status()?;
    if !status.running {
        println!("mh record daemon is not running");
        cleanup_socket_and_pid().ok();
        return Ok(());
    }

    if let Some(pid) = status.pid {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        for _ in 0..40 {
            if !daemon_status()?.running {
                cleanup_socket_and_pid()?;
                println!("mh record daemon stopped");
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        bail!("daemon did not stop within timeout");
    }

    cleanup_socket_and_pid()?;
    Ok(())
}

#[derive(Debug)]
pub struct DaemonStatus {
    pub running: bool,
    pub socket_path: std::path::PathBuf,
    pub pid: Option<u32>,
}

pub fn install_systemd_unit() -> Result<()> {
    let exe = std::env::current_exe().context("failed to resolve mh executable path")?;
    let unit_dir = dirs::home_dir()
        .context("home directory is not available")?
        .join(".config/systemd/user");
    fs::create_dir_all(&unit_dir).with_context(|| {
        format!(
            "failed to create systemd user unit directory {}",
            unit_dir.display()
        )
    })?;

    let unit_path = unit_dir.join("mh-record-daemon.service");
    let unit_body = format!(
        r#"[Unit]
Description=mh record daemon (shell history)
Documentation=https://github.com/cumakurt/mh
After=default.target

[Service]
Type=simple
ExecStart={} daemon run
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
"#,
        exe.display()
    );
    fs::write(&unit_path, &unit_body).with_context(|| {
        format!(
            "failed to write systemd unit {}",
            unit_path.display()
        )
    })?;

    println!("Wrote {}", unit_path.display());
    println!("Enable with:");
    println!("  systemctl --user daemon-reload");
    println!("  systemctl --user enable --now mh-record-daemon.service");
    println!("Check status with: mh daemon status");
    Ok(())
}

pub fn daemon_status() -> Result<DaemonStatus> {
    let socket_path = record_socket_path();
    let pid = read_pid_file().ok();
    let ping_ok = socket_path.exists() && crate::daemon::is_daemon_available();
    let pid_alive = pid.is_some_and(pid_is_alive);
    let running = ping_ok && (pid.is_none() || pid_alive);
    Ok(DaemonStatus {
        running,
        socket_path,
        pid,
    })
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}

fn handle_connection(
    stream: UnixStream,
    database: Arc<Mutex<Database>>,
    config: Arc<AppConfig>,
) -> Result<()> {
    verify_peer_credentials(&stream)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set read timeout")?;
    let mut reader = BufReader::new(stream);
    let line = read_bounded_line(&mut reader)?;

    let request: DaemonRequest =
        serde_json::from_str(line.trim()).context("failed to decode daemon request")?;

    let response = match request {
        DaemonRequest::Ping => DaemonResponse::success(),
        DaemonRequest::Record { payload } => {
            let options = precompute_record_options(payload.as_ref());
            let database = database
                .lock()
                .map_err(|_| anyhow::anyhow!("daemon database lock poisoned"))?;
            match record_pipeline::execute_with_options(
                config.as_ref(),
                &database,
                payload.as_ref(),
                options,
            ) {
                Ok(()) => DaemonResponse::success(),
                Err(error) if is_policy_denied(&error) => DaemonResponse::success(),
                Err(error) => DaemonResponse::failure(error.to_string()),
            }
        }
    };

    let mut stream = reader.into_inner();
    let body = serde_json::to_string(&response)?;
    writeln!(stream, "{body}")?;
    stream.flush()?;
    Ok(())
}

fn write_pid_file() -> Result<()> {
    let pid_path = record_pid_path();
    if let Some(parent) = pid_path.parent() {
        config::ensure_private_directory(parent)?;
    }
    fs::write(&pid_path, process::id().to_string()).with_context(|| {
        format!(
            "failed to write daemon pid file {}",
            pid_path.display()
        )
    })?;
    config::restrict_file_permissions(&pid_path)?;
    Ok(())
}

fn read_pid_file() -> Result<u32> {
    let contents = fs::read_to_string(record_pid_path())?;
    contents
        .trim()
        .parse::<u32>()
        .context("invalid daemon pid file")
}

fn cleanup_socket_and_pid() -> Result<()> {
    let socket = record_socket_path();
    if socket.exists() {
        fs::remove_file(&socket).ok();
    }
    let pid_path = record_pid_path();
    if pid_path.exists() {
        fs::remove_file(pid_path).ok();
    }
    Ok(())
}

fn install_signal_handlers() {
    SHUTDOWN.store(false, Ordering::Relaxed);
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handle_signal as *const () as usize;
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
    }
}

extern "C" fn handle_signal(_: i32) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

fn precompute_record_options(payload: &record_pipeline::RecordPayload) -> record_pipeline::RecordOptions {
    let cwd = payload.cwd.clone().or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().to_string())
    });
    let git = cwd
        .as_deref()
        .filter(|_| std::env::var("MH_SKIP_GIT_DETECT").is_err())
        .and_then(|path| {
            if git_detect::is_git_repository(path) {
                git_detect::detect_git_context_cached(path)
            } else {
                None
            }
        });
    record_pipeline::RecordOptions::for_daemon().with_precomputed_git(git)
}

fn reject_connection(mut stream: UnixStream, message: &str) -> Result<()> {
    verify_peer_credentials(&stream)?;
    let response = DaemonResponse::failure(message);
    let body = serde_json::to_string(&response)?;
    writeln!(stream, "{body}")?;
    stream.flush()?;
    Ok(())
}

fn is_policy_denied(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| matches!(cause.downcast_ref::<MhError>(), Some(MhError::PolicyDenied(_))))
}
