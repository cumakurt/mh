use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dirs::home_dir;

use crate::cli::{InitArgs, ShellKind};
use crate::config::ensure_directory;
use crate::shell::{self, hooks, resolve_config_path};

pub fn run(args: InitArgs) -> Result<()> {
    let should_install = args.install || args.shell.is_none();
    let shell_arg = args.shell.unwrap_or(ShellKind::Auto);
    let shell = shell::resolve_init_shell(shell_arg).map_err(anyhow::Error::msg)?;
    if args.repair {
        return repair(shell);
    }
    if should_install {
        return install(shell);
    }

    print!("{}", shell::integration(shell));
    Ok(())
}

fn install(shell: ShellKind) -> Result<()> {
    let config_path = shell_config_path(shell)?;
    if let Some(parent) = config_path.parent() {
        ensure_directory(parent)?;
    }

    let original = fs::read_to_string(&config_path).unwrap_or_default();
    if original.contains(hooks::BEGIN_MARKER) {
        let hook_duplicates = hooks::duplicate_hook_count(shell, &original);
        if hook_duplicates == 0 {
            println!(
                "mh shell integration is already installed in {}",
                config_path.display()
            );
            print_activation_hint(shell, &config_path);
            return Ok(());
        }
        return repair(shell);
    }

    let hook_duplicates = hooks::duplicate_hook_count(shell, &original);
    if hook_duplicates > 0 {
        bail!(
            "possible duplicate mh shell integration detected in {} ({hook_duplicates} duplicate hook line(s)); run mh init {} --repair",
            config_path.display(),
            shell_cli_name(shell)
        );
    }

    for marker in MANUAL_HOOK_MARKERS {
        if original.contains(marker) {
            bail!(
                "existing mh hook marker '{marker}' found in {}; run mh init {} --repair",
                config_path.display(),
                shell_cli_name(shell)
            );
        }
    }

    let block = managed_block(shell);
    let mut updated = original;
    if !updated.ends_with('\n') && !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str(&block);

    backup_and_write(&config_path, &updated)?;

    println!(
        "Installed mh shell integration into {}",
        config_path.display()
    );
    print_activation_hint(shell, &config_path);
    Ok(())
}

fn repair(shell: ShellKind) -> Result<()> {
    let config_path = shell_config_path(shell)?;
    if !config_path.exists() {
        bail!(
            "shell config file not found: {} — run mh init {} --install",
            config_path.display(),
            shell_cli_name(shell)
        );
    }

    let original = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let (repaired, report) = hooks::repair_content(&original, shell);

    let duplicates_before = hooks::duplicate_hook_count(shell, &original);
    if !report.changed() && duplicates_before == 0 {
        println!("No shell hook repairs needed in {}", config_path.display());
        print_activation_hint(shell, &config_path);
        return Ok(());
    }

    let duplicates_after = hooks::duplicate_hook_count(shell, &repaired);
    if repaired == original {
        if duplicates_after > 0 {
            bail!(
                "could not remove {duplicates_after} duplicate mh hook registration(s) in {}; manual edit required",
                config_path.display()
            );
        }
        println!("No shell hook repairs needed in {}", config_path.display());
        print_activation_hint(shell, &config_path);
        return Ok(());
    }

    backup_and_write(&config_path, &repaired)?;

    println!("Repaired shell integration in {}", config_path.display());
    if report.removed_managed_blocks > 0 {
        println!(
            "Removed {} duplicate managed integration block(s)",
            report.removed_managed_blocks
        );
    }
    if report.removed_duplicate_hook_lines > 0 {
        println!(
            "Removed {} duplicate hook registration line(s)",
            report.removed_duplicate_hook_lines
        );
    }
    print_activation_hint(shell, &config_path);
    Ok(())
}

fn print_activation_hint(shell: ShellKind, config_path: &Path) {
    println!("New shell sessions will load mh automatically.");
    println!(
        "To activate mh in this terminal, run: {}",
        activation_command(shell, config_path)
    );
}

fn activation_command(shell: ShellKind, config_path: &Path) -> String {
    match shell {
        ShellKind::Bash | ShellKind::Zsh | ShellKind::Fish | ShellKind::Nushell => {
            format!("source {}", shell_quote(config_path))
        }
        ShellKind::Sh => format!(". {}", shell_quote(config_path)),
        ShellKind::Pwsh => format!(". {}", pwsh_quote(config_path)),
        ShellKind::Auto => unreachable!("resolved before activation command"),
    }
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn pwsh_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "''"))
}

fn backup_and_write(config_path: &PathBuf, content: &str) -> Result<()> {
    if config_path.exists() {
        let backup = format!(
            "{}.mh.bak.{}",
            config_path.display(),
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        fs::copy(config_path, &backup).with_context(|| format!("failed to back up {backup}"))?;
        eprintln!("Created config backup: {backup}");
    }

    crate::config::write_private_file(config_path, content.as_bytes()).with_context(|| {
        format!(
            "failed to write shell integration to {}",
            config_path.display()
        )
    })?;
    Ok(())
}

const MANUAL_HOOK_MARKERS: &[&str] = &[
    "__mh_preexec",
    "__mh_precmd",
    "_mh_preexec",
    "_mh_precmd",
    "mh_preexec",
    "mh_postexec",
    "fish_preexec",
    "fish_postexec",
    "mh_history_picker",
    "__mh_before_prompt",
    "__mh_pwsh_loaded",
];

fn managed_block(shell: ShellKind) -> String {
    match shell {
        ShellKind::Bash | ShellKind::Zsh => {
            let shell_name = match shell {
                ShellKind::Bash => "bash",
                ShellKind::Zsh => "zsh",
                _ => unreachable!(),
            };
            format!(
                "{}\nif command -v mh >/dev/null 2>&1; then\n  eval \"$(mh init {shell_name})\"\nfi\n{}\n",
                hooks::BEGIN_MARKER,
                hooks::END_MARKER
            )
        }
        ShellKind::Fish => format!(
            "{}\nif command -q mh\n  mh init fish | source\nend\n{}\n",
            hooks::BEGIN_MARKER,
            hooks::END_MARKER
        ),
        ShellKind::Nushell => format!(
            "{}\nif (which mh | is-not-empty) {{\n  (mh init nushell)\n}}\n{}\n",
            hooks::BEGIN_MARKER,
            hooks::END_MARKER
        ),
        ShellKind::Sh => {
            if shell::detect::sh_emits_bash_integration() {
                format!(
                    "{}\nif command -v mh >/dev/null 2>&1; then\n  eval \"$(mh init bash)\"\nfi\n{}\n",
                    hooks::BEGIN_MARKER,
                    hooks::END_MARKER
                )
            } else {
                format!(
                    "{}\n{}\n{}\n",
                    hooks::BEGIN_MARKER,
                    shell::sh::INTEGRATION,
                    hooks::END_MARKER
                )
            }
        }
        ShellKind::Pwsh => format!(
            "{}\n{}\n{}\n",
            hooks::BEGIN_MARKER,
            shell::pwsh::INTEGRATION,
            hooks::END_MARKER
        ),
        ShellKind::Auto => unreachable!("resolved before managed_block"),
    }
}

fn shell_cli_name(shell: ShellKind) -> &'static str {
    shell::cli_name(shell)
}

pub fn shell_config_path(shell: ShellKind) -> Result<PathBuf> {
    let home = home_dir().context("could not determine home directory")?;
    Ok(resolve_config_path(shell, &home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_block_contains_init_command() {
        let block = managed_block(ShellKind::Zsh);
        assert!(block.contains("mh init zsh"));
        assert!(block.contains(hooks::BEGIN_MARKER));
    }

    #[test]
    fn install_targets_existing_bash_profile() {
        let home = tempfile::tempdir().expect("tempdir");
        let profile = home.path().join(".bash_profile");
        std::fs::write(&profile, "# login shell\n").expect("write");
        let resolved = crate::shell::resolve_config_path(ShellKind::Bash, home.path());
        assert_eq!(resolved, profile);
    }

    #[test]
    fn activation_command_quotes_shell_paths() {
        let path = PathBuf::from("/tmp/mh test/.zshrc");
        assert_eq!(
            activation_command(ShellKind::Zsh, &path),
            "source '/tmp/mh test/.zshrc'"
        );

        let sh_path = PathBuf::from("/tmp/mh test/.profile");
        assert_eq!(
            activation_command(ShellKind::Sh, &sh_path),
            ". '/tmp/mh test/.profile'"
        );
    }
}
