use anyhow::Result;

use std::path::Path;

use crate::cli::{PolicyArgs, PolicyCommand, PolicyPackCommand};
use crate::config::{AppConfig, config_path};
use crate::output::styling::Styler;
use crate::policy::PolicyAction;
use crate::policy_check::{PolicyCheckOutcome, PolicyCheckRequest, evaluate, run_check};

pub fn run(args: PolicyArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);

    match args.command {
        PolicyCommand::List => {
            println!("Default action: {}", config.policy.default_action);
            println!(
                "Enforce in shell: {}",
                if config.policy.enforce_in_shell {
                    "yes"
                } else {
                    "no"
                }
            );
            for rule in &config.policy.rules {
                println!("- {} => {} ({})", rule.id, rule.action, rule.message);
            }
        }
        PolicyCommand::Pack(args) => match args.command {
            PolicyPackCommand::Export { file, key } => {
                let key = policy_pack_key(key)?;
                let pack = crate::policy_pack::sign_policy(config.policy.clone(), &key)?;
                crate::policy_pack::write_pack(Path::new(&file), &pack)?;
                println!("Signed policy pack written to {file}");
            }
            PolicyPackCommand::Verify { file, key } => {
                let key = policy_pack_key(key)?;
                let pack = crate::policy_pack::read_pack(Path::new(&file))?;
                crate::policy_pack::verify_pack(&pack, &key)?;
                println!("Policy pack verified: {file}");
            }
            PolicyPackCommand::Apply { file, key } => {
                let key = policy_pack_key(key)?;
                let pack = crate::policy_pack::read_pack(Path::new(&file))?;
                crate::policy_pack::verify_pack(&pack, &key)?;
                let mut updated = config.clone();
                updated.policy = pack.payload.policy;
                updated.write_to_path(&config_path())?;
                println!("Applied signed policy pack: {file}");
            }
        },
        PolicyCommand::Check {
            command,
            command_arg,
            hostname,
            env,
            cwd,
            json,
            quiet,
        } => {
            let command = match (command, command_arg) {
                (Some(command), None) | (None, Some(command)) => command,
                (Some(_), Some(_)) => {
                    anyhow::bail!("provide command either positionally or with --command, not both")
                }
                (None, None) => anyhow::bail!("missing required command"),
            };
            let request = PolicyCheckRequest {
                command: &command,
                cwd: cwd.as_deref(),
                hostname: hostname.as_deref(),
                environment: env.as_deref(),
                quiet,
            };

            if quiet {
                return run_check(&config, &request);
            }

            let evaluation = evaluate(&config, &request);
            let outcome = outcome_from_action(evaluation.decision.action);
            if json {
                println!("{}", serde_json::to_string_pretty(&evaluation.decision)?);
                return map_outcome_exit(outcome);
            }

            let action = evaluation.decision.action;
            let action_text = match action {
                PolicyAction::Allow => styler.success(action_label(action)),
                PolicyAction::Warn => styler.warning(action_label(action)),
                PolicyAction::Deny => styler.warning(action_label(action)),
                PolicyAction::RequireApproval => styler.accent(action_label(action)),
            };
            println!(
                "{} {} ({})",
                action_text, evaluation.decision.rule_id, evaluation.decision.message
            );
            return map_outcome_exit(outcome);
        }
    }

    Ok(())
}

fn policy_pack_key(value: Option<String>) -> Result<String> {
    value
        .or_else(|| std::env::var("MH_POLICY_PACK_KEY").ok())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("provide --key or set MH_POLICY_PACK_KEY"))
}

fn outcome_from_action(action: PolicyAction) -> PolicyCheckOutcome {
    match action {
        PolicyAction::Allow => PolicyCheckOutcome::Allow,
        PolicyAction::Warn => PolicyCheckOutcome::Warn,
        PolicyAction::Deny => PolicyCheckOutcome::Deny,
        PolicyAction::RequireApproval => PolicyCheckOutcome::RequireApproval,
    }
}

fn map_outcome_exit(outcome: crate::policy_check::PolicyCheckOutcome) -> Result<()> {
    use crate::policy_check::{EXIT_DENY, EXIT_REQUIRE_APPROVAL, PolicyCheckOutcome};
    match outcome {
        PolicyCheckOutcome::Allow | PolicyCheckOutcome::Warn => Ok(()),
        PolicyCheckOutcome::Deny => std::process::exit(EXIT_DENY),
        PolicyCheckOutcome::RequireApproval => std::process::exit(EXIT_REQUIRE_APPROVAL),
    }
}

fn action_label(action: PolicyAction) -> &'static str {
    match action {
        PolicyAction::Allow => "allow",
        PolicyAction::Warn => "warn",
        PolicyAction::Deny => "deny",
        PolicyAction::RequireApproval => "require_approval",
    }
}
