use anyhow::Result;

use crate::cli::{PolicyArgs, PolicyCommand};
use crate::config::AppConfig;
use crate::output::styling::Styler;
use crate::policy::{PolicyAction, PolicyEngine};

pub fn run(args: PolicyArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let engine = PolicyEngine::from_config(&config)?;

    match args.command {
        PolicyCommand::List => {
            println!("Default action: {}", config.policy.default_action);
            for rule in &config.policy.rules {
                println!("- {} => {} ({})", rule.id, rule.action, rule.message);
            }
        }
        PolicyCommand::Check {
            command,
            hostname,
            env,
            json,
        } => {
            let decision = engine.evaluate(&command, hostname.as_deref(), env.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&decision)?);
            } else {
                let action_text = match decision.action {
                    PolicyAction::Allow => styler.success(action_label(decision.action)),
                    PolicyAction::Warn => styler.warning(action_label(decision.action)),
                    PolicyAction::Deny => styler.warning(action_label(decision.action)),
                    PolicyAction::RequireApproval => styler.accent(action_label(decision.action)),
                };
                println!(
                    "{} {} ({})",
                    action_text, decision.rule_id, decision.message
                );
            }
        }
    }

    Ok(())
}

fn action_label(action: PolicyAction) -> &'static str {
    match action {
        PolicyAction::Allow => "allow",
        PolicyAction::Warn => "warn",
        PolicyAction::Deny => "deny",
        PolicyAction::RequireApproval => "require_approval",
    }
}
