use anyhow::Result;
use comfy_table::{Cell, Table};

use crate::config::AppConfig;
use crate::models::CommandRow;
use crate::output::styling::Styler;

/// Single-line UTF-8 borders without double or dotted separators.
const HISTORY_TABLE_STYLE: &str = "││──╞─╪╡│    ┬┴┌┐└┘";

pub fn print(rows: &[CommandRow]) -> Result<()> {
    let config = AppConfig::load()?;
    print_with_styler(rows, &Styler::from_config(&config))
}

pub fn print_with_styler(rows: &[CommandRow], styler: &Styler) -> Result<()> {
    let mut table = Table::new();
    table.load_preset(HISTORY_TABLE_STYLE);
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
            styler.cell(row.cwd.as_deref().unwrap_or("-"), None),
            styler.category_cell(row.category.as_deref()),
            styler.tag_cell(&row.tags),
            styler.command_cell(row),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn header_cell(styler: &Styler, label: &str) -> Cell {
    styler.cell(
        label,
        if styler.enabled() {
            Some(comfy_table::Color::Cyan)
        } else {
            None
        },
    )
}

fn format_time(value: &str) -> String {
    value
        .replace('T', " ")
        .replace("+00:00", "")
        .replace('Z', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_table_style_has_expected_component_count() {
        assert_eq!(HISTORY_TABLE_STYLE.chars().count(), 19);
    }

    #[test]
    fn renders_history_table_without_garbled_borders() {
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

        let mut buffer = Vec::new();
        {
            use std::io::Write;
            let capture = &mut buffer;
            let mut table = Table::new();
            table.load_preset(HISTORY_TABLE_STYLE);
            table.set_header(vec!["ID", "Command"]);
            table.add_row(vec![Cell::new(rows[0].id), Cell::new(&rows[0].command)]);
            writeln!(capture, "{table}").expect("table should render");
        }

        let rendered = String::from_utf8(buffer).expect("table output should be utf-8");
        assert!(rendered.contains("echo hello"));
        assert!(rendered.contains('┌'));
        assert!(rendered.contains('┐'));
        assert!(rendered.contains('└'));
        assert!(rendered.contains('┘'));
        assert!(!rendered.starts_with('┴'));
    }
}
