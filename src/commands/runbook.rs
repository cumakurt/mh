use std::path::Path;

use anyhow::Result;

use crate::cli::{RunbookArgs, RunbookCommand};
use crate::command_exec::execute_shell_command;
use crate::config::AppConfig;
use crate::db::Database;
use crate::execution_policy::ensure_execution_allowed;
use crate::output::styling::Styler;

pub fn run(args: RunbookArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;

    match args.command {
        RunbookCommand::List => {
            let runbooks = database.list_runbooks()?;
            if runbooks.is_empty() {
                println!("No runbooks saved");
                return Ok(());
            }
            for runbook in runbooks {
                println!(
                    "{} {} session={}",
                    runbook.id,
                    runbook.name,
                    runbook.source_session_id.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        RunbookCommand::Show { name } => {
            let steps = database.runbook_steps(&name)?;
            for step in steps {
                println!(
                    "{}. {} cwd={}",
                    step.step_order,
                    step.command,
                    step.cwd.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        RunbookCommand::Create {
            name,
            session,
            desc,
        } => {
            let id = database.create_runbook_from_session(&name, desc.as_deref(), &session)?;
            println!(
                "{}",
                styler.success(format!(
                    "Created runbook {name} (id {id}) from session {session}"
                ))
            );
        }
        RunbookCommand::Run { name, dry_run } => {
            let steps = database.runbook_steps(&name)?;
            let hostname = hostname::get()
                .ok()
                .map(|value| value.to_string_lossy().to_string());
            for step in steps {
                if dry_run {
                    println!("[dry-run] {}", step.command);
                    continue;
                }
                ensure_execution_allowed(&config, &step.command, hostname.as_deref(), None)?;
                let cwd = step.cwd.as_deref().map(Path::new);
                let status = execute_shell_command(&step.command, cwd)?;
                if !status.success() {
                    anyhow::bail!(
                        "runbook step {} failed with status {status}",
                        step.step_order
                    );
                }
            }
        }
    }

    Ok(())
}
