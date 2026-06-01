use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};

use crate::cli::ExportArgs;
use crate::config::{self, AppConfig};
use crate::db::Database;
use crate::models::{CommandRow, SearchFilters};
use crate::output::{csv, markdown};
use crate::security;

pub fn run(args: ExportArgs) -> Result<()> {
    let targets = [
        args.json.as_ref(),
        args.csv.as_ref(),
        args.markdown.as_ref(),
        args.compressed.as_ref(),
        args.sqlite.as_ref(),
    ]
    .into_iter()
    .flatten()
    .count();
    if targets != 1 {
        bail!(
            "choose exactly one export format: --json, --csv, --markdown, --compressed, or --sqlite"
        );
    }

    if args.without_audit && args.sanitize_audit {
        bail!("--without-audit and --sanitize-audit cannot be used together");
    }
    let sanitize_exports = !args.include_secrets && args.sanitize;

    let config = AppConfig::load()?;
    let database = Database::open(&config)?;

    if let Some(path) = args.sqlite {
        export_sqlite(
            &database,
            &path,
            &config,
            args.without_audit,
            args.sanitize_audit,
            sanitize_exports,
        )?;
        return Ok(());
    }

    let mut rows = database.search_commands(&SearchFilters {
        query: None,
        cwd: None,
        failed: false,
        success: false,
        user: None,
        shell: None,
        after: args.after,
        before: args.before,
        regex: false,
        fuzzy: false,
        fts: false,
        tag: args.tag,
        category: args.category,
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

    if rows.len() > 100_000 {
        eprintln!(
            "warning: exporting {} rows loads the full result set into memory; consider narrowing with --after/--before/--tag",
            rows.len()
        );
    }

    if sanitize_exports {
        sanitize_command_rows(&mut rows, &config)?;
    } else {
        eprintln!(
            "warning: export includes full command text; omit --include-secrets to redact secrets by default"
        );
    }

    if let Some(path) = args.json {
        write_text(&path, &serde_json::to_string_pretty(&rows)?)?;
    } else if let Some(path) = args.csv {
        write_text(&path, &csv::format_rows(&rows))?;
    } else if let Some(path) = args.markdown {
        write_text(&path, &markdown::format_rows(&rows))?;
    } else if let Some(path) = args.compressed {
        let data = serde_json::to_vec_pretty(&rows)?;
        let compressed = zstd::stream::encode_all(data.as_slice(), 19)?;
        write_bytes(&path, &compressed)?;
    }

    println!("Exported {} command record(s)", rows.len());
    Ok(())
}

fn export_sqlite(
    database: &Database,
    path: &str,
    config: &AppConfig,
    without_audit: bool,
    sanitize_audit: bool,
    sanitize_commands: bool,
) -> Result<()> {
    database.checkpoint_wal()?;
    config::copy_file_safely(database.path(), Path::new(path))
        .with_context(|| format!("failed to copy database to {path}"))?;
    config::restrict_file_permissions(Path::new(path))?;
    for extension in ["db-wal", "db-shm"] {
        let sidecar = Path::new(path).with_extension(extension);
        if sidecar.exists() {
            config::restrict_file_permissions(&sidecar).with_context(|| {
                format!("failed to restrict permissions on {}", sidecar.display())
            })?;
        }
    }

    let connection = Connection::open(path)
        .with_context(|| format!("failed to open exported database at {path}"))?;

    if sanitize_commands {
        sanitize_commands_table(&connection, config)?;
    } else if !without_audit && !sanitize_audit {
        eprintln!(
            "warning: SQLite export includes full command text; use --sanitize or --sanitize-audit"
        );
    }

    if without_audit {
        connection
            .execute("DELETE FROM audit_log", [])
            .context("failed to clear audit_log from export")?;
        println!("Exported SQLite database to {path} (audit_log omitted)");
        return Ok(());
    }

    if sanitize_audit {
        sanitize_audit_log(&connection, config)?;
        println!("Exported SQLite database to {path} (audit_log sanitized, chain re-sealed)");
        return Ok(());
    }

    println!("Exported SQLite database to {path}");
    Ok(())
}

fn sanitize_command_rows(rows: &mut [CommandRow], config: &AppConfig) -> Result<()> {
    for row in rows.iter_mut() {
        row.command = security::redact_for_audit(&row.command, config)?;
        if row.command.contains("****") {
            row.is_masked = true;
        }
    }
    Ok(())
}

fn sanitize_commands_table(connection: &Connection, config: &AppConfig) -> Result<()> {
    connection
        .execute("BEGIN IMMEDIATE", [])
        .context("failed to begin commands sanitization transaction")?;

    let result = (|| -> Result<()> {
        let mut statement =
            connection.prepare("SELECT id, command FROM commands WHERE command IS NOT NULL")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (id, command) in rows {
            let redacted = security::redact_for_audit(&command, config)?;
            let masked = redacted.contains("****");
            connection.execute(
                "UPDATE commands SET command = ?1, is_masked = CASE WHEN ?2 THEN 1 ELSE is_masked END WHERE id = ?3",
                params![redacted, masked as i32, id],
            )?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            connection
                .execute("COMMIT", [])
                .context("failed to commit commands sanitization transaction")?;
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK", []);
            return Err(error);
        }
    }
    Ok(())
}

fn sanitize_audit_log(connection: &Connection, config: &AppConfig) -> Result<()> {
    connection
        .execute("BEGIN IMMEDIATE", [])
        .context("failed to begin audit sanitization transaction")?;

    let result = (|| -> Result<()> {
        let mut statement = connection.prepare(
            "SELECT id, event_type, raw_command, reason, username, hostname, created_at
             FROM audit_log ORDER BY id ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut prev_hash = String::new();
        for (id, event_type, raw_command, reason, username, hostname, created_at) in rows {
            let redacted = match raw_command.as_deref() {
                Some(command) => Some(security::redact_for_audit(command, config)?),
                None => None,
            };
            let entry_hash = crate::audit_chain::compute_entry_hash(
                &prev_hash,
                &event_type,
                redacted.as_deref(),
                reason.as_deref(),
                username.as_deref(),
                hostname.as_deref(),
                &created_at,
            );
            connection.execute(
                "UPDATE audit_log SET raw_command = ?1, prev_hash = ?2, entry_hash = ?3 WHERE id = ?4",
                params![redacted, prev_hash, entry_hash, id],
            )?;
            prev_hash = entry_hash;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            connection
                .execute("COMMIT", [])
                .context("failed to commit audit sanitization transaction")?;
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK", []);
            return Err(error);
        }
    }
    Ok(())
}

fn write_text(path: &str, content: &str) -> Result<()> {
    write_bytes(path, content.as_bytes())
}

fn write_bytes(path: &str, content: &[u8]) -> Result<()> {
    config::write_private_file(Path::new(path), content)
        .with_context(|| format!("failed to write export file {path}"))
}
