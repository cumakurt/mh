use std::io::Write;

use clap::Parser;
use mh::cli::{Cli, InitArgs, ShellKind};
use mh::commands::init;

mod common;
use common::IsolatedHome;

#[test]
fn install_is_idempotent_when_managed_block_present() {
    let home = IsolatedHome::new();
    let zshrc = home.path().join(".zshrc");
    let mut file = std::fs::File::create(&zshrc).expect("create zshrc");
    writeln!(file, "# >>> mh shell integration >>>").expect("write marker");
    writeln!(file, "eval \"$(mh init zsh)\"").expect("write hook");
    writeln!(file, "# <<< mh shell integration <<<").expect("write marker");
    drop(file);

    init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: true,
        repair: false,
    })
    .expect("repeat install should succeed when integration is already present");

    let content = std::fs::read_to_string(&zshrc).expect("read zshrc");
    assert_eq!(content.matches("# >>> mh shell integration >>>").count(), 1);
}

#[test]
fn install_refuses_manual_hook_markers() {
    let home = IsolatedHome::new();
    let zshrc = home.path().join(".zshrc");
    std::fs::write(&zshrc, "function _mh_preexec() {}\n").expect("write zshrc");

    let error = init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: true,
        repair: false,
    })
    .expect_err("manual hook install should fail");

    assert!(
        error.to_string().contains("_mh_preexec"),
        "unexpected error: {error}"
    );
}

#[test]
fn install_writes_managed_block_into_new_zshrc() {
    let home = IsolatedHome::new();
    let zshrc = home.path().join(".zshrc");

    init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: true,
        repair: false,
    })
    .expect("install should succeed");

    let content = std::fs::read_to_string(&zshrc).expect("read zshrc");
    assert!(content.contains("# >>> mh shell integration >>>"));
    assert!(content.contains("mh init zsh"));
}

#[test]
#[cfg(unix)]
fn install_does_not_chmod_existing_home_directory() {
    use std::os::unix::fs::PermissionsExt;

    let home = IsolatedHome::new();
    std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o755))
        .expect("chmod home");

    init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: true,
        repair: false,
    })
    .expect("install should succeed");

    let mode = std::fs::metadata(home.path())
        .expect("home metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
}

#[test]
fn install_repairs_duplicate_managed_blocks() {
    let home = IsolatedHome::new();
    let zshrc = home.path().join(".zshrc");
    let content = "# shell\n# >>> mh shell integration >>>\neval \"$(mh init zsh)\"\n# <<< mh shell integration <<<\n# >>> mh shell integration >>>\neval \"$(mh init zsh)\"\n# <<< mh shell integration <<<\n";
    std::fs::write(&zshrc, content).expect("write zshrc");

    init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: true,
        repair: false,
    })
    .expect("install should repair duplicate managed blocks");

    let repaired = std::fs::read_to_string(&zshrc).expect("read zshrc");
    assert_eq!(
        repaired.matches("# >>> mh shell integration >>>").count(),
        1
    );
}

#[test]
fn repair_removes_duplicate_managed_blocks() {
    let home = IsolatedHome::new();
    let zshrc = home.path().join(".zshrc");
    let content = "# shell\n# >>> mh shell integration >>>\neval \"$(mh init zsh)\"\n# <<< mh shell integration <<<\n# >>> mh shell integration >>>\neval \"$(mh init zsh)\"\n# <<< mh shell integration <<<\n";
    std::fs::write(&zshrc, content).expect("write zshrc");

    init::run(InitArgs {
        shell: Some(ShellKind::Zsh),
        install: false,
        repair: true,
    })
    .expect("repair should succeed");

    let repaired = std::fs::read_to_string(&zshrc).expect("read zshrc");
    assert_eq!(
        repaired.matches("# >>> mh shell integration >>>").count(),
        1
    );
}

#[test]
fn parses_init_repair_flag() {
    let cli = Cli::try_parse_from(["mh", "init", "zsh", "--repair"]).expect("parse");
    match cli.command {
        mh::cli::Command::Init(args) => {
            assert!(args.repair);
            assert!(!args.install);
            assert_eq!(args.shell, Some(ShellKind::Zsh));
        }
        _ => panic!("expected init command"),
    }
}

#[test]
fn parses_init_without_shell() {
    let cli = Cli::try_parse_from(["mh", "init"]).expect("parse");
    match cli.command {
        mh::cli::Command::Init(args) => {
            assert_eq!(args.shell, None);
            assert!(!args.install);
            assert!(!args.repair);
        }
        _ => panic!("expected init command"),
    }
}
