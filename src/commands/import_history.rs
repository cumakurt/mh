use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::cli::ImportArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::identity;
use crate::models::{CommandRecord, CommandRow};
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
            duration_ms: row.duration_ms.map(|duration| duration.max(0)),
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
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(content.as_bytes());

    for (index, record) in reader.records().enumerate() {
        let record = record.with_context(|| format!("invalid CSV row at line {}", index + 2))?;
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if record.len() != 9 {
            bail!(
                "invalid CSV row at line {}: expected 9 columns, got {}",
                index + 2,
                record.len()
            );
        }
        let line_number = index + 2;
        let id = record[0]
            .parse::<i64>()
            .with_context(|| format!("invalid command id at CSV line {line_number}"))?;
        rows.push(CommandRow {
            id,
            started_at: crate::timestamp::parse_import_timestamp(
                &record[1],
                &format!("CSV line {line_number}"),
            )?,
            exit_code: parse_optional(&record[2], "exit_code", line_number)?,
            duration_ms: parse_optional(&record[3], "duration_ms", line_number)?,
            cwd: empty_to_none(&record[4]),
            shell: empty_to_none(&record[5]),
            category: empty_to_none(&record[6]),
            command: record[7].to_string(),
            tags: record[8]
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

fn parse_optional<T: std::str::FromStr>(
    value: &str,
    field: &str,
    line_number: usize,
) -> Result<Option<T>>
where
    T::Err: std::fmt::Display,
{
    if value.trim().is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|error| anyhow::anyhow!("invalid {field} at CSV line {line_number}: {error}"))
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

    #[test]
    fn rejects_unterminated_csv_quotes() {
        let error = parse_csv("id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags\n1,\"2026-06-01T00:00:00Z,0,,,,echo,\n")
            .expect_err("unterminated quote should fail");
        assert!(format!("{error:#}").contains("CSV"));
    }

    #[test]
    fn parses_multiline_csv_commands() {
        let rows = parse_csv(
            "id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags\n\
             1,2026-06-01T00:00:00Z,0,10,/tmp,zsh,test,\"echo one\n\
             echo two\",tag\n",
        )
        .expect("multiline CSV command should parse");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "echo one\necho two");
    }

    #[test]
    fn rejects_invalid_csv_numeric_fields() {
        let error = parse_csv(
            "id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags\n\
             1,2026-06-01T00:00:00Z,nope,10,/tmp,zsh,test,echo,tag\n",
        )
        .expect_err("invalid exit code should fail");

        assert!(format!("{error:#}").contains("invalid exit_code"));
    }
}
