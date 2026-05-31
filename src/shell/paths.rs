use std::path::{Path, PathBuf};

use crate::cli::ShellKind;

/// Preferred shell config file paths (first existing wins, else first entry).
pub fn config_candidates(shell: ShellKind, home: &Path) -> Vec<PathBuf> {
    let xdg = env_config_dir(home);
    match shell {
        ShellKind::Bash => vec![
            home.join(".bashrc"),
            home.join(".bash_profile"),
            home.join(".profile"),
        ],
        ShellKind::Zsh => vec![home.join(".zshrc"), home.join(".zshenv")],
        ShellKind::Fish => vec![xdg.join("fish").join("config.fish")],
        ShellKind::Nushell => vec![xdg.join("nushell").join("config.nu")],
    }
}

/// Resolves the shell config path: first existing candidate, otherwise the default.
pub fn resolve_config_path(shell: ShellKind, home: &Path) -> PathBuf {
    config_candidates(shell, home)
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| default_config_path(shell, home))
}

pub fn default_config_path(shell: ShellKind, home: &Path) -> PathBuf {
    config_candidates(shell, home)
        .into_iter()
        .next()
        .unwrap_or_else(|| home.join(".profile"))
}

fn env_config_dir(home: &Path) -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_prefers_bashrc_when_present() {
        let home = tempfile::tempdir().expect("tempdir");
        let bashrc = home.path().join(".bashrc");
        std::fs::write(&bashrc, "# bash").expect("write");
        let resolved = resolve_config_path(ShellKind::Bash, home.path());
        assert_eq!(resolved, bashrc);
    }

    #[test]
    fn bash_falls_back_to_bash_profile() {
        let home = tempfile::tempdir().expect("tempdir");
        let profile = home.path().join(".bash_profile");
        std::fs::write(&profile, "# profile").expect("write");
        let resolved = resolve_config_path(ShellKind::Bash, home.path());
        assert_eq!(resolved, profile);
    }

    #[test]
    fn zsh_defaults_to_zshrc_when_missing() {
        let home = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_config_path(ShellKind::Zsh, home.path());
        assert_eq!(resolved, home.path().join(".zshrc"));
    }
}
