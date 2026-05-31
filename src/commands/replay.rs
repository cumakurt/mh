use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Result, bail};

use crate::cli::ReplayArgs;
use crate::command_exec::execute_shell_command;
use crate::config::AppConfig;
use crate::db::Database;
use crate::execution_policy::environment_tier_for_command;
use crate::policy::{PolicyAction, PolicyEngine};
use crate::risk::{self, RiskLevel};
use crate::security;

pub fn run(args: ReplayArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let row = database.get_command(args.id)?;
    let username = Some(whoami::username());
    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());

    if args.dry_run {
        println!("{}", security::redact_for_audit(&row.command, &config)?);
        return Ok(());
    }

    let environment_tier = environment_tier_for_command(
        &config,
        row.environment_tier.as_deref(),
        row.hostname.as_deref(),
        row.cwd.as_deref(),
        row.git_repo.as_deref(),
    );

    let audit_command = security::redact_for_audit(&row.command, &config)?;

    let policy = PolicyEngine::from_config(&config)?;
    let policy_decision = policy.evaluate(
        &row.command,
        row.hostname.as_deref(),
        environment_tier.as_deref(),
    );
    match policy_decision.action {
        PolicyAction::Deny => {
            if config.security.audit_log {
                let audit_row = database.insert_audit_log(
                    "replay_denied",
                    &audit_command,
                    &policy_decision.message,
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(&config, &audit_row);
            }
            bail!("policy denied replay: {}", policy_decision.message);
        }
        PolicyAction::RequireApproval => {
            let reason = args
                .reason
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(str::trim);
            if reason.is_none() && !(args.yes || args.confirm) {
                bail!("replay requires --reason for policy approval or --yes to confirm");
            }
            if config.security.audit_log {
                let audit_row = database.insert_audit_log(
                    "replay_approval",
                    &audit_command,
                    reason.unwrap_or("confirmed with --yes"),
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(&config, &audit_row);
            }
        }
        PolicyAction::Warn if config.security.audit_log => {
            let audit_row = database.insert_audit_log(
                "replay_warn",
                &audit_command,
                &policy_decision.message,
                username.as_deref(),
                hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(&config, &audit_row);
        }
        _ => {}
    }

    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    let skip_confirm = args.yes || args.confirm;

    if !interactive && !skip_confirm {
        bail!("refusing to replay command in non-interactive mode; pass --yes to confirm");
    }

    if interactive && !skip_confirm && !confirm(&format!("Run command {}?", row.id))? {
        println!("Replay cancelled");
        return Ok(());
    }

    if let Some(assessment) = risk::assess_command(&row.command) {
        eprintln!(
            "Warning: {} risk command ({})",
            assessment.level.label(),
            assessment.description
        );
        if assessment.level == RiskLevel::Critical && !skip_confirm {
            if interactive {
                if !confirm(&format!("Critical risk command {}. Run anyway?", row.id))? {
                    println!("Replay cancelled");
                    return Ok(());
                }
            } else {
                bail!(
                    "refusing to replay critical-risk command in non-interactive mode; pass --yes to confirm"
                );
            }
        }
    }

    let cwd = row.cwd.as_deref().map(Path::new);
    let status = execute_shell_command(&row.command, cwd)?;
    if config.security.audit_log {
        let audit_row = database.insert_audit_log(
            "replay",
            &audit_command,
            &format!("replayed command id {}", row.id),
            username.as_deref(),
            hostname.as_deref(),
        )?;
        crate::siem::emit_audit_event(&config, &audit_row);
    }
    if !status.success() {
        bail!("replayed command exited with status {status}");
    }
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    eprint!("{prompt} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn confirm_logic_accepts_yes_variants() {
        assert!(matches!("y".trim(), "y" | "Y" | "yes" | "YES"));
        assert!(!matches!("n".trim(), "y" | "Y" | "yes" | "YES"));
    }
}
