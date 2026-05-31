use anyhow::{Result, bail};

use crate::cli::{TagArgs, UntagArgs};
use crate::config::AppConfig;
use crate::db::Database;

pub fn run(args: TagArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;

    let (command_ids, tags) = if let Some(last) = args.last {
        (database.latest_command_ids(last)?, args.args)
    } else if let Some(command_id) = args.args.first() {
        let command_id = command_id
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("command id must be an integer"))?;
        (vec![command_id], args.args.into_iter().skip(1).collect())
    } else {
        bail!("provide a command id or use --last");
    };

    let inserted = database.add_tags(&command_ids, &tags)?;
    println!(
        "Added {inserted} tag assignment{}",
        if inserted == 1 { "" } else { "s" }
    );
    Ok(())
}

pub fn run_untag(args: UntagArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let removed = database.remove_tags(args.command_id, &args.tags)?;
    println!(
        "Removed {removed} tag assignment{}",
        if removed == 1 { "" } else { "s" }
    );
    Ok(())
}
