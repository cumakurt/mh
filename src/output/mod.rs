pub mod csv;
pub mod json;
pub mod markdown;
pub mod styling;
pub mod table;

use anyhow::Result;

use crate::models::CommandRow;

pub fn print_rows(rows: &[CommandRow], json: bool, plain: bool) -> Result<()> {
    print_rows_with_formats(rows, json, plain, false, false)
}

pub fn print_rows_with_formats(
    rows: &[CommandRow],
    json: bool,
    plain: bool,
    csv_output: bool,
    markdown_output: bool,
) -> Result<()> {
    if json {
        json::print(rows)
    } else if csv_output {
        print!("{}", csv::format_rows(rows));
        Ok(())
    } else if markdown_output {
        print!("{}", markdown::format_rows(rows));
        Ok(())
    } else if plain {
        for row in rows {
            println!("{}", row.command);
        }
        Ok(())
    } else {
        table::print(rows)
    }
}
