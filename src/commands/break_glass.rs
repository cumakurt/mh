use anyhow::Result;

use crate::break_glass::{self, BreakGlassState};
use crate::cli::{BreakGlassArgs, BreakGlassCommand};
use crate::config::AppConfig;
use crate::db::Database;
use crate::output::styling::Styler;

pub fn run(args: BreakGlassArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;
    let username = Some(whoami::username());
    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());

    match args.command {
        BreakGlassCommand::On { reason, ttl_hours } => {
            let ttl = ttl_hours.unwrap_or(config.break_glass.default_ttl_hours);
            let state = break_glass::activate(&reason, ttl)?;
            if config.security.audit_log {
                let row = database.insert_audit_log(
                    "break_glass",
                    "",
                    &format!("activated: {reason}"),
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(&config, &row);
            }
            print_active(&styler, &state);
        }
        BreakGlassCommand::Off => {
            break_glass::deactivate()?;
            if config.security.audit_log {
                let row = database.insert_audit_log(
                    "break_glass",
                    "",
                    "deactivated",
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(&config, &row);
            }
            println!("{}", styler.success("Break-glass mode deactivated"));
        }
        BreakGlassCommand::Status => {
            if let Some(state) = break_glass::read_state()? {
                print_active(&styler, &state);
            } else {
                println!("{}", styler.success("Break-glass mode is inactive"));
            }
        }
    }

    Ok(())
}

fn print_active(styler: &Styler, state: &BreakGlassState) {
    println!(
        "{} reason={} expires={} remaining={}",
        styler.warning("Break-glass mode is active"),
        state.reason,
        state.expires_at,
        state.remaining_label()
    );
}
