use std::io::{BufRead, BufReader};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result, bail};

/// Maximum daemon request payload size (JSON line).
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
/// Maximum daemon response payload size (JSON line).
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Rejects connections from a different effective UID than the daemon owner.
pub fn verify_peer_credentials(stream: &UnixStream) -> Result<()> {
    let peer_uid = peer_uid(stream)?;
    let expected = unsafe { libc::geteuid() };
    if peer_uid != expected {
        bail!("daemon rejected connection from uid {peer_uid} (expected {expected})");
    }
    Ok(())
}

pub fn read_bounded_line(reader: &mut BufReader<UnixStream>) -> Result<String> {
    read_bounded_line_limited(reader, MAX_REQUEST_BYTES, "request")
}

pub fn read_bounded_response(reader: &mut impl BufRead) -> Result<String> {
    read_bounded_line_limited(reader, MAX_RESPONSE_BYTES, "response")
}

fn read_bounded_line_limited(
    reader: &mut impl BufRead,
    max_bytes: usize,
    label: &str,
) -> Result<String> {
    let mut buffer = Vec::new();
    loop {
        let byte = reader
            .fill_buf()
            .with_context(|| format!("failed to read daemon {label}"))?;
        if byte.is_empty() {
            if buffer.is_empty() {
                bail!("daemon client closed connection without sending a {label}");
            }
            bail!("daemon {label} is missing a trailing newline");
        }
        let consumed = 1;
        let ch = byte[0];
        reader.consume(consumed);
        if ch == b'\n' {
            break;
        }
        if buffer.len() >= max_bytes {
            bail!("daemon {label} exceeds maximum size of {max_bytes} bytes");
        }
        buffer.push(ch);
    }

    String::from_utf8(buffer).with_context(|| format!("daemon {label} is not valid UTF-8"))
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Result<u32> {
    use std::mem;

    let mut cred: libc::ucred = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&cred) as u32;
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if status != 0 {
        bail!("failed to read peer credentials from daemon socket");
    }
    Ok(cred.uid)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn peer_uid(stream: &UnixStream) -> Result<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if status != 0 {
        bail!("failed to read peer credentials from daemon socket");
    }
    Ok(uid)
}

#[cfg(not(unix))]
fn peer_uid(_stream: &UnixStream) -> Result<u32> {
    Ok(unsafe { libc::geteuid() })
}
