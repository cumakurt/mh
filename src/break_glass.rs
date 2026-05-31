use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{self, config_path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreakGlassState {
    pub reason: String,
    pub expires_at: String,
    pub activated_at: String,
}

pub fn break_glass_path() -> PathBuf {
    config_path()
        .parent()
        .map(|path| path.join("break_glass"))
        .unwrap_or_else(|| PathBuf::from(".mh-break-glass"))
}

pub fn is_active() -> bool {
    read_state()
        .ok()
        .flatten()
        .is_some_and(|state| !state.is_expired())
}

pub fn read_state() -> Result<Option<BreakGlassState>> {
    let path = break_glass_path();
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read break-glass state from {}", path.display()))?;
    let state: BreakGlassState = toml::from_str(&content)
        .with_context(|| format!("failed to parse break-glass state from {}", path.display()))?;
    if state.is_expired() {
        let _ = deactivate();
        return Ok(None);
    }
    Ok(Some(state))
}

pub fn activate(reason: &str, ttl_hours: u64) -> Result<BreakGlassState> {
    let now = Utc::now();
    let expires = now + chrono::Duration::hours(ttl_hours as i64);
    let state = BreakGlassState {
        reason: reason.to_string(),
        expires_at: expires.to_rfc3339(),
        activated_at: now.to_rfc3339(),
    };
    write_state(&state)?;
    Ok(state)
}

pub fn deactivate() -> Result<()> {
    let path = break_glass_path();
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove break-glass state at {}", path.display()))?;
    }
    Ok(())
}

fn write_state(state: &BreakGlassState) -> Result<()> {
    let path = break_glass_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(state)?;
    config::write_private_file(&path, content.as_bytes())?;
    Ok(())
}

impl BreakGlassState {
    pub fn is_expired(&self) -> bool {
        DateTime::parse_from_rfc3339(&self.expires_at)
            .map(|value| value.with_timezone(&Utc) < Utc::now())
            .unwrap_or(true)
    }

    pub fn remaining_label(&self) -> String {
        DateTime::parse_from_rfc3339(&self.expires_at)
            .map(|value| {
                let remaining = value.with_timezone(&Utc) - Utc::now();
                if remaining.num_seconds() <= 0 {
                    "expired".to_string()
                } else {
                    format!("{} minutes", remaining.num_minutes())
                }
            })
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_state_is_detected() {
        let state = BreakGlassState {
            reason: "incident".to_string(),
            expires_at: "2020-01-01T00:00:00Z".to_string(),
            activated_at: "2020-01-01T00:00:00Z".to_string(),
        };
        assert!(state.is_expired());
    }
}
