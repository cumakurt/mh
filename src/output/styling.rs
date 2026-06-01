use std::env;
use std::io::{self, IsTerminal};

use comfy_table::{Cell, Color};
use crossterm::style::Stylize;

use crate::config::AppConfig;
use crate::risk::RiskLevel;

#[derive(Debug, Clone, Copy)]
pub struct Styler {
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Ok,
    Warn,
    Info,
    Error,
}

impl Styler {
    pub fn from_config(config: &AppConfig) -> Self {
        Self::from_display_config(config.display.color)
    }

    pub fn from_display_config(color: bool) -> Self {
        Self {
            enabled: color && env::var("NO_COLOR").is_err() && io::stdout().is_terminal(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn status(&self, level: StatusLevel, message: impl AsRef<str>) -> String {
        let message = message.as_ref();
        let label = match level {
            StatusLevel::Ok => "[OK   ]",
            StatusLevel::Warn => "[WARN ]",
            StatusLevel::Info => "[INFO ]",
            StatusLevel::Error => "[ERROR]",
        };

        if !self.enabled {
            return format!("{label} {message}");
        }

        let styled_label = match level {
            StatusLevel::Ok => label.green().bold(),
            StatusLevel::Warn => label.yellow().bold(),
            StatusLevel::Info => label.cyan(),
            StatusLevel::Error => label.red().bold(),
        };
        format!("{styled_label} {message}")
    }

    pub fn label_value(&self, label: &str, value: impl AsRef<str>) -> String {
        let value = value.as_ref();
        if !self.enabled {
            return format!("{label:<20}: {value}");
        }
        format!("{}: {}", label.cyan(), value.white())
    }

    pub fn section_title(&self, title: impl AsRef<str>) -> String {
        let title = title.as_ref();
        if !self.enabled {
            return format!("== {title} ==");
        }
        format!("{}", title.cyan().bold())
    }

    pub fn separator(&self) -> String {
        let line = "─".repeat(56);
        if self.enabled {
            line.dark_grey().to_string()
        } else {
            line
        }
    }

    pub fn accent(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if self.enabled {
            text.cyan().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn success(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if self.enabled {
            text.green().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn warning(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if self.enabled {
            text.yellow().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn muted(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if self.enabled {
            text.dark_grey().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn cell(&self, text: impl std::fmt::Display, color: Option<Color>) -> Cell {
        let text = text.to_string();
        match (self.enabled, color) {
            (true, Some(color)) => Cell::new(text).fg(color),
            _ => Cell::new(text),
        }
    }

    pub fn exit_code_cell(&self, exit_code: Option<i32>) -> Cell {
        match exit_code {
            Some(0) => self.cell("0", Some(Color::Green)),
            Some(code) if code != 0 => self.cell(code.to_string(), Some(Color::Red)),
            None => self.cell("-", Some(Color::DarkGrey)),
            _ => self.cell("-", None),
        }
    }

    pub fn risk_level_cell(&self, level: RiskLevel) -> Cell {
        let color = match level {
            RiskLevel::Critical => Color::Red,
            RiskLevel::High => Color::Yellow,
            RiskLevel::Medium => Color::DarkYellow,
        };
        self.cell(level.label(), Some(color))
    }

    pub fn risk_level_text(&self, level: RiskLevel) -> String {
        let label = level.label();
        if !self.enabled {
            return label.to_string();
        }
        match level {
            RiskLevel::Critical => label.red().bold().to_string(),
            RiskLevel::High => label.yellow().to_string(),
            RiskLevel::Medium => label.dark_yellow().to_string(),
        }
    }

    pub fn audit_event_cell(&self, event_type: &str) -> Cell {
        let color = match event_type {
            "skipped" => Color::Yellow,
            "masked" => Color::Magenta,
            "risky" => Color::Red,
            _ => Color::Cyan,
        };
        self.cell(event_type, Some(color))
    }

    pub fn category_cell(&self, category: Option<&str>) -> Cell {
        match category.filter(|value| !value.is_empty()) {
            Some(value) => self.cell(value, Some(Color::Blue)),
            None => self.cell("-", Some(Color::DarkGrey)),
        }
    }

    pub fn tag_cell(&self, tags: &[String]) -> Cell {
        if tags.is_empty() {
            return self.cell("-", Some(Color::DarkGrey));
        }
        self.cell(tags.join(","), Some(Color::Magenta))
    }

    pub fn command_cell(&self, row: &crate::models::CommandRow) -> Cell {
        self.cell(format_command(row), command_color(row))
    }

    pub fn command_cell_truncated(
        &self,
        row: &crate::models::CommandRow,
        max_chars: usize,
    ) -> Cell {
        self.cell(
            truncate_text(&format_command(row), max_chars),
            command_color(row),
        )
    }

    pub fn error_rate_text(&self, total: i64, failed: i64) -> String {
        if total == 0 {
            return self.muted("-");
        }
        let rate = (failed as f64 / total as f64) * 100.0;
        let text = format!("{rate:.1}%");
        if !self.enabled {
            return text;
        }
        if rate >= 20.0 {
            text.red().to_string()
        } else if rate > 0.0 {
            text.yellow().to_string()
        } else {
            text.green().to_string()
        }
    }

    pub fn heatmap_bar(&self, _count: i64, bar_len: usize) -> String {
        if bar_len == 0 {
            return String::new();
        }
        let bar = "#".repeat(bar_len);
        if self.enabled {
            bar.green().to_string()
        } else {
            bar
        }
    }
}

fn command_color(row: &crate::models::CommandRow) -> Option<Color> {
    if crate::risk::assess_command(&row.command).is_some() {
        return Some(Color::Red);
    }
    if row.is_masked {
        return Some(Color::Magenta);
    }
    if row.is_pinned {
        return Some(Color::Cyan);
    }
    if row.exit_code.is_some_and(|code| code != 0) {
        return Some(Color::Yellow);
    }
    None
}

fn truncate_text(text: &str, max_chars: usize) -> String {
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

fn format_command(row: &crate::models::CommandRow) -> String {
    let mut labels = Vec::new();
    if row.is_pinned {
        labels.push("pinned".to_string());
    }
    if row.is_masked {
        labels.push("masked".to_string());
    }
    if let Some(assessment) = crate::risk::assess_command(&row.command) {
        labels.push(format!("risk:{}", assessment.level.label()));
    }

    if labels.is_empty() {
        row.command.clone()
    } else {
        format!("{} [{}]", row.command, labels.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disables_color_when_no_color_is_set() {
        // SAFETY: test restores NO_COLOR after assertion.
        unsafe { env::set_var("NO_COLOR", "1") };
        let styler = Styler::from_display_config(true);
        assert!(!styler.enabled());
        // SAFETY: restores environment for other tests.
        unsafe { env::remove_var("NO_COLOR") };
    }

    #[test]
    fn status_plain_when_color_disabled() {
        let styler = Styler::from_display_config(false);
        assert_eq!(styler.status(StatusLevel::Ok, "ready"), "[OK   ] ready");
        assert!(
            styler
                .status(StatusLevel::Warn, "check")
                .contains("[WARN ]")
        );
    }
}
