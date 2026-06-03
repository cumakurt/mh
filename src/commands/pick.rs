use std::io::{self, IsTerminal, Stderr, Write};

use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::cli::PickArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{CommandRow, SearchFilters};
use crate::ranking::{RankContext, rank_indices, sort_by_context};

pub fn run(args: PickArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let mut rows = database.search_commands(&SearchFilters {
        query: args.query.clone(),
        cwd: args.cwd.clone(),
        failed: args.failed,
        success: false,
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: args.fuzzy,
        fts: false,
        tag: args.tag,
        category: args.category,
        pinned: args.pinned,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit: args.limit,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    })?;

    if rows.is_empty() {
        return Ok(());
    }

    let context_ranking = config.display.context_ranking && !args.recent;
    if context_ranking {
        let ctx = RankContext::from_env();
        if args.query.as_deref().unwrap_or("").trim().is_empty() {
            sort_by_context(&mut rows, &ctx);
        }
    }

    let selection = if io::stdin().is_terminal() && io::stderr().is_terminal() {
        run_interactive_picker(
            &rows,
            args.query.unwrap_or_default(),
            context_ranking,
            args.recent,
            args.recent,
        )?
    } else {
        rows.first().map(|row| row.command.clone())
    };

    if let Some(command) = selection {
        println!("{command}");
    }

    Ok(())
}

fn run_interactive_picker(
    rows: &[CommandRow],
    initial_filter: String,
    context_ranking: bool,
    preserve_entry_order: bool,
    command_only: bool,
) -> Result<Option<String>> {
    let mut terminal = PickerTerminal::enter()?;
    let mut state = PickerState::new(initial_filter);
    let rank_ctx = if context_ranking {
        Some(RankContext::from_env())
    } else {
        None
    };

    loop {
        let visible_indices =
            filter_rows(rows, &state.filter, rank_ctx.as_ref(), preserve_entry_order);
        state.clamp(visible_indices.len());
        draw(
            &mut terminal.stderr,
            rows,
            &visible_indices,
            &mut state,
            command_only,
        )?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match handle_key(key, rows, &visible_indices, &mut state) {
                PickerAction::Continue => {}
                PickerAction::Cancel => return Ok(None),
                PickerAction::Select(command) => return Ok(Some(command)),
            }
        }
    }
}

#[derive(Debug)]
struct PickerState {
    selected: usize,
    offset: usize,
    filter: String,
}

impl PickerState {
    fn new(filter: String) -> Self {
        Self {
            selected: 0,
            offset: 0,
            filter,
        }
    }

    fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.offset = 0;
            return;
        }

        if self.selected >= len {
            self.selected = len - 1;
        }
        if self.offset >= len {
            self.offset = len - 1;
        }
    }
}

enum PickerAction {
    Continue,
    Cancel,
    Select(String),
}

struct PickerTerminal {
    stderr: Stderr,
}

impl PickerTerminal {
    fn enter() -> Result<Self> {
        let mut stderr = io::stderr();
        terminal::enable_raw_mode()?;
        execute!(stderr, EnterAlternateScreen, Hide)?;
        Ok(Self { stderr })
    }
}

impl Drop for PickerTerminal {
    fn drop(&mut self) {
        let _ = execute!(self.stderr, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn handle_key(
    key: KeyEvent,
    rows: &[CommandRow],
    visible_indices: &[usize],
    state: &mut PickerState,
) -> PickerAction {
    match key.code {
        KeyCode::Esc => PickerAction::Cancel,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => PickerAction::Cancel,
        KeyCode::Enter => {
            let Some(index) = visible_indices.get(state.selected) else {
                return PickerAction::Continue;
            };
            PickerAction::Select(rows[*index].command.clone())
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
            PickerAction::Continue
        }
        KeyCode::Down => {
            if state.selected + 1 < visible_indices.len() {
                state.selected += 1;
            }
            PickerAction::Continue
        }
        KeyCode::PageUp => {
            state.selected = state.selected.saturating_sub(10);
            PickerAction::Continue
        }
        KeyCode::PageDown => {
            state.selected = (state.selected + 10).min(visible_indices.len().saturating_sub(1));
            PickerAction::Continue
        }
        KeyCode::Home => {
            state.selected = 0;
            PickerAction::Continue
        }
        KeyCode::End => {
            state.selected = visible_indices.len().saturating_sub(1);
            PickerAction::Continue
        }
        KeyCode::Backspace => {
            state.filter.pop();
            state.selected = 0;
            state.offset = 0;
            PickerAction::Continue
        }
        KeyCode::Char(value)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            state.filter.push(value);
            state.selected = 0;
            state.offset = 0;
            PickerAction::Continue
        }
        _ => PickerAction::Continue,
    }
}

fn draw(
    stderr: &mut Stderr,
    rows: &[CommandRow],
    visible_indices: &[usize],
    state: &mut PickerState,
    command_only: bool,
) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width.max(20);
    let list_height = usize::from(height.saturating_sub(5)).max(1);

    if state.selected < state.offset {
        state.offset = state.selected;
    } else if state.selected >= state.offset + list_height {
        state.offset = state.selected + 1 - list_height;
    }

    queue!(
        stderr,
        Clear(ClearType::All),
        MoveTo(0, 0),
        Print("mh history picker"),
        MoveTo(0, 1),
        Print("Enter: select  Esc/Ctrl-C: cancel  Type: filter"),
        MoveTo(0, 2),
        Print(format!("Filter: {}", state.filter)),
        MoveTo(0, 3),
        Print(header_line(width, command_only))
    )?;

    if visible_indices.is_empty() {
        queue!(
            stderr,
            MoveTo(0, 4),
            Print("No commands match the current filter")
        )?;
        stderr.flush()?;
        return Ok(());
    }

    for (screen_row, index) in visible_indices
        .iter()
        .skip(state.offset)
        .take(list_height)
        .enumerate()
    {
        let y = 4 + screen_row as u16;
        queue!(stderr, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
        if state.offset + screen_row == state.selected {
            queue!(stderr, SetAttribute(Attribute::Reverse))?;
        }
        queue!(
            stderr,
            Print(row_line(&rows[*index], width, command_only)),
            ResetColor
        )?;
        if state.offset + screen_row == state.selected {
            queue!(stderr, SetAttribute(Attribute::NoReverse))?;
        }
    }

    stderr.flush()?;
    Ok(())
}

fn header_line(width: u16, command_only: bool) -> String {
    if command_only {
        return truncate_to_width("Command", width);
    }

    truncate_to_width(
        "ID     Time                 Exit  CWD                  Command",
        width,
    )
}

fn row_line(row: &CommandRow, width: u16, command_only: bool) -> String {
    if command_only {
        return truncate_to_width(&row.command, width);
    }

    if width < 60 {
        return truncate_to_width(&row.command, width);
    }

    let cwd_width = usize::from(width).saturating_sub(50).clamp(12, 28);
    let command_width = usize::from(width).saturating_sub(31 + cwd_width);
    let exit_code = row
        .exit_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let time = format_time(&row.started_at);
    let cwd = row.cwd.as_deref().unwrap_or("-");

    let line = format!(
        "{:<6} {:<19} {:<5} {:<cwd_width$} {}",
        row.id,
        truncate_to_width(&time, 19),
        truncate_to_width(&exit_code, 5),
        truncate_to_width(cwd, cwd_width as u16),
        truncate_to_width(&row.command, command_width as u16),
    );
    truncate_to_width(&line, width)
}

fn format_time(value: &str) -> String {
    value
        .replace('T', " ")
        .replace("+00:00", "")
        .replace('Z', "")
}

fn filter_rows(
    rows: &[CommandRow],
    filter: &str,
    rank_ctx: Option<&RankContext>,
    preserve_entry_order: bool,
) -> Vec<usize> {
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        if let Some(ctx) = rank_ctx {
            let mut indices: Vec<usize> = (0..rows.len()).collect();
            indices.sort_by(|left, right| {
                crate::ranking::context_score(&rows[*right], ctx)
                    .cmp(&crate::ranking::context_score(&rows[*left], ctx))
                    .then_with(|| rows[*right].started_at.cmp(&rows[*left].started_at))
            });
            return indices;
        }
        return (0..rows.len()).collect();
    }

    let matcher = SkimMatcherV2::default();
    let fuzzy_scored = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let haystack = row_search_text(row).to_lowercase();
            matcher
                .fuzzy_match(&haystack, &filter)
                .map(|score| (score, index))
        })
        .collect::<Vec<_>>();

    if preserve_entry_order {
        return fuzzy_scored.into_iter().map(|(_, index)| index).collect();
    }

    if let Some(ctx) = rank_ctx {
        return rank_indices(rows, ctx, &fuzzy_scored);
    }

    let mut scored = fuzzy_scored;
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    scored.into_iter().map(|(_, index)| index).collect()
}

fn row_search_text(row: &CommandRow) -> String {
    let mut haystack = row.command.to_lowercase();
    if let Some(cwd) = &row.cwd {
        haystack.push(' ');
        haystack.push_str(&cwd.to_lowercase());
    }
    if let Some(category) = &row.category {
        haystack.push(' ');
        haystack.push_str(&category.to_lowercase());
    }
    for tag in &row.tags {
        haystack.push(' ');
        haystack.push_str(&tag.to_lowercase());
    }
    haystack
}

fn truncate_to_width(value: &str, width: u16) -> String {
    let width = usize::from(width);
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }

    let mut truncated: String = value.chars().take(width - 3).collect();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_command_cwd_category_and_tags() {
        let rows = vec![
            command_row("docker ps", Some("/srv/api"), Some("docker"), vec!["ops"]),
            command_row("git status", Some("/home/app"), Some("git"), vec!["code"]),
        ];

        assert_eq!(filter_rows(&rows, "docker", None, false), vec![0]);
        assert_eq!(filter_rows(&rows, "srv", None, false), vec![0]);
        assert_eq!(filter_rows(&rows, "code", None, false), vec![1]);
        assert_eq!(
            filter_rows(&rows, "missing", None, false),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn recent_mode_filter_preserves_entry_order() {
        let rows = vec![
            command_row(
                "docker compose ps",
                Some("/srv/api"),
                Some("docker"),
                vec![],
            ),
            command_row("docker ps", Some("/tmp"), Some("docker"), vec![]),
        ];

        assert_eq!(filter_rows(&rows, "docker ps", None, true), vec![0, 1]);
    }

    #[test]
    fn recent_mode_renders_only_command_column() {
        let row = command_row("git status --short", Some("/srv/api"), Some("git"), vec![]);

        assert_eq!(header_line(80, true), "Command");
        assert_eq!(row_line(&row, 80, true), "git status --short");
        assert!(header_line(80, false).contains("Time"));
        assert!(row_line(&row, 80, false).contains("git status --short"));
    }

    #[test]
    fn truncates_with_ascii_suffix() {
        assert_eq!(truncate_to_width("abcdef", 4), "a...");
        assert_eq!(truncate_to_width("abcdef", 2), "ab");
    }

    fn command_row(
        command: &str,
        cwd: Option<&str>,
        category: Option<&str>,
        tags: Vec<&str>,
    ) -> CommandRow {
        CommandRow {
            id: 1,
            command: command.to_string(),
            cwd: cwd.map(ToOwned::to_owned),
            shell: Some("zsh".to_string()),
            username: None,
            hostname: None,
            exit_code: Some(0),
            duration_ms: None,
            started_at: "2026-05-31T12:00:00Z".to_string(),
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment_tier: None,
            category: category.map(ToOwned::to_owned),
            tags: tags.into_iter().map(ToOwned::to_owned).collect(),
            is_pinned: false,
            is_masked: false,
        }
    }
}
