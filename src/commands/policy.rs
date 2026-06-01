use anyhow::Result;

use crate::cli::{PolicyArgs, PolicyCommand};
use crate::config::AppConfig;
use crate::output::styling::Styler;
use crate::policy::{PolicyAction, PolicyEngine};
use crate::policy_check::{
    PolicyCheckOutcome, PolicyCheckRequest, evaluate_request, run_check,
};

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
        PolicyCommand::Check {
            command,
            hostname,
            env,
            cwd,
            json,
            quiet,
        } => {
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

            let outcome = evaluate_request(&config, &request);
            if json {
                let engine = PolicyEngine::from_config(&config)?;
                let decision = engine.evaluate(&command, hostname.as_deref(), env.as_deref());
                println!("{}", serde_json::to_string_pretty(&decision)?);
                return map_outcome_exit(outcome);
            }

            let action = match outcome {
                PolicyCheckOutcome::Allow => PolicyAction::Allow,
                PolicyCheckOutcome::Warn => PolicyAction::Warn,
                PolicyCheckOutcome::Deny => PolicyAction::Deny,
                PolicyCheckOutcome::RequireApproval => PolicyAction::RequireApproval,
            };
            let engine = PolicyEngine::from_config(&config)?;
            let decision = engine.evaluate(&command, hostname.as_deref(), env.as_deref());
            let action_text = match action {
                PolicyAction::Allow => styler.success(action_label(action)),
                PolicyAction::Warn => styler.warning(action_label(action)),
                PolicyAction::Deny => styler.warning(action_label(action)),
                PolicyAction::RequireApproval => styler.accent(action_label(action)),
            };
            println!(
                "{} {} ({})",
                action_text, decision.rule_id, decision.message
            );
            return map_outcome_exit(outcome);
        }
    }

    Ok(())
}

fn map_outcome_exit(outcome: crate::policy_check::PolicyCheckOutcome) -> Result<()> {
    use crate::policy_check::{PolicyCheckOutcome, EXIT_DENY, EXIT_REQUIRE_APPROVAL};
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
