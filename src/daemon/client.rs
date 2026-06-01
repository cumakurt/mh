use std::env;
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::cli::RecordArgs;
use crate::daemon::protocol::{DaemonRequest, DaemonResponse};
use crate::daemon::record_socket_path;
use crate::record_pipeline::RecordPayload;

/// Keep shell hooks responsive; direct SQLite fallback is only used when no daemon is present.
const IO_TIMEOUT_MS: u64 = 750;

#[derive(Debug)]
pub enum DaemonError {
    Unavailable,
    Failed(String),
}

pub fn is_daemon_available() -> bool {
    if env::var("MH_NO_DAEMON").is_ok() {
        return false;
    }
    let path = record_socket_path();
    if !path.exists() {
        return false;
    }
    ping(&path).is_ok()
}

pub fn record_via_daemon(args: &RecordArgs) -> Result<(), DaemonError> {
    if env::var("MH_NO_DAEMON").is_ok() {
        return Err(DaemonError::Unavailable);
    }
    let path = record_socket_path();
    if !path.exists() {
        return Err(DaemonError::Unavailable);
    }

    let request = DaemonRequest::Record {
        payload: Box::new(RecordPayload::from(args)),
    };
    let response = exchange(&path, &request).map_err(map_daemon_exchange_error)?;
    if response.ok {
        Ok(())
    } else {
        Err(DaemonError::Failed(
            response
                .error
                .unwrap_or_else(|| "daemon rejected record".to_string()),
        ))
    }
}

fn ping(path: &std::path::Path) -> Result<()> {
    let response = exchange(path, &DaemonRequest::Ping)?;
    if response.ok {
        Ok(())
    } else {
        anyhow::bail!("daemon ping failed")
    }
}

fn exchange(path: &std::path::Path, request: &DaemonRequest) -> Result<DaemonResponse> {
    let mut stream = UnixStream::connect(path).context("failed to connect to mh record daemon")?;
    stream
        .set_read_timeout(Some(Duration::from_millis(IO_TIMEOUT_MS)))
        .context("failed to set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_millis(IO_TIMEOUT_MS)))
        .context("failed to set write timeout")?;

    let body = serde_json::to_string(request).context("failed to encode daemon request")?;
    writeln!(stream, "{body}").context("failed to write daemon request")?;
    stream.flush().context("failed to flush daemon request")?;

    let mut reader = BufReader::new(stream);
    let line = crate::daemon::peer::read_bounded_response(&mut reader)?;
    serde_json::from_str(line.trim()).context("failed to decode daemon response")
}

fn map_daemon_exchange_error(error: anyhow::Error) -> DaemonError {
    let mut has_unavailable_io = false;
    for io in error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
    {
        match io.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                return DaemonError::Failed(format!(
                    "daemon did not respond within {IO_TIMEOUT_MS} ms; run: mh doctor"
                ));
            }
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::NotFound
            | std::io::ErrorKind::BrokenPipe => has_unavailable_io = true,
            _ => {}
        }
    }

    if has_unavailable_io {
        DaemonError::Unavailable
    } else {
        DaemonError::Failed(format!("daemon request failed: {error:#}"))
    }
}
