use clap::Parser;
use mh::cli::Cli;

#[test]
fn parses_top_level_help_command() {
    let cli = Cli::try_parse_from(["mh", "doctor"]).expect("doctor should parse");
    assert!(matches!(cli.command, mh::cli::Command::Doctor(_)));
}

#[test]
fn parses_doctor_json_flag() {
    let cli = Cli::try_parse_from(["mh", "doctor", "--json"]).expect("doctor json");
    let mh::cli::Command::Doctor(args) = cli.command else {
        panic!("expected doctor command");
    };
    assert!(args.json);
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
fn parses_semantic_search_alias() {
    let cli = Cli::try_parse_from(["mh", "search", "--nl", "prod failed deploy"])
        .expect("semantic search should parse");
    let mh::cli::Command::Search(args) = cli.command else {
        panic!("expected search command");
    };
    assert!(args.semantic);
}

#[test]
fn parses_replay_risk_preview_flags() {
    let cli = Cli::try_parse_from([
        "mh",
        "replay",
        "42",
        "--yes",
        "--risk-preview",
        "--no-risk-guidance",
    ])
    .expect("replay should parse");
    let mh::cli::Command::Replay(args) = cli.command else {
        panic!("expected replay command");
    };
    assert!(args.risk_preview);
    assert!(args.no_risk_guidance);
}

#[test]
fn parses_policy_pack_export() {
    let cli = Cli::try_parse_from([
        "mh",
        "policy",
        "pack",
        "export",
        "policy.json",
        "--key",
        "secret",
    ])
    .expect("policy pack should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Policy(mh::cli::PolicyArgs {
            command: mh::cli::PolicyCommand::Pack(_),
        })
    ));
}

#[test]
fn parses_legacy_policy_check_command_flag() {
    let cli = Cli::try_parse_from([
        "mh",
        "policy",
        "check",
        "--command",
        "echo ok",
        "--cwd",
        "/tmp",
        "--quiet",
    ])
    .expect("legacy policy check should parse");

    let mh::cli::Command::Policy(mh::cli::PolicyArgs {
        command:
            mh::cli::PolicyCommand::Check {
                command,
                command_arg,
                quiet,
                ..
            },
    }) = cli.command
    else {
        panic!("expected policy check command");
    };
    assert_eq!(command, None);
    assert_eq!(command_arg.as_deref(), Some("echo ok"));
    assert!(quiet);
}

#[test]
fn parses_incident_export() {
    let cli = Cli::try_parse_from([
        "mh",
        "incident",
        "export",
        "--session",
        "s1",
        "--output",
        "incident.json",
    ])
    .expect("incident export should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Incident(mh::cli::IncidentArgs {
            command: mh::cli::IncidentCommand::Export { .. },
        })
    ));
}

#[test]
fn parses_tui_dashboard() {
    let cli = Cli::try_parse_from(["mh", "tui", "--dashboard"]).expect("tui dashboard");
    let mh::cli::Command::Tui(args) = cli.command else {
        panic!("expected tui command");
    };
    assert!(args.dashboard);
}

#[test]
fn parses_daemon_install() {
    let cli =
        Cli::try_parse_from(["mh", "daemon", "install"]).expect("daemon install should parse");
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
    let cli = Cli::try_parse_from(["mh", "record", "--no-daemon", "--command", "ls"])
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
