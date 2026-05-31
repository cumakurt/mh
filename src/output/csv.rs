use crate::models::CommandRow;

pub fn format_rows(rows: &[CommandRow]) -> String {
    let mut output =
        String::from("id,started_at,exit_code,duration_ms,cwd,shell,category,command,tags\n");
    for row in rows {
        output.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            row.id,
            csv_escape(&row.started_at),
            row.exit_code
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.duration_ms
                .map(|value| value.to_string())
                .unwrap_or_default(),
            csv_escape(row.cwd.as_deref().unwrap_or_default()),
            csv_escape(row.shell.as_deref().unwrap_or_default()),
            csv_escape(row.category.as_deref().unwrap_or_default()),
            csv_escape(&row.command),
            csv_escape(&row.tags.join(",")),
        ));
    }
    output
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
