use clap::Parser;
use mh::cli::Cli;

#[test]
fn parses_risk_list_command() {
    let cli = Cli::try_parse_from(["mh", "risk", "list"]).expect("risk list should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Risk(mh::cli::RiskArgs {
            command: mh::cli::RiskCommand::List,
        })
    ));
}

#[test]
fn parses_risk_check_command() {
    let cli = Cli::try_parse_from(["mh", "risk", "check", "rm -rf /", "--json"])
        .expect("risk check should parse");

    if let mh::cli::Command::Risk(mh::cli::RiskArgs {
        command: mh::cli::RiskCommand::Check { command, json },
    }) = cli.command
    {
        assert_eq!(command, "rm -rf /");
        assert!(json);
    } else {
        panic!("expected risk check command");
    }
}

#[test]
fn parses_risk_scan_command() {
    let cli = Cli::try_parse_from(["mh", "risk", "scan", "--critical", "--today", "-n", "50"])
        .expect("risk scan should parse");

    if let mh::cli::Command::Risk(mh::cli::RiskArgs {
        command: mh::cli::RiskCommand::Scan(args),
    }) = cli.command
    {
        assert!(args.critical);
        assert!(args.today);
        assert_eq!(args.limit, Some(50));
    } else {
        panic!("expected risk scan command");
    }
}

#[test]
fn assesses_dangerous_command() {
    let assessment = mh::risk::assess_command("rm -rf /").expect("command should be flagged");
    assert_eq!(assessment.level, mh::risk::RiskLevel::Critical);
}

#[test]
fn ignores_safe_command() {
    assert!(mh::risk::assess_command("ls -la").is_none());
}
