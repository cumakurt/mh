use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::{Cli, CompletionArgs, CompletionShell};
use crate::config;

pub fn run(args: CompletionArgs) -> Result<()> {
    let completion = generate_completion(args.shell);
    if let Some(path) = args.output {
        write_file(&path, &completion)?;
    } else {
        io::stdout().write_all(&completion)?;
    }
    Ok(())
}

pub fn generate_completion(shell: CompletionShell) -> Vec<u8> {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    generate(to_clap_shell(shell), &mut command, "mh", &mut buffer);
    buffer
}

fn to_clap_shell(shell: CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::PowerShell => Shell::PowerShell,
        CompletionShell::Elvish => Shell::Elvish,
    }
}

fn write_file(path: &str, content: &[u8]) -> Result<()> {
    config::write_private_file(Path::new(path), content)
        .with_context(|| format!("failed to write completion file {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_bash_completion() {
        let content = generate_completion(CompletionShell::Bash);
        let content = String::from_utf8(content).expect("completion should be UTF-8");

        assert!(content.contains("mh"));
        assert!(content.contains("completions"));
    }
}
