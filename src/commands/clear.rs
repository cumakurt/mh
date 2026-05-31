use std::io::{self, IsTerminal, Write};

use anyhow::Result;

use crate::cli::ClearArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::output::styling::Styler;

pub fn run(args: ClearArgs) -> Result<()> {
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;

    if !confirm(&styler, "Clear matching command history?")? {
        println!("{}", styler.warning("Clear cancelled"));
        return Ok(());
    }

    let deleted = database.clear_history(
        args.user.as_deref(),
        args.before.as_deref(),
        args.keep_pinned,
    )?;
    println!(
        "{}",
        styler.success(format!("Cleared {deleted} command record(s)"))
    );
    Ok(())
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
