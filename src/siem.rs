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
        row.created_at,
        row.hostname.as_deref().unwrap_or("-"),
        row.event_type,
        row.reason.as_deref().unwrap_or("-"),
        row.raw_command.as_deref().unwrap_or("-")
    )
}

fn format_cef(row: &AuditRow) -> String {
    format!(
        "CEF:0|mh|mh|0.1.0|{}|{}|5|rt={} msg={} cs1={} suser={} shost={}",
        row.event_type,
        row.event_type,
        row.created_at,
        row.reason.as_deref().unwrap_or("-"),
        row.raw_command.as_deref().unwrap_or("-"),
        row.username.as_deref().unwrap_or("-"),
        row.hostname.as_deref().unwrap_or("-")
    )
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
        return Ok(());
    }

    #[cfg(not(feature = "sync"))]
    {
        let _ = (url, message);
        Ok(())
    }
}
