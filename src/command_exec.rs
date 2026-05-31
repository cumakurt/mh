use std::path::Path;
use std::process::{Command, ExitStatus};

use anyhow::{Context, Result};

/// Run a shell command string without invoking a login shell (`-l`).
/// When `cwd` is set and exists, execution starts in that directory.
pub fn execute_shell_command(command: &str, cwd: Option<&Path>) -> Result<ExitStatus> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(command);

    if let Some(dir) = cwd.filter(|path| path.is_dir()) {
        cmd.current_dir(dir);
    }

    cmd.status()
        .with_context(|| format!("failed to execute command via {shell}"))
}
