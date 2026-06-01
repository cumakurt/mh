use clap::Parser;
use mh::cli::Cli;
use mh::commands::doctor;

mod common;
use common::IsolatedConfigHome;

#[test]
fn parses_doctor_json_flag() {
    let cli = Cli::try_parse_from(["mh", "doctor", "--json"]).expect("doctor json should parse");
    let mh::cli::Command::Doctor(args) = cli.command else {
        panic!("expected doctor command");
    };
    assert!(args.json);
}

#[test]
fn doctor_json_builds_structured_report() {
    let _guard = IsolatedConfigHome::new();

    doctor::run(mh::cli::DoctorArgs {
        strict: false,
        json: true,
    })
    .expect("doctor json run");

    let report = doctor::current_report();
    assert!(
        ["ok", "warn", "error"].contains(&report.status.as_str()),
        "unexpected status: {}",
        report.status
    );
    assert!(!report.summary.mh_version.is_empty());
    assert!(!report.checks.is_empty());
    for check in &report.checks {
        assert!(!check.code.is_empty());
        assert!(!check.level.is_empty());
        assert!(!check.message.is_empty());
    }
    let encoded = serde_json::to_string(&report).expect("encode report");
    let round_trip: doctor::DoctorReport = serde_json::from_str(&encoded).expect("decode report");
    assert_eq!(round_trip.status, report.status);
    assert_eq!(round_trip.warning_count, report.warning_count);
}
