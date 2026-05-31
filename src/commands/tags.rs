use anyhow::Result;

use crate::cli::{TagsArgs, TagsCommand};
use crate::config::AppConfig;
use crate::db::Database;

pub fn run(args: TagsArgs) -> Result<()> {
    match args.command {
        TagsCommand::List => list(),
    }
}

fn list() -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let tags = database.list_tags()?;

    if tags.is_empty() {
        println!("No tags found");
        return Ok(());
    }

    for tag in tags {
        println!("{}\t{}", tag.count, tag.tag);
    }

    Ok(())
}
