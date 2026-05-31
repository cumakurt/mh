use crate::models::CommandRow;

pub fn format_rows(rows: &[CommandRow]) -> String {
    let mut output =
        String::from("| ID | Time | Exit | CWD | Command |\n|---:|---|---:|---|---|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | {} | {} | {} | `{}` |\n",
            row.id,
            row.started_at,
            row.exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            markdown_escape(row.cwd.as_deref().unwrap_or("-")),
            markdown_escape(&row.command),
        ));
    }
    output
}

fn markdown_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
}
