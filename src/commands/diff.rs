use std::collections::BTreeSet;

use anyhow::{Result, bail};
use chrono::{Duration, Utc};

use crate::cli::DiffArgs;
use crate::config::AppConfig;
use crate::db::Database;

pub fn run(args: DiffArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;

    let (left_label, left, right_label, right) = if args.sessions.len() == 2 {
        (
            format!("session {}", args.sessions[0]),
            database.distinct_commands_by_column("session_id", &args.sessions[0])?,
            format!("session {}", args.sessions[1]),
            database.distinct_commands_by_column("session_id", &args.sessions[1])?,
        )
    } else if args.hosts.len() == 2 {
        (
            format!("host {}", args.hosts[0]),
            database.distinct_commands_by_column("hostname", &args.hosts[0])?,
            format!("host {}", args.hosts[1]),
            database.distinct_commands_by_column("hostname", &args.hosts[1])?,
        )
    } else if args.today && args.yesterday {
        let today_start = crate::timestamp::today_start_utc()?;
        let yesterday_start = today_start - Duration::days(1);
        (
            "today".to_string(),
            database
                .distinct_commands_between(&today_start.to_rfc3339(), &Utc::now().to_rfc3339())?,
            "yesterday".to_string(),
            database.distinct_commands_between(
                &yesterday_start.to_rfc3339(),
                &today_start.to_rfc3339(),
            )?,
        )
    } else {
        bail!("provide exactly two --session values, two --host values, or --today --yesterday");
    };

    print_diff(&left_label, &left, &right_label, &right);
    Ok(())
}

fn print_diff(left_label: &str, left: &[String], right_label: &str, right: &[String]) {
    let left_set = left.iter().cloned().collect::<BTreeSet<_>>();
    let right_set = right.iter().cloned().collect::<BTreeSet<_>>();

    println!("{left_label}: {} unique command(s)", left_set.len());
    println!("{right_label}: {} unique command(s)", right_set.len());
    println!();
    println!("Only in {left_label}:");
    for command in left_set.difference(&right_set) {
        println!("- {command}");
    }
    println!();
    println!("Only in {right_label}:");
    for command in right_set.difference(&left_set) {
        println!("- {command}");
    }
}
