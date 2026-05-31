use clap::Parser;
use mh::cli::Cli;
use std::process::Command as ProcessCommand;

#[test]
fn parses_context_command() {
    let cli = Cli::try_parse_from(["mh", "context"]).expect("context should parse");
    assert!(matches!(
        cli.command,
        mh::cli::Command::Context(mh::cli::ContextArgs { command: None })
    ));
}

#[test]
fn parses_context_history_command() {
    let cli = Cli::try_parse_from([
        "mh",
        "context",
        "history",
        "--repo",
        "/tmp/repo",
        "--branch",
        "main",
        "--commit",
        "abc1234",
    ])
    .expect("context history should parse");

    if let mh::cli::Command::Context(mh::cli::ContextArgs {
        command: Some(mh::cli::ContextCommand::History(args)),
    }) = cli.command
    {
        assert_eq!(args.repo.as_deref(), Some("/tmp/repo"));
        assert_eq!(args.branch.as_deref(), Some("main"));
        assert_eq!(args.commit.as_deref(), Some("abc1234"));
    } else {
        panic!("expected context history command");
    }
}

#[test]
fn detects_git_context_in_repository() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let repo_path = temp_dir.path();

    for (command, args) in [
        ("git", vec!["init", "-b", "main"]),
        ("git", vec!["config", "user.email", "test@example.com"]),
        ("git", vec!["config", "user.name", "test"]),
        ("git", vec!["commit", "--allow-empty", "-m", "init"]),
    ] {
        let status = ProcessCommand::new(command)
            .args(&args)
            .current_dir(repo_path)
            .status()
            .expect("git command should run");
        assert!(status.success(), "git command failed: {command} {args:?}");
    }

    let context = mh::git_detect::detect_git_context(&repo_path.to_string_lossy())
        .expect("git context should be detected");
    assert_eq!(context.branch.as_deref(), Some("main"));
    assert!(context.commit.is_some());
}
