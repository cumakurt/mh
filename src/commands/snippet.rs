use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use regex::Regex;

use crate::cli::{SnippetArgs, SnippetCommand};
use crate::command_exec::execute_shell_command;
use crate::config::{self, AppConfig};
use crate::db::Database;
use crate::execution_policy::ensure_execution_allowed;

pub fn run(args: SnippetArgs) -> Result<()> {
    match args.command {
        SnippetCommand::Save(args) => save(args),
        SnippetCommand::List => list(),
        SnippetCommand::Run(args) => run_snippet(args),
        SnippetCommand::Delete(args) => delete(args),
        SnippetCommand::Export(args) => export(args),
    }
}

fn save(args: crate::cli::SnippetSaveArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    database.save_snippet(
        &args.name,
        &args.command,
        args.desc.as_deref(),
        args.tags.as_deref(),
    )?;
    println!("Saved snippet {}", args.name);
    Ok(())
}

fn list() -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let snippets = database.list_snippets()?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Name", "Uses", "Description", "Command"]);
    for snippet in snippets {
        table.add_row(vec![
            Cell::new(snippet.name),
            Cell::new(snippet.use_count),
            Cell::new(snippet.description.unwrap_or_else(|| "-".to_string())),
            Cell::new(snippet.command),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn run_snippet(args: crate::cli::SnippetRunArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let snippet = database.get_snippet(&args.name)?;
    let vars = parse_vars(&args.vars)?;
    let command = replace_placeholders(&snippet.command, &vars)?;
    database.increment_snippet_use(&args.name)?;
    if args.dry_run {
        println!("{command}");
        return Ok(());
    }

    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());
    ensure_execution_allowed(&config, &command, hostname.as_deref(), None)?;

    let status = execute_shell_command(&command, None::<&Path>)?;
    if !status.success() {
        bail!("snippet command exited with status {status}");
    }
    Ok(())
}

fn delete(args: crate::cli::SnippetDeleteArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let deleted = database.delete_snippet(&args.name)?;
    println!("Deleted {deleted} snippet(s)");
    Ok(())
}

fn export(args: crate::cli::SnippetExportArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let snippets = database.list_snippets()?;
    let payload = serde_json::to_string_pretty(&snippets)?;
    config::write_private_file(Path::new(&args.file), payload.as_bytes())
        .with_context(|| format!("failed to write snippet export to {}", args.file))?;
    println!("Exported {} snippet(s)", snippets.len());
    Ok(())
}

fn parse_vars(values: &[String]) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();
    for value in values {
        let Some((key, value)) = value.split_once('=') else {
            bail!("placeholder variables must use KEY=VALUE syntax");
        };
        vars.insert(key.to_string(), value.to_string());
    }
    Ok(vars)
}

fn replace_placeholders(command: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let regex = Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}")?;
    let mut missing = Vec::new();
    let rendered = regex
        .replace_all(command, |captures: &regex::Captures<'_>| {
            let key = &captures[1];
            match vars.get(key) {
                Some(value) => value.clone(),
                None => {
                    missing.push(key.to_string());
                    captures[0].to_string()
                }
            }
        })
        .to_string();

    if !missing.is_empty() {
        bail!("missing snippet variable(s): {}", missing.join(", "));
    }

    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_named_placeholders() {
        let vars = parse_vars(&["user=admin".to_string(), "host=127.0.0.1".to_string()])
            .expect("vars should parse");
        let rendered = replace_placeholders("ssh {{user}}@{{host}}", &vars)
            .expect("placeholders should be replaced");
        assert_eq!(rendered, "ssh admin@127.0.0.1");
    }
}
