use clap::Parser;
use mh::cli::Cli;

#[test]
fn parses_top_level_help_command() {
    let cli = Cli::try_parse_from(["mh", "doctor"]).expect("doctor should parse");
    assert!(matches!(cli.command, mh::cli::Command::Doctor(_)));
}

#[test]
fn parses_doctor_strict_flag() {
    let cli = Cli::try_parse_from(["mh", "doctor", "--strict"]).expect("doctor strict");
    if let mh::cli::Command::Doctor(args) = cli.command {
        assert!(args.strict);
    } else {
        panic!("expected doctor command");
    }
}

#[test]
fn parses_search_with_new_filters() {
    let cli = Cli::try_parse_from([
        "mh",
        "search",
        "docker",
        "--hostname",
        "kali",
        "--ssh",
        "--csv",
    ])
    .expect("search should parse");

    if let mh::cli::Command::Search(args) = cli.command {
        assert_eq!(args.query.as_deref(), Some("docker"));
        assert_eq!(args.hostname.as_deref(), Some("kali"));
        assert!(args.ssh);
        assert!(args.csv);
    } else {
        panic!("expected search command");
    }
}

#[test]
fn parses_daemon_install() {
    let cli = Cli::try_parse_from(["mh", "daemon", "install"]).expect("daemon install should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Daemon(mh::cli::DaemonArgs {
            action: mh::cli::DaemonAction::Install,
        })
    ));
}

#[test]
fn parses_daemon_status() {
    let cli = Cli::try_parse_from(["mh", "daemon", "status"]).expect("daemon should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Daemon(mh::cli::DaemonArgs {
            action: mh::cli::DaemonAction::Status,
        })
    ));
}

#[test]
fn parses_record_no_daemon_flag() {
    let cli = Cli::try_parse_from([
        "mh",
        "record",
        "--no-daemon",
        "--command",
        "ls",
    ])
    .expect("record should parse");

    if let mh::cli::Command::Record(args) = cli.command {
        assert!(args.no_daemon);
        assert_eq!(args.command, "ls");
    } else {
        panic!("expected record command");
    }
}

#[test]
fn parses_export_sqlite_flag() {
    let cli = Cli::try_parse_from(["mh", "export", "--sqlite", "backup.db"])
        .expect("export should parse");

    if let mh::cli::Command::Export(args) = cli.command {
        assert_eq!(args.sqlite.as_deref(), Some("backup.db"));
    } else {
        panic!("expected export command");
    }
}
