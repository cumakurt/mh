pub mod bash;
pub mod common;
pub mod fish;
pub mod hooks;
pub mod nushell;
pub mod paths;
pub mod zsh;

pub use paths::{config_candidates, resolve_config_path};

use crate::cli::ShellKind;

pub fn integration(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => bash::INTEGRATION,
        ShellKind::Zsh => zsh::INTEGRATION,
        ShellKind::Fish => fish::INTEGRATION,
        ShellKind::Nushell => nushell::INTEGRATION,
    }
}
