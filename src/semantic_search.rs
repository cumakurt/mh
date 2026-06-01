use chrono::{Datelike, Duration, Local, TimeZone};

use crate::models::{CommandRow, SearchFilters};
use crate::risk::{self, RiskLevel};

#[derive(Debug, Clone)]
pub struct SemanticSearchPlan {
    pub filters: SearchFilters,
    pub terms: Vec<String>,
    pub minimum_risk: Option<RiskLevel>,
    pub explanations: Vec<String>,
}

pub fn build_plan(query: &str, limit: usize) -> SemanticSearchPlan {
    let normalized = query.to_ascii_lowercase();
    let tokens = normalized
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    let fetch_limit = limit.saturating_mul(50).min(5_000).max(limit);
    let mut filters = SearchFilters {
        query: None,
        cwd: None,
        failed: contains_any(&tokens, &["failed", "fail", "error", "hata", "başarısız"]),
        success: contains_any(&tokens, &["successful", "success", "ok", "başarılı"]),
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: None,
        category: category_from_tokens(&tokens),
        pinned: contains_any(&tokens, &["pinned", "pinli", "favorite", "favori"]),
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: contains_any(&tokens, &["ssh", "remote", "uzak"]),
        root: contains_any(&tokens, &["root", "sudo"]),
        limit: fetch_limit,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: environment_from_tokens(&tokens),
    };
    let mut explanations = Vec::new();

    if filters.failed {
        explanations.push("failed commands".to_string());
    }
    if filters.success {
        explanations.push("successful commands".to_string());
    }
    if filters.pinned {
        explanations.push("pinned commands".to_string());
    }
    if filters.ssh {
        explanations.push("SSH commands".to_string());
    }
    if filters.root {
        explanations.push("root commands".to_string());
    }
    if let Some(category) = &filters.category {
        explanations.push(format!("{category} category"));
    }
    if let Some(environment) = &filters.environment {
        explanations.push(format!("{environment} environment"));
    }

    if contains_any(&tokens, &["today", "bugün"]) {
        filters.after = Some(local_day_start(0));
        explanations.push("today".to_string());
    } else if contains_any(&tokens, &["yesterday", "dün"]) {
        filters.after = Some(local_day_start(1));
        filters.before = Some(local_day_start(0));
        explanations.push("yesterday".to_string());
    } else if normalized.contains("last week") || normalized.contains("son hafta") {
        filters.after = Some((Local::now() - Duration::days(7)).to_rfc3339());
        explanations.push("last 7 days".to_string());
    } else if normalized.contains("last month") || normalized.contains("son ay") {
        filters.after = Some((Local::now() - Duration::days(30)).to_rfc3339());
        explanations.push("last 30 days".to_string());
    }

    let minimum_risk = if contains_any(&tokens, &["critical", "kritik"]) {
        explanations.push("critical risk".to_string());
        Some(RiskLevel::Critical)
    } else if contains_any(&tokens, &["risky", "risk", "high", "tehlikeli", "yüksek"]) {
        explanations.push("risky commands".to_string());
        Some(RiskLevel::Medium)
    } else {
        None
    };

    let terms = tokens
        .into_iter()
        .filter(|token| !STOPWORDS.contains(token))
        .filter(|token| !SEMANTIC_KEYWORDS.contains(token))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    SemanticSearchPlan {
        filters,
        terms,
        minimum_risk,
        explanations,
    }
}

pub fn rank_rows(
    mut rows: Vec<CommandRow>,
    plan: &SemanticSearchPlan,
    limit: usize,
) -> Vec<CommandRow> {
    let mut scored = rows
        .drain(..)
        .filter_map(|row| {
            let risk = risk::assess_command(&row.command);
            if let Some(minimum) = plan.minimum_risk
                && !risk
                    .as_ref()
                    .is_some_and(|assessment| risk::is_at_least(assessment.level, minimum))
            {
                return None;
            }

            let score = score_row(&row, &plan.terms, risk.as_ref().map(|value| value.level));
            (score > 0 || plan.terms.is_empty()).then_some((score, row))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.id.cmp(&left.1.id))
    });
    scored.into_iter().map(|(_, row)| row).take(limit).collect()
}

fn score_row(row: &CommandRow, terms: &[String], risk_level: Option<RiskLevel>) -> i64 {
    let command = row.command.to_ascii_lowercase();
    let cwd = row.cwd.as_deref().unwrap_or_default().to_ascii_lowercase();
    let category = row
        .category
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tags = row.tags.join(" ").to_ascii_lowercase();
    let env = row
        .environment_tier
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut score = 0;

    for term in terms {
        let synonyms = synonyms(term);
        for candidate in std::iter::once(term.as_str()).chain(synonyms.iter().copied()) {
            if command.contains(candidate) {
                score += 8;
            }
            if cwd.contains(candidate) {
                score += 4;
            }
            if category.contains(candidate) || tags.contains(candidate) || env.contains(candidate) {
                score += 6;
            }
        }
    }

    if row.exit_code.is_some_and(|code| code != 0) {
        score += 1;
    }
    if row.is_pinned {
        score += 1;
    }
    if risk_level.is_some() {
        score += 2;
    }
    score
}

fn contains_any(tokens: &[&str], needles: &[&str]) -> bool {
    needles.iter().any(|needle| tokens.contains(needle))
}

fn category_from_tokens(tokens: &[&str]) -> Option<String> {
    let categories: &[(&str, &[&str])] = &[
        ("docker", &["docker", "podman", "container", "konteyner"]),
        ("git", &["git", "commit", "branch", "repo"]),
        ("network", &["network", "curl", "wget", "ssh", "ağ"]),
        (
            "package",
            &["package", "install", "apt", "cargo", "pip", "paket"],
        ),
        ("system", &["system", "systemctl", "journalctl", "servis"]),
    ];
    categories.iter().find_map(|(category, aliases)| {
        aliases
            .iter()
            .any(|alias| tokens.contains(alias))
            .then(|| (*category).to_string())
    })
}

fn environment_from_tokens(tokens: &[&str]) -> Option<String> {
    if contains_any(tokens, &["prod", "production", "canlı"]) {
        Some("production".to_string())
    } else if contains_any(tokens, &["stage", "staging"]) {
        Some("staging".to_string())
    } else if contains_any(tokens, &["dev", "development", "geliştirme"]) {
        Some("development".to_string())
    } else {
        None
    }
}

fn local_day_start(days_ago: i64) -> String {
    let date = Local::now().date_naive() - Duration::days(days_ago);
    Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .single()
        .map(|datetime| datetime.to_rfc3339())
        .unwrap_or_else(|| Local::now().to_rfc3339())
}

fn synonyms(term: &str) -> &'static [&'static str] {
    match term {
        "deploy" | "deployment" | "dağıtım" => &["kubectl apply", "helm upgrade", "deploy"],
        "kubernetes" | "k8s" | "pod" => &["kubectl", "helm", "pod"],
        "database" | "db" | "veritabanı" => &["psql", "mysql", "redis", "sqlite"],
        "secret" | "token" | "credential" => &["password", "token", "secret", "api_key"],
        "cleanup" | "temizlik" => &["rm ", "delete", "prune", "clean"],
        _ => &[],
    }
}

const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "the",
    "to",
    "in",
    "on",
    "for",
    "of",
    "show",
    "find",
    "list",
    "commands",
    "command",
    "komut",
    "komutları",
    "göster",
    "bul",
    "listele",
    "ve",
    "ile",
    "için",
    "son",
];

const SEMANTIC_KEYWORDS: &[&str] = &[
    "failed",
    "fail",
    "error",
    "hata",
    "başarısız",
    "successful",
    "success",
    "ok",
    "başarılı",
    "today",
    "bugün",
    "yesterday",
    "dün",
    "last",
    "week",
    "month",
    "hafta",
    "ay",
    "prod",
    "production",
    "canlı",
    "stage",
    "staging",
    "dev",
    "development",
    "geliştirme",
    "pinned",
    "pinli",
    "favorite",
    "favori",
    "ssh",
    "remote",
    "uzak",
    "root",
    "sudo",
    "critical",
    "kritik",
    "risky",
    "risk",
    "high",
    "tehlikeli",
    "yüksek",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_filters_from_natural_language() {
        let plan = build_plan("bugün prod başarısız deploy komutları", 10);
        assert!(plan.filters.failed);
        assert_eq!(plan.filters.environment.as_deref(), Some("production"));
        assert!(plan.filters.after.is_some());
        assert!(plan.terms.contains(&"deploy".to_string()));
    }
}
