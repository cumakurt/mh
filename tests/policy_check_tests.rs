use mh::config::AppConfig;
use mh::policy_check::{PolicyCheckOutcome, PolicyCheckRequest, evaluate_request};

#[test]
fn allows_when_enforcement_disabled() {
    let mut config = AppConfig::default();
    config.policy.enforce_in_shell = false;
    let outcome = evaluate_request(
        &config,
        &PolicyCheckRequest {
            command: "rm -rf /",
            cwd: Some("/"),
            hostname: Some("prod-web"),
            environment: Some("production"),
            quiet: true,
        },
    );
    assert_eq!(outcome, PolicyCheckOutcome::Allow);
}

#[test]
fn denies_critical_in_production() {
    let config = AppConfig::default();
    let outcome = evaluate_request(
        &config,
        &PolicyCheckRequest {
            command: "rm -rf /",
            cwd: Some("/srv"),
            hostname: Some("prod-web-01"),
            environment: Some("production"),
            quiet: true,
        },
    );
    assert_eq!(outcome, PolicyCheckOutcome::Deny);
}
