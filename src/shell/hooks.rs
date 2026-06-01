//! Shell hook markers, duplicate detection, and config repair helpers.

use crate::cli::ShellKind;

pub const BEGIN_MARKER: &str = "# >>> mh shell integration >>>";
pub const END_MARKER: &str = "# <<< mh shell integration <<<";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepairReport {
    pub removed_managed_blocks: usize,
    pub removed_duplicate_hook_lines: usize,
}

impl RepairReport {
    pub fn changed(&self) -> bool {
        self.removed_managed_blocks > 0 || self.removed_duplicate_hook_lines > 0
    }
}

pub fn duplicate_hook_count(shell: ShellKind, content: &str) -> usize {
    duplicate_patterns(shell)
        .iter()
        .map(|pattern| content.matches(pattern).count().saturating_sub(1))
        .sum()
}

pub fn repair_content(content: &str, shell: ShellKind) -> (String, RepairReport) {
    let mut report = RepairReport::default();
    let (body, kept_block) = strip_managed_blocks(content, &mut report);
    let (body, removed_lines) = dedupe_hook_lines(&body, shell);
    report.removed_duplicate_hook_lines = removed_lines;

    let mut repaired = body;
    if let Some(block) = kept_block {
        if !repaired.is_empty() && !repaired.ends_with('\n') {
            repaired.push('\n');
        }
        repaired.push_str(&block);
    }

    (trim_trailing_blank_lines(&repaired), report)
}

fn strip_managed_blocks(content: &str, report: &mut RepairReport) -> (String, Option<String>) {
    let mut body = String::new();
    let mut kept_block = None;
    let mut in_block = false;
    let mut current_block = String::new();

    for line in content.lines() {
        if line.contains(BEGIN_MARKER) {
            in_block = true;
            current_block.clear();
            current_block.push_str(line);
            current_block.push('\n');
            continue;
        }

        if in_block {
            current_block.push_str(line);
            current_block.push('\n');
            if line.contains(END_MARKER) {
                in_block = false;
                if kept_block.is_none() {
                    kept_block = Some(current_block.clone());
                } else {
                    report.removed_managed_blocks += 1;
                }
                current_block.clear();
            }
            continue;
        }

        body.push_str(line);
        body.push('\n');
    }

    if in_block && !current_block.is_empty() {
        body.push_str(&current_block);
    }

    (body, kept_block)
}

fn dedupe_hook_lines(content: &str, shell: ShellKind) -> (String, usize) {
    let patterns = duplicate_patterns(shell);
    let mut seen = std::collections::HashSet::new();
    let mut removed = 0;
    let mut lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }

        if patterns.iter().any(|pattern| trimmed.contains(pattern)) {
            if seen.contains(trimmed) {
                removed += 1;
                continue;
            }
            seen.insert(trimmed.to_string());
        }

        lines.push(line.to_string());
    }

    (lines.join("\n"), removed)
}

fn duplicate_patterns(shell: ShellKind) -> &'static [&'static str] {
    match shell {
        ShellKind::Zsh => &[
            "add-zsh-hook preexec _mh_preexec",
            "add-zsh-hook precmd _mh_precmd",
            "eval \"$(mh init zsh)\"",
            "mh init zsh",
        ],
        ShellKind::Bash => &[
            "trap '__mh_preexec' DEBUG",
            "__mh_precmd",
            "eval \"$(mh init bash)\"",
            "mh init bash",
        ],
        ShellKind::Fish => &[
            "function mh_preexec --on-event fish_preexec",
            "function mh_postexec --on-event fish_postexec",
            "mh init fish | source",
        ],
        ShellKind::Nushell => &[
            "$env.MH_LAST_COMMAND = $cmd",
            "MH_LAST_COMMAND = $cmd",
            "pre_execution:",
            "^mh record",
            "mh init nushell",
        ],
        ShellKind::Sh | ShellKind::Auto => &[
            "__mh_before_prompt",
            "MH_PENDING_CMD",
            "mh init sh",
            "eval \"$(mh init sh)\"",
        ],
        ShellKind::Pwsh => &[
            "__mh_pwsh_loaded",
            "AddToHistoryHandler",
            "mh init pwsh",
        ],
    }
}

fn trim_trailing_blank_lines(content: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_duplicate_managed_blocks() {
        let content = format!(
            "{BEGIN_MARKER}\neval \"$(mh init zsh)\"\n{END_MARKER}\n{BEGIN_MARKER}\neval \"$(mh init zsh)\"\n{END_MARKER}\n"
        );
        let (repaired, report) = repair_content(&content, ShellKind::Zsh);
        assert_eq!(report.removed_managed_blocks, 1);
        assert_eq!(repaired.matches(BEGIN_MARKER).count(), 1);
    }

    #[test]
    fn removes_duplicate_zsh_hook_lines() {
        let content = "add-zsh-hook preexec _mh_preexec\nadd-zsh-hook preexec _mh_preexec\n";
        let (repaired, report) = repair_content(content, ShellKind::Zsh);
        assert_eq!(report.removed_duplicate_hook_lines, 1);
        assert_eq!(
            repaired.matches("add-zsh-hook preexec _mh_preexec").count(),
            1
        );
    }

    #[test]
    fn counts_duplicate_zsh_hooks() {
        let content = "add-zsh-hook preexec _mh_preexec\nadd-zsh-hook preexec _mh_preexec";
        assert_eq!(duplicate_hook_count(ShellKind::Zsh, content), 1);
    }

    #[test]
    fn preserves_unclosed_managed_block_content() {
        let content = format!("prefix line\n{BEGIN_MARKER}\nhook body without end marker\n");
        let (repaired, _report) = repair_content(&content, ShellKind::Zsh);
        assert!(repaired.contains("prefix line"));
        assert!(repaired.contains(BEGIN_MARKER));
        assert!(repaired.contains("hook body without end marker"));
    }
}
