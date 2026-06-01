use anyhow::{Result, bail};

use crate::config::AppConfig;
use crate::policy_check::{PolicyCheckOutcome, PolicyCheckRequest, evaluate_request};

/// Block execution paths that must honor the same deny rules as recording.
pub fn ensure_execution_allowed(
    config: &AppConfig,
    command: &str,
    hostname: Option<&str>,
    environment_tier: Option<&str>,
) -> Result<()> {
    let request = PolicyCheckRequest {
        command,
        cwd: None,
        hostname,
        environment: environment_tier,
        quiet: false,
    };
    match evaluate_request(config, &request) {
        PolicyCheckOutcome::Allow | PolicyCheckOutcome::Warn => Ok(()),
        PolicyCheckOutcome::Deny => bail!("policy denied execution"),
        PolicyCheckOutcome::RequireApproval => {
            bail!(
                "policy requires approval before execution; export MH_POLICY_APPROVE=1 or run mh break-glass on"
            );
        }
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
