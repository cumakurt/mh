mod client;
pub mod peer;
pub mod protocol;
mod server;

pub use client::{is_daemon_available, record_via_daemon, DaemonError};
pub use protocol::{DaemonRequest, DaemonResponse};
pub use server::{
    daemon_status, install_systemd_unit, run_daemon, start_daemon, stop_daemon,
};

use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Unix socket path for the record daemon.
pub fn record_socket_path() -> PathBuf {
    if let Ok(path) = env::var("MH_DAEMON_SOCKET") {
        return PathBuf::from(path);
    }
    if let Ok(runtime) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("mh").join("record.sock");
    }
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .map(|dir| dir.join("mh").join("record.sock"))
        .unwrap_or_else(|| PathBuf::from(".mh-record.sock"))
}

pub fn record_pid_path() -> PathBuf {
    record_socket_path()
        .parent()
        .map(|dir| dir.join("record.pid"))
        .unwrap_or_else(|| PathBuf::from("record.pid"))
}

pub(crate) fn ensure_socket_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        crate::config::ensure_private_directory(parent)?;
    }
    Ok(())
}
