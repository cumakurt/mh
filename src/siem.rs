use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::{AppConfig, SiemConfig};
use crate::models::AuditRow;

pub fn emit_audit_event(config: &AppConfig, row: &AuditRow) {
    if !config.siem.enabled {
        return;
    }

    let _ = match config.siem.format.as_str() {
        "cef" => emit_cef(&config.siem, row),
        "json" => emit_json(&config.siem, row),
        _ => emit_syslog(&config.siem, row),
    };
}

fn emit_syslog(config: &SiemConfig, row: &AuditRow) -> Result<()> {
    let message = format_syslog(row);
    if let Some(url) = config
        .syslog_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_tcp(url, &message)?;
    }
    if let Some(url) = config
        .webhook_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_webhook(url, &message)?;
    }
    Ok(())
}

fn emit_json(config: &SiemConfig, row: &AuditRow) -> Result<()> {
    let payload = serde_json::to_string(row)?;
    if let Some(url) = config
        .webhook_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_webhook(url, &payload)?;
    } else if let Some(url) = config
        .syslog_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_tcp(url, &payload)?;
    }
    Ok(())
}

fn emit_cef(config: &SiemConfig, row: &AuditRow) -> Result<()> {
    let message = format_cef(row);
    if let Some(url) = config
        .syslog_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_tcp(url, &message)?;
    }
    if let Some(url) = config
        .webhook_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        send_webhook(url, &message)?;
    }
    Ok(())
}

fn format_syslog(row: &AuditRow) -> String {
    format!(
        "<134>1 {} {} mh - - - event={} reason={} command={}",
        flatten_log_value(&row.created_at),
        flatten_log_value(row.hostname.as_deref().unwrap_or("-")),
        flatten_log_value(&row.event_type),
        flatten_log_value(row.reason.as_deref().unwrap_or("-")),
        flatten_log_value(row.raw_command.as_deref().unwrap_or("-"))
    )
}

fn format_cef(row: &AuditRow) -> String {
    format!(
        "CEF:0|mh|mh|0.1.0|{}|{}|5|rt={} msg={} cs1={} suser={} shost={}",
        escape_cef_value(&row.event_type),
        escape_cef_value(&row.event_type),
        escape_cef_value(&row.created_at),
        escape_cef_value(row.reason.as_deref().unwrap_or("-")),
        escape_cef_value(row.raw_command.as_deref().unwrap_or("-")),
        escape_cef_value(row.username.as_deref().unwrap_or("-")),
        escape_cef_value(row.hostname.as_deref().unwrap_or("-"))
    )
}

fn flatten_log_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn escape_cef_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str(r"\\"),
            '=' => escaped.push_str(r"\="),
            '|' => escaped.push_str(r"\|"),
            '\n' | '\r' | '\t' => escaped.push(' '),
            character if character.is_control() => escaped.push(' '),
            character => escaped.push(character),
        }
    }
    escaped
}

fn send_tcp(address: &str, message: &str) -> Result<()> {
    let mut stream = TcpStream::connect(address)
        .with_context(|| format!("failed to connect to syslog endpoint {address}"))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(message.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn send_webhook(url: &str, message: &str) -> Result<()> {
    #[cfg(feature = "sync")]
    {
        use reqwest::blocking::Client;

        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
        client
            .post(url)
            .header("content-type", "application/json")
            .body(serde_json::json!({ "message": message }).to_string())
            .send()
            .with_context(|| format!("failed to POST audit event to {url}"))?;
        Ok(())
    }

    #[cfg(not(feature = "sync"))]
    {
        let _ = (url, message);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_row(raw_command: &str) -> AuditRow {
        AuditRow {
            id: 1,
            event_type: "masked".to_string(),
            raw_command: Some(raw_command.to_string()),
            reason: Some("secret\nmasked".to_string()),
            username: Some("user".to_string()),
            hostname: Some("host\rname".to_string()),
            created_at: "2026-06-01T00:00:00Z".to_string(),
            prev_hash: None,
            entry_hash: None,
        }
    }

    #[test]
    fn syslog_output_flattens_control_characters() {
        let message = format_syslog(&audit_row("echo one\n<134> forged"));
        assert!(!message.contains('\n'));
        assert!(!message.contains('\r'));
        assert!(message.contains("echo one <134> forged"));
    }

    #[test]
    fn cef_output_escapes_extension_separators() {
        let message = format_cef(&audit_row(r#"echo a=b|c\z"#));
        assert!(message.contains(r#"echo a\=b\|c\\z"#));
        assert!(!message.contains('\n'));
        assert!(!message.contains('\r'));
    }
}
