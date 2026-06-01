use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;

use crate::audit_chain;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{AuditRow, TimelineEntry};
use crate::risk;

#[derive(Debug, Serialize)]
pub struct IncidentBundle {
    pub generated_at: String,
    pub session_id: String,
    pub command_count: usize,
    pub audit_chain_verified: bool,
    pub audit_unsealed_entries: usize,
    pub audit_last_hash: String,
    pub timeline: Vec<TimelineEntry>,
    pub risky_commands: Vec<TimelineEntry>,
    pub audit_events: Vec<AuditRow>,
}

pub fn build(
    config: &AppConfig,
    database: &Database,
    session_id: &str,
    include_secrets: bool,
) -> Result<IncidentBundle> {
    let mut timeline = database.session_timeline(session_id)?;
    if timeline.is_empty() {
        bail!("no commands found for session {session_id}");
    }

    let mut risky_commands = Vec::new();
    for entry in &mut timeline {
        let assessment = risk::assess_command(&entry.command);
        entry.risk_level = assessment
            .as_ref()
            .map(|assessment| assessment.level.label().to_string());
        if assessment.is_some() {
            risky_commands.push(entry.clone());
        }
    }

    if !include_secrets {
        for entry in &mut timeline {
            entry.command = crate::security::redact_for_audit(&entry.command, config)?;
        }
        for entry in &mut risky_commands {
            entry.command = crate::security::redact_for_audit(&entry.command, config)?;
        }
    }

    let audit_rows = database.audit_rows_chronological(usize::MAX)?;
    let audit_unsealed_entries = audit_chain::count_unsealed_entries(&audit_rows);
    let audit_chain_verified = database.verify_audit_chain().is_ok();
    let mut audit_events = database.audit_rows(false, 500)?;
    if !include_secrets {
        for row in &mut audit_events {
            if let Some(command) = row.raw_command.as_deref() {
                row.raw_command = Some(crate::security::redact_for_audit(command, config)?);
            }
            if let Some(reason) = row.reason.as_deref() {
                row.reason = Some(crate::security::redact_for_audit(reason, config)?);
            }
        }
    }

    Ok(IncidentBundle {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        session_id: session_id.to_string(),
        command_count: timeline.len(),
        audit_chain_verified,
        audit_unsealed_entries,
        audit_last_hash: database
            .last_audit_hash()
            .context("failed to read last audit hash")?,
        timeline,
        risky_commands,
        audit_events,
    })
}
