use std::sync::OnceLock;

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;

use crate::config::{AppConfig, PolicyConfig, PolicyRuleConfig};
use crate::risk::{self, RiskLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Allow,
    Warn,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    id: String,
    action: PolicyAction,
    risk_level: Option<RiskLevel>,
    pattern: Option<Regex>,
    environment: Option<String>,
    hostname_pattern: Option<Regex>,
    message: String,
}

pub struct PolicyEngine {
    rules: Vec<CompiledRule>,
    default_action: PolicyAction,
}

impl PolicyEngine {
    pub fn from_config(config: &AppConfig) -> Result<Self> {
        let mut rules = Vec::new();
        for rule in &config.policy.rules {
            rules.push(compile_rule(rule)?);
        }
        Ok(Self {
            rules,
            default_action: parse_action(&config.policy.default_action)?,
        })
    }

    pub fn evaluate(
        &self,
        command: &str,
        hostname: Option<&str>,
        environment_tier: Option<&str>,
    ) -> PolicyDecision {
        for rule in &self.rules {
            if rule_matches(rule, command, hostname, environment_tier) {
                return PolicyDecision {
                    action: rule.action,
                    rule_id: rule.id.clone(),
                    message: rule.message.clone(),
                };
            }
        }

        PolicyDecision {
            action: self.default_action,
            rule_id: "default".to_string(),
            message: "No matching policy rule".to_string(),
        }
    }
}

fn compile_rule(rule: &PolicyRuleConfig) -> Result<CompiledRule> {
    Ok(CompiledRule {
        id: rule.id.clone(),
        action: parse_action(&rule.action)?,
        risk_level: rule
            .risk_level
            .as_deref()
            .map(parse_risk_level)
            .transpose()?,
        pattern: rule
            .pattern
            .as_deref()
            .map(Regex::new)
            .transpose()
            .with_context(|| format!("invalid policy pattern in rule {}", rule.id))?,
        environment: rule.environment.clone(),
        hostname_pattern: rule
            .hostname_pattern
            .as_deref()
            .map(Regex::new)
            .transpose()
            .with_context(|| format!("invalid hostname pattern in rule {}", rule.id))?,
        message: rule.message.clone(),
    })
}

fn rule_matches(
    rule: &CompiledRule,
    command: &str,
    hostname: Option<&str>,
    environment_tier: Option<&str>,
) -> bool {
    if let Some(level) = rule.risk_level {
        let Some(assessment) = risk::assess_command(command) else {
            return false;
        };
        if !risk::is_at_least(assessment.level, level) {
            return false;
        }
    }

    if let Some(pattern) = &rule.pattern
        && !pattern.is_match(command)
    {
        return false;
    }

    if let Some(env) = &rule.environment
        && environment_tier != Some(env.as_str())
    {
        return false;
    }

    if let Some(host_pattern) = &rule.hostname_pattern {
        let Some(host) = hostname else {
            return false;
        };
        if !host_pattern.is_match(host) {
            return false;
        }
    }

    rule.risk_level.is_some()
        || rule.pattern.is_some()
        || rule.environment.is_some()
        || rule.hostname_pattern.is_some()
}

fn parse_action(value: &str) -> Result<PolicyAction> {
    match value.to_ascii_lowercase().as_str() {
        "allow" => Ok(PolicyAction::Allow),
        "warn" => Ok(PolicyAction::Warn),
        "deny" => Ok(PolicyAction::Deny),
        "require_approval" | "approval" => Ok(PolicyAction::RequireApproval),
        other => anyhow::bail!("unknown policy action: {other}"),
    }
}

fn parse_risk_level(value: &str) -> Result<RiskLevel> {
    match value.to_ascii_lowercase().as_str() {
        "medium" => Ok(RiskLevel::Medium),
        "high" => Ok(RiskLevel::High),
        "critical" => Ok(RiskLevel::Critical),
        other => anyhow::bail!("unknown risk level: {other}"),
    }
}

pub fn default_policy_config() -> PolicyConfig {
    PolicyConfig {
        default_action: "allow".to_string(),
        enforce_in_shell: true,
        rules: vec![
            PolicyRuleConfig {
                id: "deny-critical-prod".to_string(),
                action: "deny".to_string(),
                risk_level: Some("critical".to_string()),
                pattern: None,
                environment: Some("production".to_string()),
                hostname_pattern: None,
                message: "Critical commands are blocked in production".to_string(),
            },
            PolicyRuleConfig {
                id: "approval-critical".to_string(),
                action: "require_approval".to_string(),
                risk_level: Some("critical".to_string()),
                pattern: None,
                environment: None,
                hostname_pattern: None,
                message: "Critical commands require explicit approval".to_string(),
            },
            PolicyRuleConfig {
                id: "warn-high".to_string(),
                action: "warn".to_string(),
                risk_level: Some("high".to_string()),
                pattern: None,
                environment: None,
                hostname_pattern: None,
                message: "High risk command detected".to_string(),
            },
        ],
    }
}

static DEFAULT_ENGINE: OnceLock<Result<PolicyEngine>> = OnceLock::new();

pub fn default_engine() -> Result<&'static PolicyEngine> {
    DEFAULT_ENGINE
        .get_or_init(|| {
            PolicyEngine::from_config(&AppConfig {
                policy: default_policy_config(),
                ..AppConfig::default()
            })
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_critical_command_in_production() {
        let engine = default_engine().expect("engine should compile");
        let decision = engine.evaluate("rm -rf /", Some("prod-web-01"), Some("production"));
        assert_eq!(decision.action, PolicyAction::Deny);
        assert_eq!(decision.rule_id, "deny-critical-prod");
    }

    #[test]
    fn requires_approval_for_critical_outside_prod() {
        let engine = default_engine().expect("engine should compile");
        let decision = engine.evaluate("rm -rf /", Some("dev-laptop"), Some("development"));
        assert_eq!(decision.action, PolicyAction::RequireApproval);
    }
}
