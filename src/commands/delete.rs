use std::io::{self, IsTerminal, Write};

use anyhow::{Result, bail};
use chrono::{Duration, Utc};

use crate::cli::DeleteArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::SearchFilters;
use crate::output::styling::Styler;

pub fn run(args: DeleteArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;

    let ids = if let Some(id) = args.id {
        database.get_command(id)?;
        vec![id]
    } else {
        if args.older_than.is_none()
            && args.contains.is_none()
            && !args.failed
            && args.tag.is_none()
        {
            bail!("provide an id or at least one delete filter");
        }

        let before = args
            .older_than
            .as_deref()
            .map(parse_age_to_rfc3339)
            .transpose()?;
        let rows = database.search_commands(&SearchFilters {
            query: args.contains,
            cwd: None,
            failed: args.failed,
            success: false,
            user: None,
            shell: None,
            after: None,
            before,
            regex: false,
            fuzzy: false,
            fts: false,
            tag: args.tag,
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
        rows.into_iter().map(|row| row.id).collect()
    };

    if ids.is_empty() {
        println!("{}", styler.warning("No matching commands found"));
        return Ok(());
    }

    if !args.yes && !confirm(&styler, &format!("Delete {} command record(s)?", ids.len()))? {
        println!("{}", styler.warning("Delete cancelled"));
        return Ok(());
    }

    let deleted = database.delete_command_ids(&ids)?;
    println!(
        "{}",
        styler.success(format!("Deleted {deleted} command record(s)"))
    );
    Ok(())
}

fn parse_age_to_rfc3339(value: &str) -> Result<String> {
    let Some(unit) = value.chars().last() else {
        bail!("age must use a suffix such as 90d, 12h, or 30m");
    };
    let number = &value[..value.len().saturating_sub(1)];
    let amount: i64 = number.parse()?;
    let duration = match unit {
        'd' => Duration::days(amount),
        'h' => Duration::hours(amount),
        'm' => Duration::minutes(amount),
        _ => bail!("age suffix must be d, h, or m"),
    };
    Ok((Utc::now() - duration).to_rfc3339())
}

fn confirm(styler: &Styler, prompt: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }

    eprint!("{} [y/N] ", styler.warning(prompt));
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}
