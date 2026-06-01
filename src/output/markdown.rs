use crate::models::CommandRow;

pub fn format_rows(rows: &[CommandRow]) -> String {
    let mut output =
        String::from("| ID | Time | Exit | CWD | Command |\n|---:|---|---:|---|---|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | {} | {} | {} | `{}` |\n",
            row.id,
            row.started_at,
            row.exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            markdown_escape(row.cwd.as_deref().unwrap_or("-")),
            markdown_escape(&row.command),
        ));
    }
    output
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CommandRow;

    #[test]
    fn markdown_rows_keep_multiline_commands_inside_one_table_row() {
        let row = CommandRow {
            id: 1,
            command: "echo `one`\necho two | cat".to_string(),
            cwd: Some("/tmp/a|b".to_string()),
            shell: None,
            username: None,
            hostname: None,
            exit_code: Some(0),
            duration_ms: None,
            started_at: "2026-06-01T00:00:00Z".to_string(),
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            category: None,
            tags: Vec::new(),
            is_pinned: false,
            is_masked: false,
            environment_tier: None,
        };

        let output = format_rows(&[row]);

        assert!(output.contains("echo \\`one\\`\\necho two \\| cat"));
        assert_eq!(output.lines().count(), 3);
    }
}
