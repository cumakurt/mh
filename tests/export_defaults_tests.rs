use clap::Parser;
use mh::cli::{Cli, Command};

#[test]
fn export_sanitizes_by_default() {
    let cli = Cli::try_parse_from(["mh", "export", "--json", "out.json"]).expect("parse");
    let Command::Export(args) = cli.command else {
        panic!("expected export command");
    };
    assert!(args.sanitize);
    assert!(!args.include_secrets);
}

#[test]
fn export_include_secrets_disables_default_redaction() {
    let cli = Cli::try_parse_from(["mh", "export", "--json", "out.json", "--include-secrets"])
        .expect("parse");
    let Command::Export(args) = cli.command else {
        panic!("expected export command");
    };
    assert!(args.include_secrets);
    let sanitize_exports = !args.include_secrets && args.sanitize;
    assert!(!sanitize_exports);
}
