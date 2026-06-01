use std::path::Path;

use anyhow::Result;

use crate::cli::{IncidentArgs, IncidentCommand};
use crate::config::AppConfig;
use crate::db::Database;

pub fn run(args: IncidentArgs) -> Result<()> {
    match args.command {
        IncidentCommand::Export {
            session,
            output,
            include_secrets,
        } => export_bundle(&session, &output, include_secrets),
    }
}

fn export_bundle(session: &str, output: &str, include_secrets: bool) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let bundle = crate::incident_bundle::build(&config, &database, session, include_secrets)?;
    let payload = serde_json::to_vec_pretty(&bundle)?;
    crate::config::write_private_file(Path::new(output), &payload)?;
    println!(
        "Exported incident bundle for session {} to {} ({} command record(s), {} risky)",
        session,
        output,
        bundle.command_count,
        bundle.risky_commands.len()
    );
    Ok(())
}
