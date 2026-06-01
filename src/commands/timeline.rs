use anyhow::{Result, bail};

use crate::cli::TimelineArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::output::styling::Styler;
use crate::output::table_format::{header_cell, new_table, print_section};
use crate::risk;

pub fn run(args: TimelineArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let mut entries = database.session_timeline(&args.session)?;
    if entries.is_empty() {
        bail!("no commands found for session {}", args.session);
    }

    for entry in &mut entries {
        entry.risk_level =
            risk::assess_command(&entry.command).map(|value| value.level.label().to_string());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    let styler = Styler::from_config(&config);
    if args.plain {
        for entry in &entries {
            println!(
                "{} {} {:?}",
                entry.started_at, entry.command, entry.exit_code
            );
        }
        return Ok(());
    }

    let mut rows = Vec::new();
    for entry in entries {
        rows.push(vec![
            styler.cell(entry.id, None),
            styler.cell(entry.started_at, None),
            styler.cell(entry.command, None),
            styler.cell(
                entry.environment_tier.unwrap_or_else(|| "-".to_string()),
                None,
            ),
            styler.cell(entry.risk_level.unwrap_or_else(|| "-".to_string()), None),
            styler.cell(
                entry
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                None,
            ),
        ]);
    }

    let mut table = new_table();
    table.set_header(vec![
        header_cell(&styler, "ID"),
        header_cell(&styler, "Time"),
        header_cell(&styler, "Command"),
        header_cell(&styler, "Env"),
        header_cell(&styler, "Risk"),
        header_cell(&styler, "Exit"),
    ]);
    for row in rows {
        table.add_row(row);
    }
    print_section(
        &styler,
        &format!("Session timeline · {}", args.session),
        &table,
    );
    Ok(())
}
