pub mod bash;
pub mod common;
pub mod detect;
pub mod fish;
pub mod hooks;
pub mod nushell;
pub mod paths;
pub mod pwsh;
pub mod sh;
pub mod zsh;

pub use detect::{cli_name, kind_from_env, kind_from_path, resolve_init_shell};
pub use paths::{config_candidates, resolve_config_path};

use crate::cli::ShellKind;

pub fn integration(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Auto => integration(
            detect::kind_from_env().unwrap_or(ShellKind::Sh),
        ),
        ShellKind::Bash => bash::INTEGRATION,
        ShellKind::Zsh => zsh::INTEGRATION,
        ShellKind::Fish => fish::INTEGRATION,
        ShellKind::Nushell => nushell::INTEGRATION,
        ShellKind::Sh if detect::sh_emits_bash_integration() => bash::INTEGRATION,
        ShellKind::Sh => sh::INTEGRATION,
        ShellKind::Pwsh => pwsh::INTEGRATION,
    }
}
