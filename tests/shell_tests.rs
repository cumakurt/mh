use mh::cli::ShellKind;
use mh::shell;

#[test]
fn bash_integration_sets_git_detect_skip_default() {
    let integration = shell::integration(ShellKind::Bash);
    assert!(integration.contains("MH_SKIP_GIT_DETECT:=1"));
}

#[test]
fn zsh_integration_sets_git_detect_skip_default() {
    let integration = shell::integration(ShellKind::Zsh);
    assert!(integration.contains("MH_SKIP_GIT_DETECT:=1"));
}

#[test]
fn fish_integration_sets_git_detect_skip_default() {
    let integration = shell::integration(ShellKind::Fish);
    assert!(integration.contains("MH_SKIP_GIT_DETECT"));
}

#[test]
fn nushell_integration_sets_git_detect_skip_default() {
    let integration = shell::integration(ShellKind::Nushell);
    assert!(integration.contains("MH_SKIP_GIT_DETECT"));
}

#[test]
fn zsh_integration_binds_up_arrow_to_picker() {
    let integration = shell::integration(ShellKind::Zsh);

    assert!(integration.contains("mh pick"));
    assert!(integration.contains("bindkey '^[[A' _mh_history_picker"));
    assert!(integration.contains("BUFFER=\"$selected\""));
}

#[test]
fn bash_integration_binds_up_arrow_to_picker() {
    let integration = shell::integration(ShellKind::Bash);

    assert!(integration.contains("mh pick"));
    assert!(integration.contains("bind -x"));
    assert!(integration.contains("READLINE_LINE=\"$selected\""));
}

#[test]
fn fish_integration_binds_up_arrow_to_picker() {
    let integration = shell::integration(ShellKind::Fish);

    assert!(integration.contains("mh pick"));
    assert!(integration.contains("bind \\e\\[A mh_history_picker"));
    assert!(integration.contains("commandline --replace \"$selected\""));
}

#[test]
fn nushell_integration_records_exit_code_and_preserves_hooks() {
    let integration = shell::integration(ShellKind::Nushell);

    assert!(integration.contains("--exit-code"));
    assert!(integration.contains("LAST_EXIT_CODE"));
    assert!(integration.contains("^mh record"));
    assert!(integration.contains("mh_existing_hooks"));
    assert!(integration.contains("MH_PICK_LIMIT"));
}

#[test]
fn bash_integration_skips_internal_unset_commands() {
    let integration = shell::integration(ShellKind::Bash);
    assert!(integration.contains("unset\\ *"));
}

#[test]
fn zsh_integration_skips_internal_commands() {
    let integration = shell::integration(ShellKind::Zsh);
    assert!(integration.contains("trap\\ *"));
    assert!(integration.contains("export\\ MH_*"));
}

#[test]
fn fish_integration_guards_missing_start_time() {
    let integration = shell::integration(ShellKind::Fish);
    assert!(integration.contains("if set -q MH_START_TIME"));
}

#[test]
fn bash_integration_uses_portable_millisecond_clock() {
    let integration = shell::integration(ShellKind::Bash);
    assert!(integration.contains("__mh_now_ms"));
    assert!(integration.contains("python3"));
}

#[test]
fn zsh_integration_loads_datetime_module() {
    let integration = shell::integration(ShellKind::Zsh);
    assert!(integration.contains("zmodload zsh/datetime"));
    assert!(integration.contains("__mh_now_ms"));
}

#[test]
fn fish_integration_uses_portable_millisecond_clock() {
    let integration = shell::integration(ShellKind::Fish);
    assert!(integration.contains("function __mh_now_ms"));
}

#[test]
fn bash_integration_supports_verbose_record_mode() {
    let integration = shell::integration(ShellKind::Bash);
    assert!(integration.contains("__mh_record"));
    assert!(integration.contains("MH_RECORD_VERBOSE"));
}

#[test]
fn zsh_integration_supports_verbose_record_mode() {
    let integration = shell::integration(ShellKind::Zsh);
    assert!(integration.contains("__mh_record"));
    assert!(integration.contains("MH_RECORD_VERBOSE"));
}

#[test]
fn fish_integration_supports_verbose_record_mode() {
    let integration = shell::integration(ShellKind::Fish);
    assert!(integration.contains("function __mh_record"));
    assert!(integration.contains("MH_RECORD_VERBOSE"));
}

#[test]
fn nushell_integration_supports_verbose_record_mode() {
    let integration = shell::integration(ShellKind::Nushell);
    assert!(integration.contains("MH_RECORD_VERBOSE"));
}

#[test]
fn fish_integration_skips_internal_commands() {
    let integration = shell::integration(ShellKind::Fish);
    assert!(integration.contains("'function *'"));
    assert!(integration.contains("'local *'"));
    assert!(integration.contains("'export MH_*'"));
}

#[test]
fn bash_integration_supports_policy_verbose_mode() {
    let integration = shell::integration(ShellKind::Bash);
    assert!(integration.contains("MH_POLICY_VERBOSE"));
}
