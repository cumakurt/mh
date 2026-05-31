use anyhow::{Result, bail};

use crate::config::AppConfig;
use crate::policy::{PolicyAction, PolicyEngine};

/// Block execution paths that must honor the same deny rules as recording.
pub fn ensure_execution_allowed(
    config: &AppConfig,
    command: &str,
    hostname: Option<&str>,
    environment_tier: Option<&str>,
) -> Result<()> {
    let engine = PolicyEngine::from_config(config)?;
    let decision = engine.evaluate(command, hostname, environment_tier);
    match decision.action {
        PolicyAction::Deny => {
            bail!("policy denied execution: {}", decision.message);
        }
        PolicyAction::RequireApproval => {
            bail!(
                "policy requires approval before execution (rule {}); use an interactive command with --reason",
                decision.rule_id
            );
        }
        PolicyAction::Warn | PolicyAction::Allow => Ok(()),
    }
}

/// Resolve environment tier from a stored row, re-classifying when legacy rows lack it.
pub fn environment_tier_for_command(
    config: &AppConfig,
    stored_tier: Option<&str>,
    hostname: Option<&str>,
    cwd: Option<&str>,
    git_repo: Option<&str>,
) -> Option<String> {
    stored_tier.map(str::to_string).or_else(|| {
        Some(crate::environment::classify_label(
            config, hostname, cwd, git_repo,
        ))
    })
}
