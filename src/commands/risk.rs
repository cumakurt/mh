use anyhow::{Result, bail};
use chrono::{Local, TimeZone};
use serde::Serialize;

use crate::cli::{RiskArgs, RiskCommand, RiskScanArgs};
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::SearchFilters;
use crate::output::styling::Styler;
use crate::output::table_format::{header_cell, new_table, print_section};
use crate::risk::{self, RiskAssessment, RiskLevel, is_at_least};

#[derive(Debug, Serialize)]
struct RiskScanRow {
    id: i64,
    command: String,
    level: RiskLevel,
    rule_id: String,
    description: String,
    started_at: String,
}

pub fn run(args: RiskArgs) -> Result<()> {
    match args.command {
        RiskCommand::List => list_rules(),
        RiskCommand::Check { command, json } => check_command(&command, json),
        RiskCommand::Scan(scan_args) => scan_history(scan_args),
    }
}

fn list_rules() -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let mut table = new_table();
    table.set_header(vec![
        header_cell(&styler, "ID"),
        header_cell(&styler, "Level"),
        header_cell(&styler, "Description"),
    ]);

    for rule in risk::list_rules() {
        table.add_row(vec![
            styler.cell(rule.id, None),
            styler.risk_level_cell(rule.level),
            styler.cell(rule.description, None),
        ]);
    }

    print_section(&styler, "Risk rules", &table);
    Ok(())
}

fn check_command(command: &str, json: bool) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);

    match risk::assess_command(command) {
        Some(assessment) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&assessment)?);
            } else {
                print_assessment(&styler, &assessment);
            }
        }
        None => {
            if json {
                println!("null");
            } else {
                println!("{}", styler.success("No risk detected"));
            }
        }
    }
    Ok(())
}

fn scan_history(args: RiskScanArgs) -> Result<()> {
    if args.critical && args.high {
        bail!("--critical and --high cannot be used together");
    }

    let minimum = minimum_level(&args);
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let limit = args.limit.unwrap_or(config.display.default_limit);
    let database = Database::open(&config)?;

    let after = if args.today {
        Some(today_start_rfc3339())
    } else {
        None
    };

    let filters = SearchFilters {
        query: None,
        cwd: None,
        failed: false,
        success: false,
        user: None,
        shell: None,
        after,
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
        limit,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    };

    let rows = database.search_commands(&filters)?;
    let matches: Vec<RiskScanRow> = rows
        .into_iter()
        .filter_map(|row| {
            risk::assess_command(&row.command).and_then(|assessment| {
                if is_at_least(assessment.level, minimum) {
                    Some(RiskScanRow {
                        id: row.id,
                        command: row.command,
                        level: assessment.level,
                        rule_id: assessment.rule_id,
                        description: assessment.description,
                        started_at: row.started_at,
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&matches)?);
        return Ok(());
    }

    if matches.is_empty() {
        println!("{}", styler.success("No risky commands found"));
        return Ok(());
    }

    let mut table = new_table();
    table.set_header(vec![
        header_cell(&styler, "ID"),
        header_cell(&styler, "Level"),
        header_cell(&styler, "Time"),
        header_cell(&styler, "Rule"),
        header_cell(&styler, "Command"),
    ]);

    for row in matches {
        table.add_row(vec![
            styler.cell(row.id, None),
            styler.risk_level_cell(row.level),
            styler.cell(row.started_at, None),
            styler.cell(row.rule_id, None),
            styler.cell(
                row.command,
                if styler.enabled() {
                    Some(comfy_table::Color::Red)
                } else {
                    None
                },
            ),
        ]);
    }

    print_section(&styler, "Risk scan results", &table);
    Ok(())
}

fn minimum_level(args: &RiskScanArgs) -> RiskLevel {
    if args.critical {
        RiskLevel::Critical
    } else if args.high {
        RiskLevel::High
    } else {
        RiskLevel::Medium
    }
}

fn print_assessment(styler: &Styler, assessment: &RiskAssessment) {
    println!(
        "{}",
        styler.label_value("Level", styler.risk_level_text(assessment.level))
    );
    println!("{}", styler.label_value("Rule", &assessment.rule_id));
    println!(
        "{}",
        styler.label_value("Description", &assessment.description)
    );
}

fn today_start_rfc3339() -> String {
    let today = Local::now().date_naive();
    today
        .and_hms_opt(0, 0, 0)
        .and_then(|datetime| Local.from_local_datetime(&datetime).single())
        .map(|datetime| datetime.to_rfc3339())
        .unwrap_or_else(|| Local::now().to_rfc3339())
}
