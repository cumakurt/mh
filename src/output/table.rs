use anyhow::Result;

use crate::config::AppConfig;
use crate::models::CommandRow;
use crate::output::styling::Styler;
use crate::output::table_format::{header_cell, new_table, print_table, truncate_display};

pub fn print(rows: &[CommandRow]) -> Result<()> {
    let config = AppConfig::load()?;
    print_with_styler(rows, &Styler::from_config(&config))
}

pub fn print_with_styler(rows: &[CommandRow], styler: &Styler) -> Result<()> {
    if rows.is_empty() {
        println!();
        println!("{}", styler.muted("No matching commands."));
        return Ok(());
    }

    let mut table = new_table();
    table.set_header(vec![
        header_cell(styler, "ID"),
        header_cell(styler, "Time"),
        header_cell(styler, "Exit"),
        header_cell(styler, "Duration"),
        header_cell(styler, "CWD"),
        header_cell(styler, "Category"),
        header_cell(styler, "Tags"),
        header_cell(styler, "Command"),
    ]);

    let cwd_max = 28usize;
    let cmd_max = 64usize;

    for row in rows {
        table.add_row(vec![
            styler.cell(row.id, None),
            styler.cell(format_time(&row.started_at), None),
            styler.exit_code_cell(row.exit_code),
            styler.cell(
                row.duration_ms
                    .map(|value| format!("{value}ms"))
                    .unwrap_or_else(|| "-".to_string()),
                None,
            ),
            styler.cell(
                row.cwd
                    .as_deref()
                    .map(|cwd| truncate_display(cwd, cwd_max))
                    .unwrap_or_else(|| "-".to_string()),
                None,
            ),
            styler.category_cell(row.category.as_deref()),
            styler.tag_cell(&row.tags),
            styler.command_cell_truncated(row, cmd_max),
        ]);
    }

    print_table(&table);
    Ok(())
}

fn format_time(value: &str) -> String {
    let normalized = value
        .replace('T', " ")
        .replace("+00:00", "")
        .replace('Z', "");
    truncate_display(&normalized, 19)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_history_table_with_rounded_borders() {
        let rows = [CommandRow {
            id: 1,
            command: "echo hello".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("zsh".to_string()),
            username: None,
            hostname: None,
            exit_code: Some(0),
            duration_ms: Some(12),
            started_at: "2026-05-31T12:00:00Z".to_string(),
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            category: None,
            tags: Vec::new(),
            is_pinned: false,
            is_masked: false,
            environment_tier: None,
        }];

        let styler = Styler::from_display_config(false);
        let mut table = new_table();
        table.set_header(vec![header_cell(&styler, "ID"), header_cell(&styler, "Command")]);
        table.add_row(vec![
            styler.cell(rows[0].id, None),
            styler.cell(&rows[0].command, None),
        ]);

        let rendered = format!("{table}");
        assert!(rendered.contains("echo hello"));
        assert!(rendered.contains('┌'));
    }
}
