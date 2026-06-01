use crate::models::CommandRow;

pub fn format_rows(rows: &[CommandRow]) -> String {
    let mut output =
        String::from("id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags\n");
    for row in rows {
        output.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            row.id,
            csv_escape(&row.started_at),
            row.exit_code
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.duration_ms
                .map(|value| value.to_string())
                .unwrap_or_default(),
            csv_escape(row.cwd.as_deref().unwrap_or_default()),
            csv_escape(row.shell.as_deref().unwrap_or_default()),
            csv_escape(row.category.as_deref().unwrap_or_default()),
            csv_escape(&row.command),
            csv_escape(&row.tags.join(",")),
        ));
    }
    output
}

fn csv_escape(value: &str) -> String {
    let safe_value = spreadsheet_safe_value(value);
    if safe_value.contains(',') || safe_value.contains('"') || safe_value.contains('\n') {
        format!("\"{}\"", safe_value.replace('"', "\"\""))
    } else {
        safe_value
    }
}

fn spreadsheet_safe_value(value: &str) -> String {
    let trimmed = value.trim_start();
    let starts_with_control_prefix = value
        .chars()
        .next()
        .is_some_and(|character| matches!(character, '\t' | '\r'));
    if trimmed
        .chars()
        .next()
        .is_some_and(|character| matches!(character, '=' | '+' | '-' | '@'))
        || starts_with_control_prefix
    {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CommandRow;

    #[test]
    fn csv_escapes_spreadsheet_formulas() {
        let row = CommandRow {
            id: 1,
            command: "=cmd|'/C calc'!A0".to_string(),
            cwd: Some("+network".to_string()),
            shell: Some("zsh".to_string()),
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

        assert!(output.contains("'+network"));
        assert!(output.contains("'=cmd|'/C calc'!A0"));
    }
}
