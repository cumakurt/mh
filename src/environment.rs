use crate::config::{AppConfig, EnvironmentRuleConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentTier {
    Production,
    Staging,
    Development,
    Unknown,
}

impl EnvironmentTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::Development => "development",
            Self::Unknown => "unknown",
        }
    }
}

pub fn classify(
    config: &AppConfig,
    hostname: Option<&str>,
    cwd: Option<&str>,
    git_repo: Option<&str>,
) -> EnvironmentTier {
    let hostname = hostname.unwrap_or_default();
    let cwd = cwd.unwrap_or_default();
    let git_repo = git_repo.unwrap_or_default();

    for rule in &config.environment.rules {
        if rule_matches(rule, hostname, cwd, git_repo) {
            return parse_tier(&rule.tier);
        }
    }

    EnvironmentTier::Unknown
}

pub fn classify_label(
    config: &AppConfig,
    hostname: Option<&str>,
    cwd: Option<&str>,
    git_repo: Option<&str>,
) -> String {
    classify(config, hostname, cwd, git_repo)
        .label()
        .to_string()
}

fn rule_matches(rule: &EnvironmentRuleConfig, hostname: &str, cwd: &str, git_repo: &str) -> bool {
    if let Some(pattern) = rule.hostname_contains.as_deref()
        && !hostname.contains(pattern)
    {
        return false;
    }

    if let Some(pattern) = rule.cwd_contains.as_deref()
        && !cwd.contains(pattern)
    {
        return false;
    }

    if let Some(pattern) = rule.git_repo_contains.as_deref()
        && !git_repo.contains(pattern)
    {
        return false;
    }

    rule.hostname_contains.is_some()
        || rule.cwd_contains.is_some()
        || rule.git_repo_contains.is_some()
}

fn parse_tier(value: &str) -> EnvironmentTier {
    match value.to_ascii_lowercase().as_str() {
        "production" | "prod" => EnvironmentTier::Production,
        "staging" | "stage" => EnvironmentTier::Staging,
        "development" | "dev" => EnvironmentTier::Development,
        _ => EnvironmentTier::Unknown,
    }
}

pub fn default_environment_config() -> crate::config::EnvironmentConfig {
    crate::config::EnvironmentConfig {
        rules: vec![
            EnvironmentRuleConfig {
                tier: "production".to_string(),
                hostname_contains: Some("prod".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
            EnvironmentRuleConfig {
                tier: "staging".to_string(),
                hostname_contains: Some("stage".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
            EnvironmentRuleConfig {
                tier: "development".to_string(),
                hostname_contains: Some("dev".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_prod_hostname() {
        let config = AppConfig {
            environment: default_environment_config(),
            ..Default::default()
        };
        assert_eq!(
            classify(&config, Some("prod-web-01"), None, None),
            EnvironmentTier::Production
        );
    }
}
