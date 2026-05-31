use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::models::AuditRow;

pub fn compute_entry_hash(
    prev_hash: &str,
    event_type: &str,
    raw_command: Option<&str>,
    reason: Option<&str>,
    username: Option<&str>,
    hostname: Option<&str>,
    created_at: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(event_type.as_bytes());
    hasher.update(raw_command.unwrap_or("").as_bytes());
    hasher.update(reason.unwrap_or("").as_bytes());
    hasher.update(username.unwrap_or("").as_bytes());
    hasher.update(hostname.unwrap_or("").as_bytes());
    hasher.update(created_at.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn verify_chain(rows: &[AuditRow]) -> Result<()> {
    let mut prev_hash = String::new();
    let mut chain_started = false;

    for row in rows {
        if let Some(stored_prev) = row.prev_hash.as_deref()
            && chain_started
            && stored_prev != prev_hash
        {
            anyhow::bail!(
                "audit chain prev_hash mismatch at id {}: expected {prev_hash}, stored {stored_prev}",
                row.id
            );
        }

        let expected = compute_entry_hash(
            &prev_hash,
            &row.event_type,
            row.raw_command.as_deref(),
            row.reason.as_deref(),
            row.username.as_deref(),
            row.hostname.as_deref(),
            &row.created_at,
        );
        let actual = row.entry_hash.as_deref().unwrap_or("");

        if chain_started {
            if actual.is_empty() {
                anyhow::bail!(
                    "audit chain gap at id {}: entry is missing a hash after the chain was sealed",
                    row.id
                );
            }
            if actual != expected {
                anyhow::bail!(
                    "audit chain broken at id {}: expected {expected}, got {actual}",
                    row.id
                );
            }
            prev_hash = actual.to_string();
            continue;
        }

        if actual.is_empty() {
            prev_hash = expected;
            continue;
        }

        chain_started = true;
        if actual != expected {
            anyhow::bail!(
                "audit chain broken at id {}: expected {expected}, got {actual}",
                row.id
            );
        }
        prev_hash = actual.to_string();
    }

    Ok(())
}

pub fn count_unsealed_entries(rows: &[AuditRow]) -> usize {
    rows.iter()
        .filter(|row| row.entry_hash.as_deref().unwrap_or("").is_empty())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_for_same_input() {
        let first = compute_entry_hash(
            "",
            "risky",
            Some("rm -rf /"),
            Some("critical"),
            None,
            None,
            "t1",
        );
        let second = compute_entry_hash(
            "",
            "risky",
            Some("rm -rf /"),
            Some("critical"),
            None,
            None,
            "t1",
        );
        assert_eq!(first, second);
    }

    #[test]
    fn chain_links_previous_hash() {
        let first_hash =
            compute_entry_hash("", "masked", Some("curl"), Some("secret"), None, None, "t1");
        let second_hash = compute_entry_hash(
            &first_hash,
            "risky",
            Some("rm -rf /"),
            Some("critical"),
            None,
            None,
            "t2",
        );
        assert_ne!(first_hash, second_hash);
    }

    #[test]
    fn rejects_gap_after_sealed_chain() {
        let rows = vec![
            AuditRow {
                id: 1,
                event_type: "risky".to_string(),
                raw_command: Some("a".to_string()),
                reason: Some("r".to_string()),
                username: None,
                hostname: None,
                created_at: "t1".to_string(),
                prev_hash: Some(String::new()),
                entry_hash: Some(compute_entry_hash(
                    "",
                    "risky",
                    Some("a"),
                    Some("r"),
                    None,
                    None,
                    "t1",
                )),
            },
            AuditRow {
                id: 2,
                event_type: "risky".to_string(),
                raw_command: Some("b".to_string()),
                reason: Some("r".to_string()),
                username: None,
                hostname: None,
                created_at: "t2".to_string(),
                prev_hash: Some(String::new()),
                entry_hash: Some(String::new()),
            },
        ];
        assert!(verify_chain(&rows).is_err());
    }
}
