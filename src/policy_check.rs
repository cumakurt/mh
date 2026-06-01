//! Fast policy evaluation for shell hooks and automation.

use std::env;
use std::process;

use anyhow::Result;

use crate::break_glass;
use crate::config::AppConfig;
use crate::environment;
use crate::policy::{PolicyAction, PolicyEngine};

/// Exit codes for `mh policy check --quiet`.
pub const EXIT_ALLOW: i32 = 0;
pub const EXIT_DENY: i32 = 2;
pub const EXIT_REQUIRE_APPROVAL: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum PolicyCheckOutcome {
    Allow,
    Warn,
    Deny,
    RequireApproval,
}

pub struct PolicyCheckRequest<'a> {
    pub command: &'a str,
    pub cwd: Option<&'a str>,
    pub hostname: Option<&'a str>,
    pub environment: Option<&'a str>,
    pub quiet: bool,
}

pub fn evaluate_request(config: &AppConfig, request: &PolicyCheckRequest<'_>) -> PolicyCheckOutcome {
    if !config.policy.enforce_in_shell {
        return PolicyCheckOutcome::Allow;
    }

    if break_glass::is_active() {
        return PolicyCheckOutcome::Allow;
    }

    if env::var_os("MH_POLICY_APPROVE").is_some() {
        return PolicyCheckOutcome::Allow;
    }

    let hostname = request
        .hostname
        .map(str::to_string)
        .or_else(|| hostname::get().ok().and_then(|value| value.into_string().ok()));

    let cwd = request
        .cwd
        .map(str::to_string)
        .or_else(|| env::current_dir().ok().map(|path| path.to_string_lossy().into_owned()));

    let environment = request.environment.map(str::to_string).or_else(|| {
        cwd.as_deref().map(|path| {
            environment::classify_label(config, hostname.as_deref(), Some(path), None)
        })
    });

    let Ok(engine) = PolicyEngine::from_config(config) else {
        return PolicyCheckOutcome::Allow;
    };
    let decision = engine.evaluate(
        request.command,
        hostname.as_deref(),
        environment.as_deref(),
    );

    match decision.action {
        PolicyAction::Allow => PolicyCheckOutcome::Allow,
        PolicyAction::Warn => PolicyCheckOutcome::Warn,
        PolicyAction::Deny => PolicyCheckOutcome::Deny,
        PolicyAction::RequireApproval => PolicyCheckOutcome::RequireApproval,
    }
}

pub fn run_check(config: &AppConfig, request: &PolicyCheckRequest<'_>) -> Result<()> {
    let outcome = evaluate_request(config, request);

    if !request.quiet {
        match outcome {
            PolicyCheckOutcome::Allow => {}
            PolicyCheckOutcome::Warn => {
                println!("mh: policy warn (command allowed)");
            }
            PolicyCheckOutcome::Deny => {
                eprintln!("mh: policy denied — command blocked");
            }
            PolicyCheckOutcome::RequireApproval => {
                eprintln!(
                    "mh: policy requires approval — export MH_POLICY_APPROVE=1 for one command, or run mh break-glass on"
                );
            }
        }
    }

    match outcome {
        PolicyCheckOutcome::Allow | PolicyCheckOutcome::Warn => Ok(()),
        PolicyCheckOutcome::Deny => {
            if request.quiet {
                process::exit(EXIT_DENY);
            }
            process::exit(EXIT_DENY);
        }
        PolicyCheckOutcome::RequireApproval => {
            if request.quiet {
                process::exit(EXIT_REQUIRE_APPROVAL);
            }
            process::exit(EXIT_REQUIRE_APPROVAL);
        }
    }
}
