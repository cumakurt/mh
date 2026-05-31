use std::fs;

use anyhow::Result;

use crate::cli::{PrivateArgs, PrivateCommand};
use crate::config::{self, AppConfig, private_mode_path};
use crate::output::styling::Styler;
use crate::security;

pub fn run(args: PrivateArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let path = private_mode_path();
    match args.command {
        PrivateCommand::On => {
            if let Some(parent) = path.parent() {
                config::ensure_private_directory(parent)?;
            }
            config::write_private_file(&path, b"enabled\n")?;
            println!("{}", styler.warning("Private mode is enabled"));
        }
        PrivateCommand::Off => {
            if path.exists() {
                fs::remove_file(&path)?;
            }
            println!("{}", styler.success("Private mode is disabled"));
        }
        PrivateCommand::Status => {
            if security::private_mode_enabled(&config) {
                println!("{}", styler.warning("Private mode is enabled"));
            } else {
                println!("{}", styler.success("Private mode is disabled"));
            }
        }
    }
    Ok(())
}
