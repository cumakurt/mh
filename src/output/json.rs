use anyhow::Result;

use crate::models::CommandRow;

pub fn print(rows: &[CommandRow]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(rows)?);
    Ok(())
}
