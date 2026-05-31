use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use clap::CommandFactory;

use crate::cli::{Cli, ManArgs};
use crate::config;

pub fn run(args: ManArgs) -> Result<()> {
    let man_page = generate_man_page()?;
    if let Some(path) = args.output {
        write_file(&path, &man_page)?;
    } else {
        io::stdout().write_all(&man_page)?;
    }
    Ok(())
}

pub fn generate_man_page() -> Result<Vec<u8>> {
    let command = Cli::command();
    let mut buffer = Vec::new();
    clap_mangen::Man::new(command).render(&mut buffer)?;
    Ok(buffer)
}

fn write_file(path: &str, content: &[u8]) -> Result<()> {
    config::write_private_file(Path::new(path), content)
        .with_context(|| format!("failed to write man page file {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_man_page() {
        let content = generate_man_page().expect("man page should render");
        let content = String::from_utf8(content).expect("man page should be UTF-8");

        assert!(content.contains(".TH"));
        assert!(content.contains("mh"));
    }
}
