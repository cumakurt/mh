use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use crate::cli::{ConfigArgs, ConfigCommand};
use crate::config::{AppConfig, ConfigFixReport, config_path};

pub fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Show => {
            let config = AppConfig::load()?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
        ConfigCommand::Path => {
            println!("{}", config_path().display());
        }
        ConfigCommand::Edit => {
            let path = config_path();
            if !path.exists() {
                AppConfig::default().write_to_path(&path)?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = Command::new(editor).arg(&path).status()?;
            if !status.success() {
                anyhow::bail!("editor exited with status {status}");
            }
        }
        ConfigCommand::Set { key, value } => {
            let path = config_path();
            let mut toml_value = if path.exists() {
                let content = fs::read_to_string(&path)?;
                let config: AppConfig = toml::from_str(&content)?;
                toml::Value::try_from(config)?
            } else {
                toml::Value::try_from(AppConfig::default())?
            };
            set_value(&mut toml_value, &key, parse_value(&value))?;
            let config: AppConfig = toml_value.try_into()?;
            config.write_to_path(&path)?;
            println!("Updated {key}");
        }
        ConfigCommand::Reset => {
            let path = config_path();
            AppConfig::default().write_to_path(&path)?;
            println!("Config reset to defaults");
        }
        ConfigCommand::Validate => {
            let config = AppConfig::load()?;
            crate::security::SecurityEngine::from_config(&config)
                .context("security ignore patterns failed validation")?;
            crate::policy::PolicyEngine::from_config(&config)
                .context("policy rules failed validation")?;
            let warnings = crate::config::legacy_ignore_pattern_warnings(&config);
            for warning in &warnings {
                eprintln!("warning: {warning}");
            }
            if warnings.is_empty() {
                println!("Config is valid");
            } else {
                println!(
                    "Config is valid with {} legacy ignore pattern warning(s) — run mh config fix",
                    warnings.len()
                );
            }
        }
        ConfigCommand::Fix => {
            let report = crate::config::fix_local_config()?;
            if report.tightened_config_dir {
                println!("Tightened config directory permissions");
            }
            if report.tightened_config_file {
                println!("Tightened config file permissions");
            }
            if report.removed_legacy_patterns > 0 {
                println!(
                    "Removed {} legacy ignore pattern(s)",
                    report.removed_legacy_patterns
                );
            }
            if report == ConfigFixReport::default() {
                println!("No config fixes were needed");
            }
        }
    }

    Ok(())
}

fn parse_value(value: &str) -> toml::Value {
    if let Ok(value) = value.parse::<bool>() {
        toml::Value::Boolean(value)
    } else if let Ok(value) = value.parse::<i64>() {
        toml::Value::Integer(value)
    } else {
        toml::Value::String(value.to_string())
    }
}

fn set_value(root: &mut toml::Value, key: &str, value: toml::Value) -> Result<()> {
    let parts = key.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        anyhow::bail!("config key must not be empty");
    }

    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        current = current
            .as_table_mut()
            .and_then(|table| table.get_mut(*part))
            .ok_or_else(|| anyhow::anyhow!("unknown config section: {part}"))?;
    }

    let last = parts[parts.len() - 1];
    let table = current
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("config path is not a table: {key}"))?;
    if !table.contains_key(last) {
        anyhow::bail!("unknown config key: {key}");
    }
    table.insert(last.to_string(), value);
    Ok(())
}
