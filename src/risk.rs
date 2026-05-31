use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskRule {
    pub id: &'static str,
    pub level: RiskLevel,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub rule_id: String,
    pub description: String,
}

pub fn list_rules() -> &'static [RiskRule] {
    static RULES: &[RiskRule] = &[
        RiskRule {
            id: "rm_recursive_root",
            level: RiskLevel::Critical,
            description: "Recursive delete targeting root or system paths",
        },
        RiskRule {
            id: "disk_overwrite",
            level: RiskLevel::Critical,
            description: "Direct disk overwrite with dd or similar",
        },
        RiskRule {
            id: "filesystem_format",
            level: RiskLevel::Critical,
            description: "Filesystem formatting command",
        },
        RiskRule {
            id: "fork_bomb",
            level: RiskLevel::Critical,
            description: "Fork bomb or runaway process pattern",
        },
        RiskRule {
            id: "permission_chown_root",
            level: RiskLevel::High,
            description: "Recursive permission or ownership change on broad paths",
        },
        RiskRule {
            id: "iptables_flush",
            level: RiskLevel::High,
            description: "Firewall rules flush or disable",
        },
        RiskRule {
            id: "shutdown_reboot",
            level: RiskLevel::High,
            description: "Immediate shutdown or reboot",
        },
        RiskRule {
            id: "curl_pipe_shell",
            level: RiskLevel::High,
            description: "Remote script piped directly into a shell",
        },
        RiskRule {
            id: "recursive_delete",
            level: RiskLevel::Medium,
            description: "Recursive delete on a non-trivial path",
        },
        RiskRule {
            id: "chmod_recursive",
            level: RiskLevel::Medium,
            description: "Recursive permission change",
        },
    ];
    RULES
}

pub fn assess_command(command: &str) -> Option<RiskAssessment> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    for (regex, rule_id, level, description) in compiled_rules() {
        if regex.is_match(trimmed) {
            return Some(RiskAssessment {
                level: *level,
                rule_id: (*rule_id).to_string(),
                description: (*description).to_string(),
            });
        }
    }

    None
}

pub fn is_at_least(level: RiskLevel, minimum: RiskLevel) -> bool {
    level >= minimum
}

fn compiled_rules() -> &'static [(&'static Regex, &'static str, RiskLevel, &'static str)] {
    static RULES: OnceLock<Vec<(&'static Regex, &'static str, RiskLevel, &'static str)>> =
        OnceLock::new();

    RULES.get_or_init(|| {
        let patterns: &[(&str, &str, RiskLevel, &str)] = &[
            (
                r"(?i)\brm\s+(-[^\s]*r[^\s]*\s+|-[^\s]*f[^\s]*\s+)*(/|\~|/\*|/\.\.?|/\S*)",
                "rm_recursive_root",
                RiskLevel::Critical,
                "Recursive delete targeting root or system paths",
            ),
            (
                r"(?i)\bdd\s+if=/dev/(sd[a-z]|nvme\d+n\d+|vd[a-z]|xvd[a-z])",
                "disk_overwrite",
                RiskLevel::Critical,
                "Direct disk overwrite with dd or similar",
            ),
            (
                r"(?i)\b(mkfs\.|mkfs\s|mke2fs|wipefs|parted\s+.*mklabel)",
                "filesystem_format",
                RiskLevel::Critical,
                "Filesystem formatting command",
            ),
            (
                r":\(\)\{\s*:\|\:&\s*\};:",
                "fork_bomb",
                RiskLevel::Critical,
                "Fork bomb or runaway process pattern",
            ),
            (
                r"(?i)\b(chmod|chown)\s+(-[^\s]*R[^\s]*\s+|-[^\s]*r[^\s]*\s+)*(/|\~|/\*)",
                "permission_chown_root",
                RiskLevel::High,
                "Recursive permission or ownership change on broad paths",
            ),
            (
                r"(?i)\biptables\s+(-[^\s]*F[^\s]*\s+|-F\b|--flush\b)",
                "iptables_flush",
                RiskLevel::High,
                "Firewall rules flush or disable",
            ),
            (
                r"(?i)\b(shutdown|reboot|poweroff|init\s+0|systemctl\s+(poweroff|reboot|halt))\b",
                "shutdown_reboot",
                RiskLevel::High,
                "Immediate shutdown or reboot",
            ),
            (
                r"(?i)(curl|wget)\s+[^\n|]+\|\s*(ba)?sh\b",
                "curl_pipe_shell",
                RiskLevel::High,
                "Remote script piped directly into a shell",
            ),
            (
                r"(?i)\brm\s+(-[^\s]*r[^\s]*\s+|-[^\s]*f[^\s]*\s+)+[^\s]+",
                "recursive_delete",
                RiskLevel::Medium,
                "Recursive delete on a non-trivial path",
            ),
            (
                r"(?i)\bchmod\s+(-[^\s]*R[^\s]*\s+|-[^\s]*r[^\s]*\s+)+",
                "chmod_recursive",
                RiskLevel::Medium,
                "Recursive permission change",
            ),
        ];

        let regexes: Vec<Regex> = patterns
            .iter()
            .map(|(pattern, ..)| Regex::new(pattern).expect("risk regex is valid"))
            .collect();

        regexes
            .into_iter()
            .enumerate()
            .map(|(index, regex)| {
                let (_, rule_id, level, description) = patterns[index];
                (
                    Box::leak(Box::new(regex)) as &'static Regex,
                    rule_id,
                    level,
                    description,
                )
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_critical_rm_rf_root() {
        let assessment = assess_command("rm -rf /").expect("command should be flagged");
        assert_eq!(assessment.level, RiskLevel::Critical);
        assert_eq!(assessment.rule_id, "rm_recursive_root");
    }

    #[test]
    fn flags_high_curl_pipe_shell() {
        let assessment = assess_command("curl https://example.com/install.sh | bash")
            .expect("command should be flagged");
        assert_eq!(assessment.level, RiskLevel::High);
    }

    #[test]
    fn ignores_safe_ls_command() {
        assert!(assess_command("ls -la").is_none());
    }
}
