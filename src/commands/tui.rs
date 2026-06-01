use std::io::{self, IsTerminal, Stderr};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::cli::TuiArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{CommandRow, SearchFilters, StatEntry, StatsPeriod, StatsSummary};
use crate::output;
use crate::output::table_format::truncate_display;
use crate::output::tui_theme::{
    self, footer_style, highlight_style, panel_block, risk_line, screen_margin, stat_line,
    title_line, ACCENT, MUTED, SUCCESS, WARNING,
};
use crate::risk::{self, RiskLevel};

pub fn run(args: TuiArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    if args.dashboard {
        return run_dashboard(&database);
    }
    let rows = database.search_commands(&SearchFilters {
        query: args.query.clone(),
        cwd: None,
        failed: args.failed,
        success: false,
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: false,
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

    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return output::print_rows(&rows, false, false);
    }

    if let Some(command) = run_tui(&database, rows, args.query.unwrap_or_default())? {
        println!("{command}");
    }
    Ok(())
}

struct DashboardData {
    summary: StatsSummary,
    risky: Vec<CommandRow>,
    environments: Vec<StatEntry>,
}

fn run_dashboard(database: &Database) -> Result<()> {
    let data = dashboard_data(database)?;
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        print_dashboard_plain(&data);
        return Ok(());
    }

    let mut terminal = TuiTerminal::enter()?;
    loop {
        terminal
            .terminal
            .draw(|frame| draw_dashboard(frame, &data))?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }
        }
    }
    Ok(())
}

fn dashboard_data(database: &Database) -> Result<DashboardData> {
    let summary = database.stats_summary(StatsPeriod::All, 5)?;
    let environments = database.distinct_environments(5)?;
    let rows = database.search_commands(&SearchFilters {
        query: None,
        cwd: None,
        failed: false,
        success: false,
        user: None,
        shell: None,
        after: None,
        before: None,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: None,
        category: None,
        pinned: false,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit: 500,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    })?;
    let risky = rows
        .into_iter()
        .filter(|row| risk::assess_command(&row.command).is_some())
        .take(8)
        .collect();

    Ok(DashboardData {
        summary,
        risky,
        environments,
    })
}

fn print_dashboard_plain(data: &DashboardData) {
    println!("mh dashboard");
    println!("Total commands: {}", data.summary.total_commands);
    println!("Failed commands: {}", data.summary.failed_commands);
    println!(
        "Error rate: {:.1}%",
        if data.summary.total_commands == 0 {
            0.0
        } else {
            (data.summary.failed_commands as f64 / data.summary.total_commands as f64) * 100.0
        }
    );
    println!("Risky recent commands: {}", data.risky.len());
}

fn run_tui(
    database: &Database,
    rows: Vec<CommandRow>,
    initial_filter: String,
) -> Result<Option<String>> {
    let mut terminal = TuiTerminal::enter()?;
    let mut app = TuiApp::new(rows, initial_filter);

    loop {
        terminal.terminal.draw(|frame| draw(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if let Some(action) = handle_key(database, &mut app, key.code, key.modifiers)? {
                return Ok(action);
            }
        }
    }
}

fn handle_key(
    database: &Database,
    app: &mut TuiApp,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<Option<Option<String>>> {
    match app.mode {
        TuiMode::Normal => handle_normal_key(database, app, code, modifiers),
        TuiMode::TagInput => {
            handle_tag_key(database, app, code, modifiers)?;
            Ok(None)
        }
        TuiMode::DeleteConfirm => {
            handle_delete_key(database, app, code)?;
            Ok(None)
        }
    }
}

fn handle_normal_key(
    database: &Database,
    app: &mut TuiApp,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<Option<Option<String>>> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => Ok(Some(None)),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.copy_selected();
            Ok(None)
        }
        KeyCode::Enter => {
            if let Some(command) = app.selected_command() {
                Ok(Some(Some(command)))
            } else {
                app.message = "No command selected".to_string();
                Ok(None)
            }
        }
        KeyCode::Up => {
            app.previous();
            Ok(None)
        }
        KeyCode::Down => {
            app.next();
            Ok(None)
        }
        KeyCode::PageUp => {
            app.page_up();
            Ok(None)
        }
        KeyCode::PageDown => {
            app.page_down();
            Ok(None)
        }
        KeyCode::Backspace => {
            app.backspace_filter();
            Ok(None)
        }
        KeyCode::Char('p') => {
            app.toggle_pin(database)?;
            Ok(None)
        }
        KeyCode::Char('d') => {
            app.begin_delete();
            Ok(None)
        }
        KeyCode::Char('t') => {
            app.begin_tag_input();
            Ok(None)
        }
        KeyCode::Char(value) if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
            app.push_filter(value);
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn handle_tag_key(
    database: &Database,
    app: &mut TuiApp,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<()> {
    match code {
        KeyCode::Esc => app.cancel_mode("Tag entry cancelled"),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.cancel_mode("Tag entry cancelled");
        }
        KeyCode::Enter => app.add_tag(database)?,
        KeyCode::Backspace => {
            app.tag_input.pop();
        }
        KeyCode::Char(value) if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
            app.tag_input.push(value);
        }
        _ => {}
    }
    Ok(())
}

fn handle_delete_key(database: &Database, app: &mut TuiApp, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            app.delete_selected(database)?
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_mode("Delete cancelled");
        }
        _ => {}
    }
    Ok(())
}

struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl TuiTerminal {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        if let Err(error) = execute!(stderr, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let backend = CrosstermBackend::new(stderr);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                Err(error.into())
            }
        }
    }
}

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct TuiApp {
    rows: Vec<CommandRow>,
    visible: Vec<usize>,
    filter: String,
    tag_input: String,
    message: String,
    mode: TuiMode,
    state: ListState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiMode {
    Normal,
    TagInput,
    DeleteConfirm,
}

impl TuiApp {
    fn new(rows: Vec<CommandRow>, filter: String) -> Self {
        let mut app = Self {
            rows,
            visible: Vec::new(),
            filter,
            tag_input: String::new(),
            message: String::new(),
            mode: TuiMode::Normal,
            state: ListState::default(),
        };
        app.refresh_visible();
        app
    }

    fn refresh_visible(&mut self) {
        self.visible = filter_indices(&self.rows, &self.filter);
        if self.visible.is_empty() {
            self.state.select(None);
        } else {
            let selected = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.visible.len().saturating_sub(1));
            self.state.select(Some(selected));
        }
    }

    fn selected_row(&self) -> Option<&CommandRow> {
        let row_index = self.selected_row_index()?;
        self.rows.get(row_index)
    }

    fn selected_row_index(&self) -> Option<usize> {
        let selected = self.state.selected()?;
        self.visible.get(selected).copied()
    }

    fn selected_command(&self) -> Option<String> {
        self.selected_row().map(|row| row.command.clone())
    }

    fn previous(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(selected.saturating_sub(1)));
    }

    fn next(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(
            (selected + 1).min(self.visible.len().saturating_sub(1)),
        ));
    }

    fn page_up(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(selected.saturating_sub(10)));
    }

    fn page_down(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(
            (selected + 10).min(self.visible.len().saturating_sub(1)),
        ));
    }

    fn push_filter(&mut self, value: char) {
        self.filter.push(value);
        self.state.select(Some(0));
        self.refresh_visible();
    }

    fn backspace_filter(&mut self) {
        self.filter.pop();
        self.state.select(Some(0));
        self.refresh_visible();
    }

    fn begin_tag_input(&mut self) {
        if self.selected_row().is_none() {
            self.message = "No command selected".to_string();
            return;
        }
        self.mode = TuiMode::TagInput;
        self.tag_input.clear();
        self.message = "Enter a tag and press Enter".to_string();
    }

    fn begin_delete(&mut self) {
        if self.selected_row().is_none() {
            self.message = "No command selected".to_string();
            return;
        }
        self.mode = TuiMode::DeleteConfirm;
        self.message = "Delete selected command? y/N".to_string();
    }

    fn cancel_mode(&mut self, message: &str) {
        self.mode = TuiMode::Normal;
        self.tag_input.clear();
        self.message = message.to_string();
    }

    fn toggle_pin(&mut self, database: &Database) -> Result<()> {
        let Some(index) = self.selected_row_index() else {
            self.message = "No command selected".to_string();
            return Ok(());
        };
        let pinned = !self.rows[index].is_pinned;
        database.set_pinned(&[self.rows[index].id], pinned)?;
        self.rows[index].is_pinned = pinned;
        self.message = if pinned {
            "Pinned selected command".to_string()
        } else {
            "Unpinned selected command".to_string()
        };
        Ok(())
    }

    fn add_tag(&mut self, database: &Database) -> Result<()> {
        let tag = self.tag_input.trim().to_string();
        if tag.is_empty() {
            self.cancel_mode("Tag must not be empty");
            return Ok(());
        }
        let Some(index) = self.selected_row_index() else {
            self.cancel_mode("No command selected");
            return Ok(());
        };

        database.add_tags(&[self.rows[index].id], std::slice::from_ref(&tag))?;
        if !self.rows[index]
            .tags
            .iter()
            .any(|existing| existing == &tag)
        {
            self.rows[index].tags.push(tag.clone());
        }
        self.mode = TuiMode::Normal;
        self.tag_input.clear();
        self.message = format!("Added tag {tag}");
        self.refresh_visible();
        Ok(())
    }

    fn delete_selected(&mut self, database: &Database) -> Result<()> {
        let Some(index) = self.selected_row_index() else {
            self.cancel_mode("No command selected");
            return Ok(());
        };
        let id = self.rows[index].id;
        database.delete_command_ids(&[id])?;
        self.rows.remove(index);
        self.mode = TuiMode::Normal;
        self.message = format!("Deleted command {id}");
        self.refresh_visible();
        Ok(())
    }

    fn copy_selected(&mut self) {
        let Some(command) = self.selected_command() else {
            self.message = "No command selected".to_string();
            return;
        };

        match copy_to_clipboard(&command) {
            Ok(()) => self.message = "Copied selected command".to_string(),
            Err(error) => self.message = format!("Clipboard unavailable: {error}"),
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &mut TuiApp) {
    let area = frame.area().inner(screen_margin());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(area);

    let search_text = match app.mode {
        TuiMode::Normal => format!("  {}", app.filter),
        TuiMode::TagInput => format!("  tag: {}", app.tag_input),
        TuiMode::DeleteConfirm => "  Confirm delete — y/Enter yes · n/Esc cancel".to_string(),
    };
    let mode_hint = match app.mode {
        TuiMode::Normal => "filter",
        TuiMode::TagInput => "tag",
        TuiMode::DeleteConfirm => "confirm",
    };
    let search = Paragraph::new(search_text).block(panel_block(title_line(&format!(
        "mh · {mode_hint}"
    ))));
    frame.render_widget(search, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .margin(1)
        .split(chunks[1]);

    let list_width = body[0].width.saturating_sub(14) as usize;
    let items = app
        .visible
        .iter()
        .map(|index| list_item(&app.rows[*index], list_width))
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(panel_block(title_line(&format!(
            "History ({})",
            app.visible.len()
        ))))
        .highlight_style(highlight_style())
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, body[0], &mut app.state);

    let detail = Paragraph::new(detail_text(app.selected_row()))
        .block(panel_block(title_line("Details")))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, body[1]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "↑↓ move · Enter run · Ctrl-C copy · p pin · t tag · d delete · q quit",
            footer_style(),
        ),
        Span::raw("\n"),
        Span::styled(&app.message, Style::default().fg(ACCENT)),
    ]))
    .block(panel_block(title_line("Controls")));
    frame.render_widget(footer, chunks[2]);
}

fn draw_dashboard(frame: &mut ratatui::Frame<'_>, data: &DashboardData) {
    let area = frame.area().inner(screen_margin());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let error_rate = if data.summary.total_commands == 0 {
        0.0
    } else {
        (data.summary.failed_commands as f64 / data.summary.total_commands as f64) * 100.0
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Total ", Style::default().fg(MUTED)),
            Span::styled(
                data.summary.total_commands.to_string(),
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("OK ", Style::default().fg(MUTED)),
            Span::styled(
                data.summary.successful_commands.to_string(),
                Style::default().fg(SUCCESS),
            ),
            Span::raw("   "),
            Span::styled("Failed ", Style::default().fg(MUTED)),
            Span::styled(
                data.summary.failed_commands.to_string(),
                Style::default().fg(if data.summary.failed_commands > 0 {
                    WARNING
                } else {
                    MUTED
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Error rate ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{error_rate:.1}%"),
                Style::default().fg(if error_rate >= 20.0 {
                    tui_theme::DANGER
                } else if error_rate > 0.0 {
                    WARNING
                } else {
                    SUCCESS
                }),
            ),
            Span::raw("   "),
            Span::styled("Peak hour ", Style::default().fg(MUTED)),
            Span::raw(data.summary.peak_hour.as_deref().unwrap_or("-")),
        ]),
    ])
    .block(panel_block(title_line("mh dashboard")));
    frame.render_widget(header, chunks[0]);

    if area.width >= 100 {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .margin(1)
            .split(chunks[1]);

        let label_width = body[0].width.saturating_sub(8);
        frame.render_widget(
            dashboard_stat_list("Top commands", &data.summary.top_commands, label_width),
            body[0],
        );
        frame.render_widget(
            dashboard_risk_list(&data.risky, body[1].width.saturating_sub(16) as usize),
            body[1],
        );
        frame.render_widget(
            dashboard_stat_list("Environments", &data.environments, label_width),
            body[2],
        );
    } else {
        let body = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .margin(1)
            .split(chunks[1]);

        let label_width = area.width.saturating_sub(12);
        frame.render_widget(
            dashboard_stat_list("Top commands", &data.summary.top_commands, label_width),
            body[0],
        );
        frame.render_widget(
            dashboard_risk_list(&data.risky, area.width.saturating_sub(20) as usize),
            body[1],
        );
        frame.render_widget(
            dashboard_stat_list("Environments", &data.environments, label_width),
            body[2],
        );
    }

    let footer = Paragraph::new(Span::styled("q or Esc to exit", footer_style()))
        .block(panel_block(title_line("Controls")));
    frame.render_widget(footer, chunks[2]);
}

fn dashboard_stat_list(title: &'static str, entries: &[StatEntry], label_width: u16) -> List<'static> {
    let items = if entries.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No data",
            Style::default().fg(MUTED),
        )]))]
    } else {
        entries
            .iter()
            .map(|entry| ListItem::new(stat_line(entry.count, &entry.label, label_width)))
            .collect()
    };
    List::new(items).block(panel_block(title_line(title)))
}

fn dashboard_risk_list(rows: &[CommandRow], command_width: usize) -> List<'static> {
    let items = if rows.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No risky recent commands",
            Style::default().fg(MUTED),
        )]))]
    } else {
        rows.iter()
            .map(|row| {
                let level = risk::assess_command(&row.command)
                    .map(|assessment| risk_label(assessment.level))
                    .unwrap_or("-");
                ListItem::new(risk_line(level, row.id, &row.command, command_width))
            })
            .collect()
    };
    List::new(items).block(panel_block(title_line("Risk")))
}

fn risk_label(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Critical => "critical",
        RiskLevel::High => "high",
        RiskLevel::Medium => "medium",
    }
}

fn list_item(row: &CommandRow, command_width: usize) -> ListItem<'static> {
    let exit = row
        .exit_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let exit_style = match row.exit_code {
        Some(0) => Style::default().fg(SUCCESS),
        Some(_) => Style::default().fg(tui_theme::DANGER),
        None => Style::default().fg(MUTED),
    };
    let pin = if row.is_pinned {
        Span::styled("◆ ", Style::default().fg(ACCENT))
    } else {
        Span::raw("  ")
    };
    ListItem::new(Line::from(vec![
        pin,
        Span::styled(format!("{:<5} ", row.id), Style::default().fg(MUTED)),
        Span::styled(format!("{exit:<4} "), exit_style),
        Span::raw(truncate_display(&row.command, command_width)),
    ]))
}

fn detail_text(row: Option<&CommandRow>) -> String {
    let Some(row) = row else {
        return "Select a command from the list.".to_string();
    };

    format!(
        "ID        {}\nPinned    {}\nMasked    {}\nTime      {}\nExit      {}\nDuration  {}\nShell     {}\nCWD       {}\nCategory  {}\nTags      {}\n\n{}",
        row.id,
        row.is_pinned,
        row.is_masked,
        row.started_at,
        row.exit_code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        row.duration_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "-".to_string()),
        row.shell.as_deref().unwrap_or("-"),
        row.cwd.as_deref().unwrap_or("-"),
        row.category.as_deref().unwrap_or("-"),
        if row.tags.is_empty() {
            "-".to_string()
        } else {
            row.tags.join(", ")
        },
        row.command
    )
}

fn filter_indices(rows: &[CommandRow], filter: &str) -> Vec<usize> {
    let filter = filter.trim();
    if filter.is_empty() {
        return (0..rows.len()).collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut scored = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            matcher
                .fuzzy_match(&search_text(row), filter)
                .map(|score| (score, index))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    scored.into_iter().map(|(_, index)| index).collect()
}

fn search_text(row: &CommandRow) -> String {
    let mut text = row.command.clone();
    if let Some(cwd) = &row.cwd {
        text.push(' ');
        text.push_str(cwd);
    }
    if let Some(category) = &row.category {
        text.push(' ');
        text.push_str(category);
    }
    for tag in &row.tags {
        text.push(' ');
        text.push_str(tag);
    }
    text
}

fn copy_to_clipboard(command: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open clipboard")?;
    clipboard
        .set_text(command.to_string())
        .context("failed to write clipboard")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_tui_rows_fuzzily() {
        let rows = vec![row("docker ps"), row("git status")];
        assert_eq!(filter_indices(&rows, "dps"), vec![0]);
    }

    #[test]
    fn app_tracks_tag_input_mode() {
        let mut app = TuiApp::new(vec![row("docker ps")], String::new());
        app.begin_tag_input();
        app.tag_input.push_str("ops");

        assert_eq!(app.mode, TuiMode::TagInput);
        assert_eq!(app.tag_input, "ops");
    }

    #[test]
    fn navigation_moves_selection() {
        let mut app = TuiApp::new(vec![row("alpha"), row("beta"), row("gamma")], String::new());
        assert_eq!(app.state.selected(), Some(0));
        app.next();
        assert_eq!(app.state.selected(), Some(1));
        app.page_down();
        assert_eq!(app.state.selected(), Some(2));
        app.previous();
        assert_eq!(app.state.selected(), Some(1));
    }

    #[test]
    fn filter_backspace_updates_visible_rows() {
        let mut app = TuiApp::new(vec![row("docker ps"), row("git status")], String::new());
        app.push_filter('d');
        assert_eq!(app.visible.len(), 1);
        app.backspace_filter();
        assert_eq!(app.visible.len(), 2);
    }

    #[test]
    fn delete_confirm_mode_can_be_cancelled() {
        let mut app = TuiApp::new(vec![row("docker ps")], String::new());
        app.begin_delete();
        assert_eq!(app.mode, TuiMode::DeleteConfirm);
        app.cancel_mode("Delete cancelled");
        assert_eq!(app.mode, TuiMode::Normal);
        assert_eq!(app.message, "Delete cancelled");
    }

    #[test]
    fn detail_text_marks_masked_commands() {
        let mut masked = row("mysql -p****");
        masked.is_masked = true;
        let text = detail_text(Some(&masked));
        assert!(text.contains("Masked    true"));
        assert!(text.contains("mysql"));
    }

    #[test]
    fn search_text_includes_tags_for_fuzzy_filter() {
        let mut tagged = row("deploy");
        tagged.tags = vec!["production".to_string()];
        assert!(search_text(&tagged).contains("production"));
        assert_eq!(filter_indices(&[tagged], "prod"), vec![0]);
    }

    fn row(command: &str) -> CommandRow {
        CommandRow {
            id: 1,
            command: command.to_string(),
            cwd: None,
            shell: None,
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
            category: None,
            tags: Vec::new(),
            is_pinned: false,
            is_masked: false,
        }
    }
}
