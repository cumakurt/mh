use anyhow::{Result, bail};

use crate::cli::{SyncArgs, SyncCommand};
use crate::config::{AppConfig, config_path};
use crate::db::Database;

pub fn run(args: SyncArgs) -> Result<()> {
    match args.command {
        SyncCommand::Status => status(),
        SyncCommand::Setup { url, token } => setup(url, token),
        SyncCommand::Push => push(),
        SyncCommand::Pull => pull(),
        SyncCommand::Enable => enable(),
        SyncCommand::Disable => disable(),
    }
}

fn status() -> Result<()> {
    let config = AppConfig::load()?;
    println!(
        "Sync: {}",
        if config.sync.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("Server URL: {}", empty_placeholder(&config.sync.server_url));
    println!(
        "Token configured: {}",
        if config.sync.token.is_empty() {
            "no"
        } else {
            "yes"
        }
    );
    println!(
        "Auto sync interval: {} minute(s)",
        config.sync.auto_sync_interval_minutes
    );
    println!("Device ID: {}", config.sync.device_id);
    println!(
        "Last synced at: {}",
        empty_placeholder(&config.sync.last_synced_at)
    );
    #[cfg(feature = "sync")]
    println!("Remote sync client: enabled");
    #[cfg(not(feature = "sync"))]
    println!("Remote sync client: disabled (rebuild with --features sync)");
    println!("Config path: {}", config_path().display());
    Ok(())
}

fn setup(url: String, token: String) -> Result<()> {
    if url.trim().is_empty() {
        bail!("sync server URL must not be empty");
    }
    if token.trim().is_empty() {
        bail!("sync token must not be empty");
    }

    let path = config_path();
    let mut config = AppConfig::load()?;
    config.sync.server_url = url.trim().to_string();
    config.sync.token = token;
    config.write_to_path(&path)?;
    println!("Sync configuration saved");
    println!("Run 'mh sync enable' to enable automatic sync state");
    Ok(())
}

fn enable() -> Result<()> {
    let path = config_path();
    let mut config = AppConfig::load()?;
    ensure_sync_configured(&config)?;
    config.sync.enabled = true;
    config.write_to_path(&path)?;
    println!("Sync enabled");
    Ok(())
}

fn disable() -> Result<()> {
    let path = config_path();
    let mut config = AppConfig::load()?;
    config.sync.enabled = false;
    config.write_to_path(&path)?;
    println!("Sync disabled");
    Ok(())
}

fn push() -> Result<()> {
    let config = AppConfig::load()?;
    ensure_sync_ready(&config)?;
    let database = Database::open(&config)?;
    sync_push(&config, &database)
}

fn pull() -> Result<()> {
    let config = AppConfig::load()?;
    ensure_sync_ready(&config)?;
    let database = Database::open(&config)?;
    sync_pull(&config, &database)
}

#[cfg(feature = "sync")]
fn sync_push(config: &AppConfig, database: &Database) -> Result<()> {
    crate::sync::client::push(config, database)
}

#[cfg(not(feature = "sync"))]
fn sync_push(_config: &AppConfig, _database: &Database) -> Result<()> {
    bail!("sync push requires building mh with --features sync")
}

#[cfg(feature = "sync")]
fn sync_pull(config: &AppConfig, database: &Database) -> Result<()> {
    crate::sync::client::pull(config, database)
}

#[cfg(not(feature = "sync"))]
fn sync_pull(_config: &AppConfig, _database: &Database) -> Result<()> {
    bail!("sync pull requires building mh with --features sync")
}

fn ensure_sync_ready(config: &AppConfig) -> Result<()> {
    if !config.sync.enabled {
        bail!("sync is disabled; run 'mh sync enable' first");
    }
    ensure_sync_configured(config)
}

fn ensure_sync_configured(config: &AppConfig) -> Result<()> {
    if config.sync.server_url.trim().is_empty() {
        bail!("sync server URL is not configured; run 'mh sync setup <url> <token>' first");
    }
    if config.sync.token.trim().is_empty() {
        bail!("sync token is not configured; run 'mh sync setup <url> <token>' first");
    }
    Ok(())
}

fn empty_placeholder(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}
