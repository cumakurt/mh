use std::io::Read;

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zstd::stream::{Decoder, encode_all};

use crate::config::{AppConfig, config_path};
use crate::db::Database;
use crate::models::{CommandRecord, CommandRow, SearchFilters};
use crate::security::{SecurityAction, SecurityEngine};
use crate::sync::crypto::{decrypt_payload, encrypt_payload};

#[derive(Debug, Serialize, Deserialize)]
struct SyncEnvelope {
    device_id: String,
    synced_at: String,
    commands: Vec<CommandRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncRequest {
    device_id: String,
    synced_at: String,
    payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncResponse {
    payload: String,
    synced_at: Option<String>,
}

const MAX_SYNC_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_SYNC_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
const MAX_SYNC_ROWS: usize = 1_000_000;

pub fn push(config: &AppConfig, database: &Database) -> Result<()> {
    let after = if config.sync.last_synced_at.trim().is_empty() {
        None
    } else {
        Some(config.sync.last_synced_at.clone())
    };

    let mut rows = database.search_commands(&SearchFilters {
        query: None,
        cwd: None,
        failed: false,
        success: false,
        user: None,
        shell: None,
        after,
        before: None,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: None,
        category: None,
        pinned: false,
        duration_gt: None,
        duration_lt: None,
        hostname: None,
        ssh: false,
        root: false,
        limit: 1_000_000,
        session_id: None,
        git_repo: None,
        git_branch: None,
        git_commit: None,
        environment: None,
    })?;

    for row in &mut rows {
        row.command = crate::security::redact_for_audit(&row.command, config)?;
        if row.command.contains("****") {
            row.is_masked = true;
        }
    }

    let synced_at = now_rfc3339();
    let envelope = SyncEnvelope {
        device_id: config.sync.device_id.clone(),
        synced_at: synced_at.clone(),
        commands: rows,
    };

    let json = serde_json::to_vec(&envelope).context("failed to serialize sync payload")?;
    let compressed = encode_all(json.as_slice(), 19)?;
    let body = if config.sync.encrypt_payload {
        encrypt_payload(&config.sync.token, &compressed)?
    } else {
        compressed
    };
    let payload = STANDARD.encode(body);

    let client = http_client()?;
    let url = sync_url(&config.sync.server_url, "push")?;
    let response = client
        .post(url)
        .bearer_auth(&config.sync.token)
        .json(&SyncRequest {
            device_id: config.sync.device_id.clone(),
            synced_at: synced_at.clone(),
            payload,
        })
        .send()
        .context("sync push request failed")?;

    if !response.status().is_success() {
        bail!(
            "sync push failed with HTTP {}: {}",
            response.status(),
            response.text().unwrap_or_default()
        );
    }

    save_last_synced_at(config, &synced_at)?;
    println!("Pushed {} command record(s)", envelope.commands.len());
    Ok(())
}

pub fn pull(config: &AppConfig, database: &Database) -> Result<()> {
    let client = http_client()?;
    let url = sync_url(&config.sync.server_url, "pull")?;
    let response = client
        .get(url)
        .bearer_auth(&config.sync.token)
        .query(&[("device_id", config.sync.device_id.as_str())])
        .send()
        .context("sync pull request failed")?;

    if !response.status().is_success() {
        bail!(
            "sync pull failed with HTTP {}: {}",
            response.status(),
            response.text().unwrap_or_default()
        );
    }

    let body: SyncResponse = response
        .json()
        .context("failed to parse sync pull response")?;
    if body.payload.trim().is_empty() {
        println!("No remote history to pull");
        return Ok(());
    }

    let encoded = STANDARD
        .decode(body.payload.as_bytes())
        .context("invalid sync payload encoding")?;
    if encoded.len() > MAX_SYNC_PAYLOAD_BYTES {
        bail!(
            "sync payload exceeds maximum size of {} bytes",
            MAX_SYNC_PAYLOAD_BYTES
        );
    }
    let compressed = if config.sync.encrypt_payload {
        decrypt_payload(&config.sync.token, &encoded)?
    } else {
        encoded
    };
    let json = decode_zstd_bounded(compressed.as_slice())?;
    let envelope: SyncEnvelope =
        serde_json::from_slice(&json).context("failed to parse sync payload")?;
    if envelope.commands.len() > MAX_SYNC_ROWS {
        bail!(
            "sync payload contains {} rows, exceeding the maximum of {}",
            envelope.commands.len(),
            MAX_SYNC_ROWS
        );
    }

    let security = SecurityEngine::from_config(config)?;
    let mut imported = 0;
    let mut skipped = 0;
    for row in envelope.commands {
        let decision = security.process(&row.command, config)?;
        if matches!(decision.action, SecurityAction::Skipped(_)) {
            skipped += 1;
            continue;
        }

        let command = decision.command;
        let hash = hash_command(&command);
        if database.command_hash_exists(&hash)? {
            skipped += 1;
            continue;
        }
        database.insert_command(&CommandRecord {
            command,
            command_hash: hash,
            cwd: row.cwd,
            shell: row.shell,
            username: row.username,
            hostname: row.hostname,
            exit_code: row.exit_code,
            duration_ms: row.duration_ms,
            started_at: row.started_at,
            finished_at: None,
            session_id: row.session_id,
            tty: None,
            is_ssh: false,
            is_root: false,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment_tier: None,
            category: row.category,
            env_context: None,
            is_pinned: row.is_pinned,
            is_masked: matches!(decision.action, SecurityAction::Masked) || row.is_masked,
            tags: row.tags,
        })?;
        imported += 1;
    }

    let synced_at = body
        .synced_at
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(envelope.synced_at);
    if imported > 0 {
        database.enforce_max_entries(config.history.max_entries, config.database.auto_vacuum)?;
    }

    save_last_synced_at(config, &synced_at)?;
    println!("Pulled {imported} command record(s), skipped {skipped}");
    Ok(())
}

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("failed to create HTTP client")
}

fn sync_url(base: &str, action: &str) -> Result<String> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("sync server URL must not be empty");
    }
    let mut url = reqwest::Url::parse(trimmed).context("sync server URL must be absolute")?;
    match url.scheme() {
        "https" => {}
        "http" if is_local_http_url(&url) => {}
        "http" => bail!("sync server URL must use https unless it points to localhost"),
        scheme => bail!("unsupported sync server URL scheme: {scheme}"),
    }
    url.set_query(None);
    url.set_fragment(None);
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("sync server URL cannot be used as a base URL"))?
        .pop_if_empty()
        .extend(["api", "v1", "sync", action]);
    Ok(url.to_string())
}

fn is_local_http_url(url: &reqwest::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn decode_zstd_bounded(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = Decoder::new(input).context("failed to open sync zstd payload")?;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = decoder
            .read(&mut buffer)
            .context("failed to decompress sync payload")?;
        if read == 0 {
            break;
        }
        if output.len() + read > MAX_SYNC_DECOMPRESSED_BYTES {
            bail!(
                "decompressed sync payload exceeds maximum size of {} bytes",
                MAX_SYNC_DECOMPRESSED_BYTES
            );
        }
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}

fn save_last_synced_at(config: &AppConfig, synced_at: &str) -> Result<()> {
    let path = config_path();
    let mut updated = config.clone();
    updated.sync.last_synced_at = synced_at.to_string();
    updated.write_to_path(&path)
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn hash_command(command: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_plain_http_remote_url() {
        let error = sync_url("http://history.example.com", "push")
            .expect_err("remote http URL should be rejected");
        assert!(format!("{error:#}").contains("https"));
    }

    #[test]
    fn allows_local_http_url() {
        let url = sync_url("http://127.0.0.1:8080/base", "pull").expect("local URL");
        assert_eq!(url, "http://127.0.0.1:8080/base/api/v1/sync/pull");
    }

    #[test]
    fn rejects_oversized_decompressed_payload() {
        let mut input = Vec::new();
        let mut encoder = zstd::stream::Encoder::new(&mut input, 1).expect("encoder");
        let chunk = vec![b'a'; 1024];
        for _ in 0..(MAX_SYNC_DECOMPRESSED_BYTES / 1024 + 1) {
            std::io::Write::write_all(&mut encoder, &chunk).expect("write");
        }
        encoder.finish().expect("finish");

        let error =
            decode_zstd_bounded(&input).expect_err("oversized decompressed payload should fail");
        assert!(format!("{error:#}").contains("decompressed sync payload exceeds"));
    }
}
