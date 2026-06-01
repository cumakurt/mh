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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionGuidance {
    pub safer_alternative: Option<String>,
    pub preview_command: Option<String>,
    pub checklist: Vec<String>,
}

const RISK_PATTERNS: &[(&str, &str, RiskLevel, &str)] = &[
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

    for rule in compiled_rules() {
        if rule.regex.is_match(trimmed) {
            return Some(RiskAssessment {
                level: rule.level,
                rule_id: rule.rule_id.to_string(),
                description: rule.description.to_string(),
            });
        }
    }

    None
}

pub fn is_at_least(level: RiskLevel, minimum: RiskLevel) -> bool {
    level >= minimum
}

pub fn execution_guidance(command: &str) -> ExecutionGuidance {
    let trimmed = command.trim();
    let lower = trimmed.to_ascii_lowercase();
    let mut checklist = vec![
        "verify the target host, cwd, and environment".to_string(),
        "confirm backups or rollback path exist".to_string(),
    ];

    let (safer_alternative, preview_command) = if lower.starts_with("rm ") || lower.contains(" rm ")
    {
        checklist.push(
            "prefer moving files to a quarantine directory before permanent delete".to_string(),
        );
        (
            Some("Review targets first with: find <path> -maxdepth 1 -print".to_string()),
            Some(format!("printf '%s\n' {}", shell_quote(trimmed))),
        )
    } else if lower.starts_with("kubectl delete ") {
        (
            Some(trimmed.replacen("kubectl delete", "kubectl get", 1)),
            Some(format!("{trimmed} --dry-run=server -o yaml")),
        )
    } else if lower.starts_with("kubectl apply ") {
        (
            Some("kubectl diff -f <manifest>".to_string()),
            Some(format!("{trimmed} --dry-run=server -o yaml")),
        )
    } else if lower.starts_with("helm upgrade ") || lower.starts_with("helm install ") {
        (
            Some(trimmed.replacen("helm ", "helm --dry-run ", 1)),
            Some(format!("{trimmed} --dry-run --debug")),
        )
    } else if lower.starts_with("terraform apply") || lower.starts_with("terraform destroy") {
        (
            Some("terraform plan -out=tfplan".to_string()),
            Some("terraform plan".to_string()),
        )
    } else if lower.starts_with("ansible-playbook ") {
        (
            Some(format!("{trimmed} --check --diff")),
            Some(format!("{trimmed} --check --diff")),
        )
    } else if lower.starts_with("docker system prune") {
        (
            Some("docker system df && docker image ls".to_string()),
            None,
        )
    } else {
        (None, None)
    };

    ExecutionGuidance {
        safer_alternative,
        preview_command,
        checklist,
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

struct CompiledRiskRule {
    regex: Regex,
    rule_id: &'static str,
    level: RiskLevel,
    description: &'static str,
}

fn compiled_rules() -> &'static [CompiledRiskRule] {
    static RULES: OnceLock<Vec<CompiledRiskRule>> = OnceLock::new();

    RULES.get_or_init(|| {
        RISK_PATTERNS
            .iter()
            .filter_map(|(pattern, rule_id, level, description)| {
                Regex::new(pattern).ok().map(|regex| CompiledRiskRule {
                    regex,
                    rule_id,
                    level: *level,
                    description,
                })
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

    #[test]
    fn built_in_risk_regexes_compile() {
        for (pattern, rule_id, ..) in RISK_PATTERNS {
            assert!(
                Regex::new(pattern).is_ok(),
                "risk rule {rule_id} has an invalid regex"
            );
        }
    }

    #[test]
    fn guidance_suggests_kubectl_preview() {
        let guidance = execution_guidance("kubectl delete deploy app");
        assert!(
            guidance
                .preview_command
                .as_deref()
                .is_some_and(|command| command.contains("--dry-run=server"))
        );
    }
}
