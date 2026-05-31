use anyhow::{Result, bail};

use crate::cli::StatsArgs;
use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{StatEntry, StatsPeriod};
use crate::output::styling::Styler;

pub fn run(args: StatsArgs) -> Result<()> {
    let period = period_from_args(&args)?;
    let config = AppConfig::load()?;
    let styler = Styler::from_config(&config);
    let database = Database::open(&config)?;
    let summary = database.stats_summary(period, args.top)?;

    println!(
        "{}",
        styler.label_value("Period", period_label(summary.period))
    );
    println!(
        "{}",
        styler.label_value("Total commands", summary.total_commands.to_string())
    );
    println!(
        "{}",
        styler.label_value(
            "Successful commands",
            styler.success(summary.successful_commands.to_string())
        )
    );
    println!(
        "{}",
        styler.label_value(
            "Failed commands",
            if summary.failed_commands > 0 {
                styler.warning(summary.failed_commands.to_string())
            } else {
                styler.muted(summary.failed_commands.to_string())
            }
        )
    );
    println!(
        "{}",
        styler.label_value(
            "Error rate",
            styler.error_rate_text(summary.total_commands, summary.failed_commands)
        )
    );
    println!(
        "{}",
        styler.label_value(
            "Average duration",
            summary
                .average_duration_ms
                .map(|value| format!("{value:.1} ms"))
                .unwrap_or_else(|| styler.muted("-"))
        )
    );
    println!(
        "{}",
        styler.label_value(
            "Longest duration",
            summary
                .longest_duration_ms
                .map(|value| format!("{value} ms"))
                .unwrap_or_else(|| styler.muted("-"))
        )
    );
    if let Some(peak_hour) = summary.peak_hour {
        println!("{}", styler.label_value("Peak hour", peak_hour));
    }

    print_entries(&styler, "Top commands", &summary.top_commands);
    print_entries(&styler, "Top directories", &summary.top_directories);
    print_entries(
        &styler,
        "Error-prone commands",
        &summary.error_prone_commands,
    );
    print_entries(&styler, "Shells", &summary.shell_counts);
    if args.category || !summary.category_counts.is_empty() {
        print_entries(&styler, "Categories", &summary.category_counts);
    }
    if args.heatmap {
        let heatmap = database.hourly_activity(period)?;
        print_heatmap(&styler, &heatmap);
    }

    Ok(())
}

fn period_from_args(args: &StatsArgs) -> Result<StatsPeriod> {
    let selected = [args.today, args.week, args.month]
        .into_iter()
        .filter(|selected| *selected)
        .count();
    if selected > 1 {
        bail!("only one of --today, --week, or --month can be used");
    }

    if args.today {
        Ok(StatsPeriod::Today)
    } else if args.week {
        Ok(StatsPeriod::Week)
    } else if args.month {
        Ok(StatsPeriod::Month)
    } else {
        Ok(StatsPeriod::All)
    }
}

fn period_label(period: StatsPeriod) -> &'static str {
    match period {
        StatsPeriod::All => "all time",
        StatsPeriod::Today => "today",
        StatsPeriod::Week => "last 7 days",
        StatsPeriod::Month => "last 30 days",
    }
}

fn print_entries(styler: &Styler, title: &str, entries: &[StatEntry]) {
    println!();
    println!("{}", styler.section_title(title));
    if entries.is_empty() {
        println!("  {}", styler.muted("-"));
        return;
    }

    for entry in entries {
        let count = if entry.count > 0 {
            styler.accent(entry.count.to_string())
        } else {
            styler.muted(entry.count.to_string())
        };
        println!("  {count:>6}  {}", entry.label);
    }
}

fn print_heatmap(styler: &Styler, entries: &[StatEntry]) {
    println!();
    println!("{}", styler.section_title("Hourly activity"));
    let max_count = entries.iter().map(|entry| entry.count).max().unwrap_or(0);
    for hour in 0..24 {
        let label = format!("{hour:02}");
        let count = entries
            .iter()
            .find(|entry| entry.label == label)
            .map(|entry| entry.count)
            .unwrap_or(0);
        let bar_len = if max_count == 0 {
            0
        } else {
            ((count * 20) / max_count).max(i64::from(count > 0)) as usize
        };
        let count_text = if count > 0 {
            styler.accent(format!("{count:>6}"))
        } else {
            styler.muted(format!("{count:>6}"))
        };
        println!(
            "{}:00 {} {}",
            label,
            count_text,
            styler.heatmap_bar(count, bar_len)
        );
    }
}
