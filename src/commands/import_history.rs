use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::cli::ImportArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{CommandRecord, CommandRow};
use crate::identity;
use crate::security::{SecurityAction, SecurityEngine};

pub fn run(args: ImportArgs) -> Result<()> {
    let mut rows = read_import_rows(&args.file)?;
    for (index, row) in rows.iter_mut().enumerate() {
        row.started_at = crate::timestamp::parse_import_timestamp(
            &row.started_at,
            &format!("import record {}", index + 1),
        )?;
    }
    if args.dry_run {
        println!("Would import {} command record(s)", rows.len());
        return Ok(());
    }

    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let engine = SecurityEngine::from_config(&config)?;
    let mut imported = 0;
    let mut skipped = 0;

    for row in rows {
        let decision = engine.process(&row.command, &config)?;
        if matches!(decision.action, SecurityAction::Skipped(_)) {
            skipped += 1;
            continue;
        }

        let hash = hash_command(&decision.command);
        if args.merge && database.command_hash_exists(&hash)? {
            skipped += 1;
            continue;
        }

        database.insert_command(&CommandRecord {
            command: decision.command,
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
            is_root: identity::is_effective_root(),
            git_repo: row.git_repo,
            git_branch: row.git_branch,
            git_commit: row.git_commit,
            environment_tier: row.environment_tier,
            category: row.category,
            env_context: None,
            is_pinned: row.is_pinned,
            is_masked: matches!(decision.action, SecurityAction::Masked),
            tags: row.tags,
        })?;
        imported += 1;
    }

    if imported > 0 {
        database.enforce_max_entries(config.history.max_entries, config.database.auto_vacuum)?;
    }

    println!("Imported {imported} command record(s), skipped {skipped}");
    Ok(())
}

const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 1_000_000;

fn read_import_rows(path: &str) -> Result<Vec<CommandRow>> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read import file metadata: {path}"))?;
    if metadata.len() as usize > MAX_IMPORT_BYTES {
        bail!(
            "import file exceeds maximum size of {} bytes",
            MAX_IMPORT_BYTES
        );
    }
    let bytes = fs::read(path).with_context(|| format!("failed to read import file: {path}"))?;
    let data = if path.ends_with(".zst") {
        decode_zstd_bounded(bytes.as_slice())?
    } else {
        bytes
    };

    if data.len() > MAX_DECOMPRESSED_BYTES {
        bail!(
            "import payload exceeds maximum size of {} bytes after decompression",
            MAX_DECOMPRESSED_BYTES
        );
    }

    let rows = if Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    {
        parse_csv(std::str::from_utf8(&data)?)?
    } else {
        serde_json::from_slice(&data)?
    };

    if rows.len() > MAX_IMPORT_ROWS {
        bail!(
            "import file contains {} rows, exceeding the maximum of {}",
            rows.len(),
            MAX_IMPORT_ROWS
        );
    }

    Ok(rows)
}

fn decode_zstd_bounded(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = zstd::stream::Decoder::new(input).context("failed to open zstd decoder")?;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = decoder
            .read(&mut buffer)
            .context("failed to decompress import file")?;
        if read == 0 {
            break;
        }
        if output.len() + read > MAX_DECOMPRESSED_BYTES {
            bail!(
                "decompressed import exceeds maximum size of {} bytes",
                MAX_DECOMPRESSED_BYTES
            );
        }
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}

fn parse_csv(content: &str) -> Result<Vec<CommandRow>> {
    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let columns = parse_csv_line(line);
        if columns.len() != 9 {
            bail!(
                "invalid CSV row at line {}: expected 9 columns, got {}",
                index + 1,
                columns.len()
            );
        }
        let id = columns[0]
            .parse::<i64>()
            .with_context(|| format!("invalid command id at CSV line {}", index + 1))?;
        rows.push(CommandRow {
            id,
            started_at: crate::timestamp::parse_import_timestamp(
                &columns[1],
                &format!("CSV line {}", index + 1),
            )?,
            exit_code: parse_optional(&columns[2]),
            duration_ms: parse_optional(&columns[3]),
            cwd: empty_to_none(&columns[4]),
            shell: empty_to_none(&columns[5]),
            category: empty_to_none(&columns[6]),
            command: columns[7].clone(),
            tags: columns[8]
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            username: None,
            hostname: None,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment_tier: None,
            is_pinned: false,
            is_masked: false,
        });
    }
    Ok(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                columns.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    columns.push(current);
    columns
}

fn parse_optional<T: std::str::FromStr>(value: &str) -> Option<T> {
    if value.trim().is_empty() {
        None
    } else {
        value.parse().ok()
    }
}

fn empty_to_none(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
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
    fn rejects_oversized_decompressed_payload() {
        let mut input = Vec::new();
        let mut encoder = zstd::stream::Encoder::new(&mut input, 1).expect("encoder");
        let chunk = vec![b'a'; 1024];
        for _ in 0..(MAX_DECOMPRESSED_BYTES / 1024 + 1) {
            std::io::Write::write_all(&mut encoder, &chunk).expect("write");
        }
        encoder.finish().expect("finish");
        let result = decode_zstd_bounded(&input);
        assert!(result.is_err());
    }
}
