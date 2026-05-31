use std::collections::BTreeMap;

pub fn classify_command(
    command: &str,
    categories: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let command = command.trim_start();
    if command.is_empty() {
        return None;
    }

    for (category, prefixes) in categories {
        if prefixes.iter().any(|prefix| command.starts_with(prefix)) {
            return Some(category.clone());
        }
    }

    let first_word = shell_words::split(command)
        .ok()
        .and_then(|parts| parts.into_iter().next())
        .unwrap_or_else(|| {
            command
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        });

    match first_word.as_str() {
        "git" | "gh" | "hub" => Some("git".to_string()),
        "docker" | "docker-compose" | "podman" => Some("docker".to_string()),
        "curl" | "wget" | "ssh" | "nc" | "nmap" | "ping" => Some("network".to_string()),
        "systemctl" | "journalctl" | "top" | "htop" => Some("system".to_string()),
        "apt" | "apt-get" | "dpkg" | "snap" | "cargo" | "pip" => Some("package".to_string()),
        _ => None,
    }
}
