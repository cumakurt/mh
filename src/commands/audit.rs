use anyhow::{Context, Result, bail};

use crate::cli::AuditArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::output::styling::Styler;

pub fn run(args: AuditArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;

    if args.rebuild_chain {
        if !args.yes {
            bail!("audit chain rebuild requires --yes");
        }
        let updated = database
            .rebuild_audit_chain()
            .context("failed to rebuild audit hash chain")?;
        database.verify_audit_chain()?;
        let styler = Styler::from_config(&config);
        println!(
            "{}",
            styler.success(format!(
                "Rebuilt audit hash chain for {updated} entr{}",
                if updated == 1 { "y" } else { "ies" }
            ))
        );
        return Ok(());
    }

    if args.verify_chain {
        let rows = database.audit_rows_chronological(usize::MAX)?;
        let unsealed = crate::audit_chain::count_unsealed_entries(&rows);
        database.verify_audit_chain()?;
        let styler = Styler::from_config(&config);
        println!(
            "{}",
            styler.success("Audit hash chain verified successfully")
        );
        if unsealed > 0 {
            eprintln!(
                "warning: {unsealed} legacy audit entries lack sealed hashes and are not tamper-evident — run mh audit --rebuild-chain --yes"
            );
        }
        return Ok(());
    }

    let styler = Styler::from_config(&config);
    let rows = database.audit_rows(args.today, args.limit)?;

    let mut display_rows = rows;
    for row in &mut display_rows {
        if let Some(command) = row.raw_command.as_ref() {
            row.raw_command = Some(crate::security::redact_for_audit(command, &config)?);
        }
    }

    match args.format {
        crate::cli::AuditFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&display_rows)?)
        }
        crate::cli::AuditFormat::Table => {
            use crate::output::table_format::{header_cell, new_table, print_section};

            let mut table = new_table();
            table.set_header(vec![
                header_cell(&styler, "ID"),
                header_cell(&styler, "Time"),
                header_cell(&styler, "Type"),
                header_cell(&styler, "Reason"),
                header_cell(&styler, "Command"),
                header_cell(&styler, "Hash"),
            ]);
            for row in display_rows {
                table.add_row(vec![
                    styler.cell(row.id, None),
                    styler.cell(row.created_at, None),
                    styler.audit_event_cell(&row.event_type),
                    styler.cell(row.reason.unwrap_or_else(|| "-".to_string()), None),
                    styler.cell(row.raw_command.unwrap_or_else(|| "-".to_string()), None),
                    styler.cell(
                        row.entry_hash
                            .map(|hash| hash.chars().take(12).collect::<String>())
                            .unwrap_or_else(|| "-".to_string()),
                        None,
                    ),
                ]);
            }
            print_section(&styler, "Audit log", &table);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn audit_chain_verifies_after_inserts() {
        let temp_dir = crate::config::private_tempdir().expect("temp dir");
        let mut config = AppConfig::default();
        config.database.path = temp_dir
            .path()
            .join("history.db")
            .to_string_lossy()
            .to_string();
        let database = Database::open(&config).expect("database should open");
        database
            .insert_audit_log(
                "skipped",
                "secret cmd",
                "private",
                Some("user"),
                Some("host"),
            )
            .expect("audit insert");
        database
            .insert_audit_log("risky", "rm -rf /", "critical", Some("user"), Some("host"))
            .expect("audit insert");
        database.verify_audit_chain().expect("chain should verify");
    }
}
