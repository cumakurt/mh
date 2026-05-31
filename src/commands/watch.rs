use anyhow::Result;

use crate::cli::{AuditFormat, WatchArgs};
use crate::config::AppConfig;
use crate::db::Database;

pub fn run(args: WatchArgs) -> Result<()> {
    let config = AppConfig::load()?;
    if !config.siem.enabled {
        eprintln!("SIEM streaming is disabled; enable siem.enabled in config");
    }

    let database = Database::open(&config)?;
    let rows = database.audit_rows(false, args.limit)?;

    match args.format {
        AuditFormat::Json => {
            for row in rows {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        AuditFormat::Table => {
            for row in rows {
                println!(
                    "[{}] {} {} {}",
                    row.created_at,
                    row.event_type,
                    row.reason.as_deref().unwrap_or("-"),
                    row.raw_command.as_deref().unwrap_or("-")
                );
            }
        }
    }

    if args.follow {
        eprintln!("Follow mode prints the latest batch; use shell tooling for live tailing.");
    }

    Ok(())
}
