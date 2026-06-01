//! Shared terminal table layout: width-aware, spaced sections, rounded borders.

use std::io::{self, IsTerminal};

use comfy_table::{Cell, ContentArrangement, Table};
use comfy_table::presets::UTF8_BORDERS_ONLY;

use crate::models::StatEntry;
use crate::output::styling::Styler;

const TABLE_MARGIN_COLUMNS: u16 = 4;
const MIN_TABLE_WIDTH: u16 = 52;
const DEFAULT_TABLE_WIDTH: u16 = 120;

/// Visible terminal width for table layout (honours `COLUMNS` when set).
pub fn terminal_columns() -> u16 {
    crossterm::terminal::size()
        .map(|(cols, _)| cols)
        .ok()
        .or_else(|| std::env::var("COLUMNS").ok().and_then(|value| value.parse().ok()))
        .unwrap_or(DEFAULT_TABLE_WIDTH)
        .clamp(MIN_TABLE_WIDTH, 256)
}

/// Table width with side margins so borders do not wrap on narrow terminals.
pub fn table_width() -> u16 {
    terminal_columns().saturating_sub(TABLE_MARGIN_COLUMNS)
}

/// New table with rounded UTF-8 borders, dynamic wrapping, and terminal-aware width.
pub fn new_table() -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_width(table_width());
    table
}

pub fn header_cell(styler: &Styler, label: &str) -> Cell {
    styler.cell(
        label,
        if styler.enabled() {
            Some(comfy_table::Color::Cyan)
        } else {
            None
        },
    )
}

/// Prints a table with blank lines so consecutive tables never visually collide.
pub fn print_table(table: &Table) {
    if io::stdout().is_terminal() {
        println!();
    }
    println!("{table}");
    if io::stdout().is_terminal() {
        println!();
    }
}

/// Section heading plus spaced table output.
pub fn print_section(styler: &Styler, title: &str, table: &Table) {
    println!();
    println!("{}", styler.section_title(title));
    print_table(table);
}

/// Two-column stats table (label / count).
pub fn stat_table(styler: &Styler, entries: &[StatEntry]) -> Table {
    let mut table = new_table();
    table.set_header(vec![
        header_cell(styler, "Item"),
        header_cell(styler, "Count"),
    ]);
    for entry in entries {
        table.add_row(vec![
            styler.cell(&entry.label, None),
            styler.cell(
                entry.count,
                if styler.enabled() {
                    Some(comfy_table::Color::Green)
                } else {
                    None
                },
            ),
        ]);
    }
    table
}

pub fn print_stat_section(styler: &Styler, title: &str, entries: &[StatEntry]) {
    if entries.is_empty() {
        println!();
        println!("{}", styler.section_title(title));
        println!("  {}", styler.muted("(no data)"));
        return;
    }
    print_section(styler, title, &stat_table(styler, entries));
}

/// Truncate text for list/table cells without breaking UTF-8 scalars.
pub fn truncate_display(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let prefix: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{prefix}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_display_respects_char_boundary() {
        assert_eq!(truncate_display("hello", 10), "hello");
        assert_eq!(truncate_display("hello world", 8), "hello w…");
    }

    #[test]
    fn table_width_stays_within_terminal_bounds() {
        let width = table_width();
        assert!(width >= MIN_TABLE_WIDTH);
        assert!(width <= 256);
    }
}
