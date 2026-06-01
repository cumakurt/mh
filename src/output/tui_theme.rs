//! Ratatui visual theme for mh (rounded panels, spacing, readable contrast).

use ratatui::layout::Margin;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

pub const ACCENT: Color = Color::Cyan;
pub const MUTED: Color = Color::DarkGray;
pub const SUCCESS: Color = Color::Green;
pub const WARNING: Color = Color::Yellow;
pub const DANGER: Color = Color::Red;
pub const HIGHLIGHT_BG: Color = Color::Rgb(45, 52, 64);

/// Outer margin so panels do not touch terminal edges.
pub fn screen_margin() -> Margin {
    Margin::new(1, 2)
}

pub fn panel_block(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED))
        .style(Style::default().fg(Color::White))
}

pub fn title_line(text: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        text.to_string(),
        Style::default()
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD),
    )])
}

pub fn highlight_style() -> Style {
    Style::default()
        .fg(ACCENT)
        .bg(HIGHLIGHT_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn footer_style() -> Style {
    Style::default().fg(MUTED)
}

pub fn stat_line(count: i64, label: &str, label_width: u16) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:>5}", count),
            Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::raw(crate::output::table_format::truncate_display(
            label,
            label_width as usize,
        )),
    ])
}

pub fn risk_line(level: &str, id: i64, command: &str, command_width: usize) -> Line<'static> {
    let level_style = match level {
        "critical" => Style::default().fg(DANGER).add_modifier(Modifier::BOLD),
        "high" => Style::default().fg(WARNING),
        _ => Style::default().fg(MUTED),
    };
    Line::from(vec![
        Span::styled(format!("{:<8}", level), level_style),
        Span::raw(format!(" {:<5} ", id)),
        Span::raw(crate::output::table_format::truncate_display(command, command_width)),
    ])
}
