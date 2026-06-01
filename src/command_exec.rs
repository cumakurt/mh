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
        .filter(|path| path.is_absolute())
        .filter(|path| path.is_file())
        .unwrap_or_else(|| std::path::PathBuf::from("/bin/sh"))
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
}
