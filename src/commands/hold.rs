use anyhow::{Result, bail};

use crate::cli::{HoldAddArgs, HoldArgs, HoldCommand};
use crate::config::AppConfig;
use crate::db::Database;
use crate::output::styling::Styler;

pub fn run(args: HoldArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;
    let username = Some(whoami::username());
    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());

    match args.command {
        HoldCommand::Add(add_args) => run_add(&database, &styler, add_args)?,
        HoldCommand::List => run_list(&database, &styler)?,
        HoldCommand::Remove { id } => {
            if database.remove_legal_hold(id)? {
                println!("{}", styler.success(format!("Removed legal hold {id}")));
            } else {
                bail!("legal hold {id} not found");
            }
        }
        HoldCommand::Purge { dry_run } => run_purge(
            &config,
            &database,
            &styler,
            dry_run,
            username.as_deref(),
            hostname.as_deref(),
        )?,
    }

    Ok(())
}

fn run_add(database: &Database, styler: &Styler, args: HoldAddArgs) -> Result<()> {
    if args.session.is_none()
        && args.command.is_none()
        && args.tag.is_none()
        && args.git_repo.is_none()
    {
        bail!("at least one of --session, --command, --tag, or --git-repo is required");
    }

    let hold_id = database.add_legal_hold(
        &args.label,
        args.session.as_deref(),
        args.command,
        args.tag.as_deref(),
        args.git_repo.as_deref(),
        args.reason.as_deref(),
    )?;
    println!(
        "{}",
        styler.success(format!("Created legal hold {hold_id}: {}", args.label))
    );
    Ok(())
}

fn run_list(database: &Database, _styler: &Styler) -> Result<()> {
    let holds = database.list_legal_holds()?;
    if holds.is_empty() {
        println!("No legal holds configured");
        return Ok(());
    }

    for hold in holds {
        println!(
            "{} {} session={} command={:?} tag={:?} repo={:?}",
            hold.id,
            hold.label,
            hold.session_id.unwrap_or_else(|| "-".to_string()),
            hold.command_id,
            hold.tag,
            hold.git_repo,
        );
    }
    Ok(())
}

fn run_purge(
    config: &AppConfig,
    database: &Database,
    styler: &Styler,
    dry_run: bool,
    username: Option<&str>,
    hostname: Option<&str>,
) -> Result<()> {
    if !config.retention.enabled && !dry_run {
        bail!("retention policy is disabled in config; set retention.enabled = true");
    }

    if dry_run {
        let cutoff =
            chrono::Utc::now() - chrono::Duration::days(config.retention.retention_days as i64);
        println!(
            "Dry run: would purge commands older than {} days (before {})",
            config.retention.retention_days,
            cutoff.to_rfc3339()
        );
        return Ok(());
    }

    let deleted = database.retention_purge(
        config.retention.retention_days,
        config.retention.respect_legal_hold,
    )?;
    database.insert_purge_audit(
        "retention_purge",
        Some(&format!("{}d", config.retention.retention_days)),
        deleted,
        username,
        hostname,
    )?;
    if config.security.audit_log {
        let _ = database.insert_audit_log(
            "purge",
            "",
            &format!("retention purge deleted {deleted} commands"),
            username,
            hostname,
        );
    }
    println!(
        "{}",
        styler.success(format!("Retention purge deleted {deleted} command(s)"))
    );
    Ok(())
}
