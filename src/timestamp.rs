use anyhow::{Context, Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};

/// UTC midnight for the current calendar day.
pub fn today_start_utc() -> Result<DateTime<Utc>> {
    Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|time| time.and_utc())
        .context("failed to construct UTC midnight for today")
}

/// Validates and normalizes an RFC3339 timestamp string.
pub fn parse_rfc3339(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("timestamp must not be empty");
    }
    DateTime::parse_from_rfc3339(trimmed)
        .with_context(|| format!("invalid RFC3339 timestamp: {trimmed}"))?;
    Ok(trimmed.to_string())
}

/// Parses an optional RFC3339 timestamp; empty values become `None`.
pub fn parse_optional_rfc3339(label: &str, value: Option<&String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_rfc3339(trimmed)
        .map(Some)
        .with_context(|| format!("invalid {label} timestamp"))
}

/// Parses import timestamps, defaulting empty values to the current time.
pub fn parse_import_timestamp(value: &str, context: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    parse_rfc3339(trimmed).with_context(|| format!("invalid import timestamp in {context}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_rfc3339_timestamp() {
        let parsed = parse_rfc3339("2026-05-31T12:00:00+00:00").expect("parse");
        assert_eq!(parsed, "2026-05-31T12:00:00+00:00");
    }

    #[test]
    fn rejects_invalid_timestamp() {
        assert!(parse_rfc3339("not-a-date").is_err());
    }

    #[test]
    fn import_timestamp_defaults_empty_to_now() {
        let parsed = parse_import_timestamp("", "line 2").expect("default");
        assert!(parsed.contains('T'));
    }
}
