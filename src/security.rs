use std::env;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use regex::Regex;

use crate::config::{AppConfig, private_mode_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityAction {
    Store,
    Masked,
    Skipped(String),
}

#[derive(Debug, Clone)]
pub struct SecurityDecision {
    pub command: String,
    pub action: SecurityAction,
}

pub struct SecurityEngine {
    ignore_patterns: Vec<Regex>,
}

impl SecurityEngine {
    pub fn from_config(config: &AppConfig) -> Result<Self> {
        let ignore_patterns = config
            .ignore
            .patterns
            .iter()
            .map(|pattern| {
                Regex::new(pattern)
                    .with_context(|| format!("invalid ignore regex pattern: {pattern}"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { ignore_patterns })
    }

    pub fn process(&self, command: &str, config: &AppConfig) -> Result<SecurityDecision> {
        process_command_with_engine(command, config, self)
    }
}

pub fn process_command(command: &str, config: &AppConfig) -> Result<SecurityDecision> {
    let engine = SecurityEngine::from_config(config)?;
    engine.process(command, config)
}

/// Text safe to persist in audit logs and SIEM exports.
pub fn redact_for_audit(command: &str, config: &AppConfig) -> Result<String> {
    if private_mode_enabled(config) && !crate::break_glass::is_active() {
        return Ok("[redacted: private mode]".to_string());
    }

    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    if contains_secret(trimmed)? {
        return mask_secrets(command);
    }

    Ok(command.to_string())
}

pub fn command_for_audit(
    original: &str,
    decision: &SecurityDecision,
    config: &AppConfig,
) -> Result<String> {
    match &decision.action {
        SecurityAction::Skipped(reason) if reason.contains("private mode") => {
            Ok("[redacted: private mode]".to_string())
        }
        SecurityAction::Masked => Ok(decision.command.clone()),
        _ => redact_for_audit(original, config),
    }
}

fn process_command_with_engine(
    command: &str,
    config: &AppConfig,
    engine: &SecurityEngine,
) -> Result<SecurityDecision> {
    if private_mode_enabled(config) && !crate::break_glass::is_active() {
        return Ok(SecurityDecision {
            command: command.to_string(),
            action: SecurityAction::Skipped("private mode is enabled".to_string()),
        });
    }

    if config.history.ignore_space_prefix && has_hide_prefix(command) {
        return Ok(SecurityDecision {
            command: command.to_string(),
            action: SecurityAction::Skipped(
                "command starts with a leading space or tab".to_string(),
            ),
        });
    }

    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Ok(SecurityDecision {
            command: command.to_string(),
            action: SecurityAction::Skipped("command is empty".to_string()),
        });
    }

    for ignored in &config.ignore.commands {
        if trimmed == ignored || trimmed.starts_with(&format!("{ignored} ")) {
            return Ok(SecurityDecision {
                command: command.to_string(),
                action: SecurityAction::Skipped(format!("command matches ignore rule: {ignored}")),
            });
        }
    }

    if contains_secret(trimmed)? {
        if config.security.skip_secret_commands {
            return Ok(SecurityDecision {
                command: command.to_string(),
                action: SecurityAction::Skipped("command contains sensitive data".to_string()),
            });
        }

        if config.security.mask_secrets {
            let masked = mask_secrets(command)?;
            if masked != command {
                return Ok(SecurityDecision {
                    command: masked,
                    action: SecurityAction::Masked,
                });
            }
            return Ok(SecurityDecision {
                command: command.to_string(),
                action: SecurityAction::Skipped(
                    "command contains sensitive data that could not be masked".to_string(),
                ),
            });
        }

        return Ok(SecurityDecision {
            command: command.to_string(),
            action: SecurityAction::Skipped(
                "command contains sensitive data (storage disabled when masking is off)"
                    .to_string(),
            ),
        });
    }

    for pattern in &engine.ignore_patterns {
        if pattern.is_match(trimmed) {
            return Ok(SecurityDecision {
                command: command.to_string(),
                action: SecurityAction::Skipped(format!(
                    "command matches ignore pattern: {pattern}"
                )),
            });
        }
    }

    Ok(SecurityDecision {
        command: command.to_string(),
        action: SecurityAction::Store,
    })
}

pub fn private_mode_enabled(config: &AppConfig) -> bool {
    env::var(&config.security.private_mode_env)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
        || private_mode_path().exists()
}

pub fn contains_secret(command: &str) -> Result<bool> {
    Ok(contains_luhn_credit_card(command)
        || authorization_header_regex().is_match(command)
        || export_secret_env_regex().is_match(command)
        || long_flag_regex().is_match(command)
        || mysql_password_flag_regex().is_match(command)
        || docker_login_password_regex().is_match(command)
        || curl_user_password_regex().is_match(command)
        || curl_long_user_regex().is_match(command)
        || wget_password_regex().is_match(command)
        || redis_auth_regex().is_match(command)
        || sshpass_env_regex().is_match(command)
        || sshpass_password_flag_regex().is_match(command)
        || db_connection_url_regex().is_match(command)
        || pgpassword_env_regex().is_match(command)
        || kubectl_secret_literal_regex().is_match(command))
}

pub fn mask_secrets(command: &str) -> Result<String> {
    let mut masked = command.to_string();
    for (regex, replacement) in masking_regexes().iter().zip(masking_replacements().iter()) {
        masked = regex.replace_all(&masked, *replacement).to_string();
    }
    masked = mask_luhn_credit_cards(&masked);
    Ok(masked)
}

const MASKING_PATTERNS: &[&str] = &[
    r#"(?i)(authorization:\s*basic\s+)([a-z0-9+/=]+)"#,
    r#"(?i)(authorization:\s*bearer\s+)([^"'\s]+)"#,
    r#"(?i)(PGPASSWORD=)([^\s]+)"#,
    r#"(?i)(bearer\s+)([^"'\s]+)"#,
    r#"(?i)((?:password|passwd|pwd|token|secret|api[_-]?key|aws_secret_access_key|aws_access_key_id|github_token|gitlab_token|mysql_pwd)\s*=\s*)("[^"]*"|'[^']*'|[^"'\s]+)"#,
    r#"(?i)(--?(?:password|passwd|pwd|token|secret|api[_-]?key)\s+)(?:"[^"]*"|'[^']*'|[^"'\s]+)"#,
    r#"(?i)(--(?:password|passwd|token|secret|api[_-]?key)=)([^\s"']+)"#,
    r#"(?i)((?:^|[\s;&|])(?:export\s+)?(?:[A-Za-z_]*(?:PASSWORD|PASSWD|TOKEN|SECRET|API[_-]?KEY|AWS_SECRET_ACCESS_KEY|AWS_ACCESS_KEY_ID|GITHUB_TOKEN|GITLAB_TOKEN|MYSQL_PWD)[A-Za-z_]*)\s*=\s*)([^\s]+)"#,
    r#"(?i)(sshpass\s+-p\s+)(?:"[^"]*"|'[^']*'|[^\s]+)"#,
    r#"(?i)(SSHPASS=)([^\s]+)"#,
    r#"(?i)((?:mysql|mariadb|mysqldump)\b[^\n]*-p)(?:"[^"]*"|'[^']*'|\S+|\s+(?:"[^"]*"|'[^']*'|\S+))"#,
    r#"(?i)(docker\s+login\b[^\n]*-p\s+)(?:"[^"]*"|'[^']*'|[^\s]+)"#,
    r#"(?i)(docker\s+login\b[^\n]*-p)(?:"[^"]*"|'[^']*'|\S+)"#,
    r#"(?i)(curl\b[^\n]*\s-u\s+)([^:]+):(?:"[^"]*"|'[^']*'|[^"'\s]+)"#,
    r#"(?i)(curl\b[^\n]*\s--user\s+)([^:]+):(?:"[^"]*"|'[^']*'|[^"'\s]+)"#,
    r#"(?i)(wget\b[^\n]*\s--password=)([^\s"']+)"#,
    r#"(?i)(redis-cli\b[^\n]*\s-a\s+)([^\s"']+)"#,
    r#"(?i)([a-z][a-z0-9+.-]*://[^:"'\s/@]+:)([^@"'\s]+)(@)"#,
    r#"(?i)(kubectl\b[^\n]*--from-literal=)([^\s"']+)"#,
];

const MASKING_REPLACEMENTS: &[&str] = &[
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1****",
    "$1$2:****",
    "$1$2:****",
    "$1****",
    "$1****",
    "$1****$3",
    "$1****",
];

fn masking_regexes() -> &'static Vec<Regex> {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        MASKING_PATTERNS
            .iter()
            .map(|pattern| Regex::new(pattern).expect("masking regex is valid"))
            .collect()
    })
}

fn masking_replacements() -> &'static [&'static str] {
    MASKING_REPLACEMENTS
}

fn authorization_header_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)Authorization:\s*(?:Bearer|Basic)\s+\S+"#)
            .expect("authorization header regex is valid")
    })
}

fn export_secret_env_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?:^|[\s;&|]|^export\s+)[A-Za-z_]*(?:PASSWORD|PASSWD|TOKEN|SECRET|API[_-]?KEY|AWS_SECRET_ACCESS_KEY|AWS_ACCESS_KEY_ID|GITHUB_TOKEN|GITLAB_TOKEN|MYSQL_PWD)[A-Za-z_]*\s*=\S+"#,
        )
        .expect("export secret env regex is valid")
    })
}

fn pgpassword_env_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)PGPASSWORD=\S+").expect("pgpassword env regex is valid"))
}

fn credit_card_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:\d[ -]*?){13,19}\b").expect("credit card regex is valid"))
}

fn long_flag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)--(?:password|passwd|token|secret|api[_-]?key)(?:=\S+|\s+\S+)")
            .expect("long flag regex is valid")
    })
}

fn mysql_password_flag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?:mysql|mariadb|mysqldump)\b[^\n]*-p(?:"[^"]*"|'[^']*'|\S+|\s+\S+)"#,
        )
        .expect("mysql password regex is valid")
    })
}

fn docker_login_password_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)docker\s+login\b[^\n]*-p(?:\s+\S+|\S+)")
            .expect("docker login regex is valid")
    })
}

fn curl_user_password_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)curl\b[^\n]*\s-u\s+[^:]+:[^\s]+"#)
            .expect("curl user password regex is valid")
    })
}

fn curl_long_user_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)curl\b[^\n]*\s--user\s+[^:]+:[^\s]+")
            .expect("curl long user regex is valid")
    })
}

fn wget_password_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)wget\b[^\n]*\s--password=[^\s]+").expect("wget password regex is valid")
    })
}

fn redis_auth_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)redis-cli\b[^\n]*\s-a\s+\S+").expect("redis auth regex is valid")
    })
}

fn has_hide_prefix(command: &str) -> bool {
    command
        .chars()
        .next()
        .is_some_and(|character| matches!(character, ' ' | '\t'))
}

fn sshpass_env_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)SSHPASS=\S+").expect("sshpass env regex is valid"))
}

fn sshpass_password_flag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)sshpass\s+-p\s+(?:"[^"]*"|'[^']*'|\S+)"#)
            .expect("sshpass password flag regex is valid")
    })
}

fn db_connection_url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)[a-z][a-z0-9+.-]*://[^:"'\s/@]+:[^@"'\s]+@"#)
            .expect("database connection url regex is valid")
    })
}

fn kubectl_secret_literal_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)kubectl\b[^\n]*\b(?:create|apply)\b[^\n]*\bsecret\b[^\n]*--from-literal=[^\s"']+"#,
        )
        .expect("kubectl secret literal regex is valid")
    })
}

fn contains_luhn_credit_card(command: &str) -> bool {
    credit_card_regex()
        .find_iter(command)
        .any(|candidate| luhn_valid(&digits_only(candidate.as_str())))
}

fn mask_luhn_credit_cards(command: &str) -> String {
    credit_card_regex()
        .replace_all(command, |caps: &regex::Captures| {
            let matched = caps.get(0).map(|value| value.as_str()).unwrap_or("");
            if luhn_valid(&digits_only(matched)) {
                "****".to_string()
            } else {
                matched.to_string()
            }
        })
        .into_owned()
}

fn digits_only(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn luhn_valid(digits: &str) -> bool {
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let mut sum = 0u32;
    let mut double = false;
    for digit in digits.chars().rev().filter_map(|character| character.to_digit(10)) {
        let mut value = digit;
        if double {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
        double = !double;
    }
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn engine_masks_docker_login_password() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let original = std::env::var("XDG_CONFIG_HOME").ok();
        // SAFETY: test runs in isolation and restores the previous config home.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let result = {
            let config = AppConfig::default();
            let engine = SecurityEngine::from_config(&config).expect("engine should compile");
            let decision = engine
                .process("docker login -u user -p secret", &config)
                .expect("process should succeed");
            assert_eq!(decision.action, SecurityAction::Masked);
            assert!(!decision.command.contains("secret"));
            Ok::<(), anyhow::Error>(())
        };

        // SAFETY: restores the environment for other tests in this process.
        unsafe {
            match original {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }

        result.expect("docker login masking should succeed");
    }

    #[test]
    fn skips_when_secret_cannot_be_masked() {
        let config = AppConfig::default();
        let decision = process_command("SSHPASS=topsecret sshpass -e ssh host", &config)
            .expect("process should succeed");
        assert!(matches!(
            decision.action,
            SecurityAction::Masked | SecurityAction::Skipped(_)
        ));
        assert!(!decision.command.contains("topsecret"));
    }
}
