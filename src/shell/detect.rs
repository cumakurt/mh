//! Map `$SHELL` paths and binary names to supported integration kinds.

use std::path::{Path, PathBuf};

use crate::cli::ShellKind;

/// Classification for a shell executable path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellExecutable {
    /// Full interactive hook integration (`mh init <kind>`).
    Supported(ShellKind),
    /// Same hooks as Bash (restricted bash).
    BashCompatible,
    /// Terminal multiplexer — use hooks for the shell inside the pane.
    Multiplexer,
    /// No automatic hooks; `mh record` and CLI still work.
    Unsupported,
}

/// Well-known paths seen on Linux (Debian, Kali, RHEL, containers).
pub const KNOWN_SHELL_PATHS: &[&str] = &[
    "/bin/sh",
    "/usr/bin/sh",
    "/bin/bash",
    "/usr/bin/bash",
    "/bin/rbash",
    "/usr/bin/rbash",
    "/usr/bin/dash",
    "/bin/zsh",
    "/usr/bin/zsh",
    "/usr/bin/fish",
    "/usr/bin/nu",
    "/usr/bin/pwsh",
    "/opt/microsoft/powershell/7/pwsh",
];

/// Human-readable list for doctor / help output.
pub const SUPPORTED_HOOK_SHELLS: &[&str] = &[
    "bash (includes /bin/bash, /usr/bin/bash, rbash)",
    "zsh (/bin/zsh, /usr/bin/zsh)",
    "fish",
    "nushell (nu)",
    "sh / dash / posix-sh (fc history hook; see `mh init sh`)",
    "pwsh (PowerShell 7+ with PSReadLine)",
];

pub fn executable_kind(path: &Path) -> ShellExecutable {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return ShellExecutable::Unsupported;
    };
    let lower = name.to_ascii_lowercase();
    classify_name(&lower)
}

pub fn kind_from_env() -> Option<ShellKind> {
    let shell = std::env::var("SHELL").ok()?;
    match executable_kind(Path::new(&shell)) {
        ShellExecutable::Supported(kind) => Some(kind),
        ShellExecutable::BashCompatible => Some(ShellKind::Bash),
        _ => None,
    }
}

pub fn kind_from_path(path: &str) -> Option<ShellKind> {
    match executable_kind(Path::new(path)) {
        ShellExecutable::Supported(kind) => Some(kind),
        ShellExecutable::BashCompatible => Some(ShellKind::Bash),
        _ => None,
    }
}

pub fn record_shell_name(path: &Path) -> String {
    match executable_kind(path) {
        ShellExecutable::Supported(ShellKind::Bash) | ShellExecutable::BashCompatible => {
            "bash".to_string()
        }
        ShellExecutable::Supported(kind) => cli_name(kind).to_string(),
        _ => path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown")
            .to_string(),
    }
}

pub fn cli_name(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Auto => "auto",
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::Fish => "fish",
        ShellKind::Nushell => "nushell",
        ShellKind::Sh => "sh",
        ShellKind::Pwsh => "pwsh",
    }
}

pub fn resolve_init_shell(shell: ShellKind) -> Result<ShellKind, String> {
    match shell {
        ShellKind::Auto => kind_from_env().ok_or_else(|| {
            format!(
                "could not map $SHELL to a supported integration; set SHELL or run mh init <shell> (supported: {})",
                SUPPORTED_HOOK_SHELLS.join(", ")
            )
        }),
        other => Ok(other),
    }
}

fn classify_name(name: &str) -> ShellExecutable {
    match name {
        "bash" => ShellExecutable::Supported(ShellKind::Bash),
        "rbash" => ShellExecutable::BashCompatible,
        "zsh" => ShellExecutable::Supported(ShellKind::Zsh),
        "fish" => ShellExecutable::Supported(ShellKind::Fish),
        "nu" | "nushell" => ShellExecutable::Supported(ShellKind::Nushell),
        "sh" | "dash" | "ash" | "busybox" => ShellExecutable::Supported(ShellKind::Sh),
        "pwsh" => ShellExecutable::Supported(ShellKind::Pwsh),
        "screen" | "tmux" => ShellExecutable::Multiplexer,
        _ => ShellExecutable::Unsupported,
    }
}

/// When `/bin/sh` is bash, prefer bash integration for DEBUG trap support.
pub fn sh_emits_bash_integration() -> bool {
    if std::env::var("BASH_VERSION").is_ok() {
        return true;
    }
    let sh = PathBuf::from("/bin/sh");
    if sh.exists()
        && let Ok(target) = std::fs::read_link(&sh)
    {
        let target = target.to_string_lossy().to_ascii_lowercase();
        return target.contains("bash");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_paths() {
        assert!(matches!(
            executable_kind(Path::new("/usr/bin/bash")),
            ShellExecutable::Supported(ShellKind::Bash)
        ));
        assert!(matches!(
            executable_kind(Path::new("/bin/rbash")),
            ShellExecutable::BashCompatible
        ));
        assert!(matches!(
            executable_kind(Path::new("/usr/bin/dash")),
            ShellExecutable::Supported(ShellKind::Sh)
        ));
        assert!(matches!(
            executable_kind(Path::new("/usr/bin/zsh")),
            ShellExecutable::Supported(ShellKind::Zsh)
        ));
        assert_eq!(
            executable_kind(Path::new("/usr/bin/tmux")),
            ShellExecutable::Multiplexer
        );
    }

    #[test]
    fn maps_rbash_to_bash_kind() {
        assert_eq!(kind_from_path("/usr/bin/rbash"), Some(ShellKind::Bash));
    }
}
