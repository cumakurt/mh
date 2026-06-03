use std::path::Path;
use std::process::{Command, ExitStatus};

use anyhow::{Context, Result};

/// Run a shell command string without invoking a login shell (`-l`).
/// When `cwd` is set and exists, execution starts in that directory.
pub fn execute_shell_command(command: &str, cwd: Option<&Path>) -> Result<ExitStatus> {
    let shell = resolve_shell();
    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(command);

    if let Some(dir) = cwd.filter(|path| path.is_dir()) {
        cmd.current_dir(dir);
    }

    cmd.status()
        .with_context(|| format!("failed to execute command via {}", shell.display()))
}

fn resolve_shell() -> std::path::PathBuf {
    std::env::var_os("SHELL")
        .map(std::path::PathBuf::from)
        .filter(|path| is_trusted_shell_path(path))
        .unwrap_or_else(|| std::path::PathBuf::from("/bin/sh"))
}

fn is_trusted_shell_path(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() || !is_executable_file(path) {
        return false;
    }

    #[cfg(unix)]
    {
        let Ok(canonical) = path.canonicalize() else {
            return false;
        };
        if !canonical.is_file() || !is_executable_file(&canonical) {
            return false;
        }
        !path_chain_is_group_or_other_writable(path)
            && !path_chain_is_group_or_other_writable(&canonical)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
fn path_chain_is_group_or_other_writable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    };

    loop {
        if current
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o022 != 0)
            .unwrap_or(true)
        {
            return true;
        }
        if !current.pop() {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn resolve_shell_ignores_relative_shell_env() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::var_os("SHELL");
        unsafe {
            std::env::set_var("SHELL", "sh");
        }

        assert_eq!(resolve_shell(), std::path::PathBuf::from("/bin/sh"));

        unsafe {
            match original {
                Some(value) => std::env::set_var("SHELL", value),
                None => std::env::remove_var("SHELL"),
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn resolve_shell_rejects_world_writable_shell_parent() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::var_os("SHELL");
        let temp_dir = crate::config::private_tempdir().expect("temp dir");
        let unsafe_parent = temp_dir.path().join("unsafe");
        std::fs::create_dir_all(&unsafe_parent).expect("unsafe parent");
        std::fs::set_permissions(&unsafe_parent, std::fs::Permissions::from_mode(0o777))
            .expect("chmod unsafe parent");
        let shell = unsafe_parent.join("sh");
        std::fs::write(&shell, b"#!/bin/sh\n").expect("write shell");
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755))
            .expect("chmod shell");

        unsafe {
            std::env::set_var("SHELL", &shell);
        }

        assert_eq!(resolve_shell(), std::path::PathBuf::from("/bin/sh"));

        unsafe {
            match original {
                Some(value) => std::env::set_var("SHELL", value),
                None => std::env::remove_var("SHELL"),
            }
        }
    }
}
