use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::break_glass;
use crate::cli::{DoctorArgs, ShellKind};
use crate::config::{AppConfig, config_path, has_restricted_permissions, private_mode_path};
use crate::daemon::record_pid_path;
use crate::db::{Database, EXPECTED_SCHEMA_VERSION};
use crate::output::styling::{StatusLevel, Styler};
use crate::security;
use crate::shell::{config_candidates, hooks, resolve_config_path};

const BEGIN_MARKER: &str = hooks::BEGIN_MARKER;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub status: String,
    pub warning_count: usize,
    pub checks: Vec<DoctorCheck>,
    pub summary: DoctorSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub code: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorSummary {
    pub mh_version: String,
    pub config_path: Option<String>,
    pub database_path: Option<String>,
    pub schema_version: Option<i64>,
    pub command_count: Option<i64>,
    pub daemon_running: Option<bool>,
    pub private_mode: Option<bool>,
    pub strict: bool,
}

thread_local! {
    static WARNING_COUNT: RefCell<usize> = const { RefCell::new(0) };
    static JSON_MODE: RefCell<bool> = const { RefCell::new(false) };
    static CHECKS: RefCell<Vec<DoctorCheck>> = const { RefCell::new(Vec::new()) };
    static SUMMARY: RefCell<DoctorSummary> = RefCell::new(DoctorSummary::default());
}

pub fn run(args: DoctorArgs) -> Result<()> {
    reset_doctor_state(&args);
    let cfg_path = config_path();
    let styler = if args.json {
        Styler::from_display_config(false)
    } else {
        Styler::from_config(&AppConfig::default())
    };
    print_env_overrides(&styler);
    let config = match AppConfig::load() {
        Ok(config) => config,
        Err(error) => {
            say(
                &styler,
                StatusLevel::Warn,
                format!("Config could not be loaded: {error:#}"),
            );
            say(
                &styler,
                StatusLevel::Info,
                format!(
                    "Fix or remove {} then rerun {}",
                    cfg_path.display(),
                    if args.json {
                        "mh doctor".to_string()
                    } else {
                        styler.accent("mh doctor")
                    }
                ),
            );
            set_summary_config_path(&cfg_path);
            check_env_override_paths(&styler);
            return finish(&args);
        }
    };
    let styler = if args.json {
        Styler::from_display_config(false)
    } else {
        Styler::from_config(&config)
    };
    set_summary_config_path(&cfg_path);

    say(
        &styler,
        StatusLevel::Ok,
        format!("Config loaded from {}", cfg_path.display()),
    );
    check_env_override_paths(&styler);
    validate_config_engines(&styler, &config);

    let database = match Database::open(&config) {
        Ok(database) => database,
        Err(error) => {
            say(
                &styler,
                StatusLevel::Warn,
                format!("Database could not be opened: {error:#}"),
            );
            check_config_directory_permissions(&styler, &cfg_path);
            check_file_permissions(
                &styler,
                &cfg_path,
                &config.database_path().unwrap_or_default(),
            );
            check_private_mode(&styler, &config);
            check_shell_hook_diagnostics(&styler, &config);
            check_legacy_ignore_patterns(&styler, &config);
            check_vault_passphrase_exposure(&styler);
            check_record_daemon(&styler);
            check_break_glass_marker(&styler);
            print_sync_status(&styler, &config);
            print_vault_status(&styler, &config);
            print_shell_info(&styler);
            check_shell_integration(&styler);
            check_binary_in_path(&styler);
            return finish(&args);
        }
    };
    let db_path = database.path();
    set_summary_database(&database);
    say(
        &styler,
        StatusLevel::Ok,
        format!("Database opened at {}", db_path.display()),
    );

    check_database_size(&styler, &config, db_path);
    check_disk_space(&styler, db_path);
    check_write_permission(&styler, db_path);
    check_file_permissions(&styler, &cfg_path, db_path);
    check_wal_sidecar_permissions(&styler, db_path);
    check_config_directory_permissions(&styler, &cfg_path);
    check_symlinks(&styler, &cfg_path, db_path);
    check_integrity(&styler, &database);
    check_schema(&styler, &database);
    check_audit_chain(&styler, &database);
    check_private_mode(&styler, &config);
    check_shell_hook_diagnostics(&styler, &config);
    check_legacy_ignore_patterns(&styler, &config);
    check_vault_passphrase_exposure(&styler);
    print_record_count(&styler, &database);
    check_record_daemon(&styler);
    check_break_glass_marker(&styler);
    print_sync_status(&styler, &config);
    print_vault_status(&styler, &config);
    print_shell_info(&styler);
    check_shell_integration(&styler);
    check_binary_in_path(&styler);

    finish(&args)
}

fn reset_doctor_state(args: &DoctorArgs) {
    WARNING_COUNT.with(|counter| *counter.borrow_mut() = 0);
    JSON_MODE.with(|mode| *mode.borrow_mut() = args.json);
    CHECKS.with(|checks| checks.borrow_mut().clear());
    SUMMARY.with(|summary| {
        *summary.borrow_mut() = DoctorSummary {
            mh_version: env!("CARGO_PKG_VERSION").to_string(),
            strict: args.strict,
            ..DoctorSummary::default()
        };
    });
}

fn set_summary_config_path(path: &Path) {
    SUMMARY.with(|summary| {
        summary.borrow_mut().config_path = Some(path.display().to_string());
    });
}

fn set_summary_database(database: &Database) {
    SUMMARY.with(|summary| {
        let mut entry = summary.borrow_mut();
        entry.database_path = Some(database.path().display().to_string());
        entry.schema_version = database.schema_version().ok();
        entry.command_count = database.count_commands().ok();
    });
}

fn set_summary_private_mode(enabled: bool) {
    SUMMARY.with(|summary| summary.borrow_mut().private_mode = Some(enabled));
}

fn set_summary_daemon_running(running: bool) {
    SUMMARY.with(|summary| summary.borrow_mut().daemon_running = Some(running));
}

/// Returns the report from the most recent `run` in this thread (checks are accumulated in TLS).
pub fn current_report() -> DoctorReport {
    build_report()
}

fn build_report() -> DoctorReport {
    let warning_count = WARNING_COUNT.with(|counter| *counter.borrow());
    let checks = CHECKS.with(|checks| checks.borrow().clone());
    let summary = SUMMARY.with(|summary| summary.borrow().clone());
    let status = if warning_count > 0 {
        "warn"
    } else if checks.iter().any(|check| check.level == "error") {
        "error"
    } else {
        "ok"
    };
    DoctorReport {
        status: status.to_string(),
        warning_count,
        checks,
        summary,
    }
}

fn finish(args: &DoctorArgs) -> Result<()> {
    let report = build_report();
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).context("failed to serialize doctor report")?
        );
    }
    if args.strict && report.warning_count > 0 {
        anyhow::bail!(
            "doctor reported {} warning(s); fix the issues above or rerun without --strict",
            report.warning_count
        );
    }
    Ok(())
}

fn print_env_overrides(styler: &Styler) {
    if let Ok(path) = env::var("MH_CONFIG") {
        say(
            styler,
            StatusLevel::Info,
            format!("MH_CONFIG override: {path}"),
        );
    }
    if let Ok(path) = env::var("MH_DB") {
        say(styler, StatusLevel::Info, format!("MH_DB override: {path}"));
    }
    if env::var("MH_NO_DAEMON").is_ok() {
        say(
            styler,
            StatusLevel::Info,
            "MH_NO_DAEMON is set; hooks bypass the record daemon",
        );
    }
}

fn say(styler: &Styler, level: StatusLevel, message: impl AsRef<str>) {
    let message = message.as_ref();
    if matches!(level, StatusLevel::Warn) {
        WARNING_COUNT.with(|counter| *counter.borrow_mut() += 1);
    }
    CHECKS.with(|checks| {
        checks.borrow_mut().push(DoctorCheck {
            code: check_code(message),
            level: status_level_name(level).to_string(),
            message: message.to_string(),
        });
    });
    if !JSON_MODE.with(|mode| *mode.borrow()) {
        println!("{}", styler.status(level, message));
    }
}

fn status_level_name(level: StatusLevel) -> &'static str {
    match level {
        StatusLevel::Ok => "ok",
        StatusLevel::Warn => "warn",
        StatusLevel::Info => "info",
        StatusLevel::Error => "error",
    }
}

fn check_code(message: &str) -> String {
    let mut hasher = DefaultHasher::new();
    message.hash(&mut hasher);
    format!("check_{:016x}", hasher.finish())
}

fn check_database_size(styler: &Styler, config: &AppConfig, db_path: &Path) {
    match fs::metadata(db_path) {
        Ok(metadata) => {
            let size_mb = metadata.len() as f64 / 1_048_576.0;
            let max_size_mb = config.database.max_size_mb as f64;
            if size_mb > max_size_mb {
                say(
                    styler,
                    StatusLevel::Warn,
                    format!(
                        "Database size is {:.2} MB, above configured max of {} MB",
                        size_mb, config.database.max_size_mb
                    ),
                );
            } else {
                say(
                    styler,
                    StatusLevel::Info,
                    format!(
                        "Database size: {:.2} MB / {} MB",
                        size_mb, config.database.max_size_mb
                    ),
                );
            }
        }
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not read database metadata: {error}"),
        ),
    }
}

fn check_write_permission(styler: &Styler, db_path: &Path) {
    let directory = db_path.parent().unwrap_or_else(|| Path::new("."));
    let probe = directory.join(".mh-write-test");
    match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            say(styler, StatusLevel::Ok, "Write permission available");
        }
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Write permission unavailable: {error}"),
        ),
    }
}

fn check_wal_sidecar_permissions(styler: &Styler, db_path: &Path) {
    for label in ["WAL journal", "WAL shared memory"] {
        let extension = if label.contains("journal") {
            "db-wal"
        } else {
            "db-shm"
        };
        let path = db_path.with_extension(extension);
        if !path.exists() {
            continue;
        }
        match has_restricted_permissions(&path) {
            Ok(true) => say(
                styler,
                StatusLevel::Ok,
                format!("{label} permissions are restricted"),
            ),
            Ok(false) => say(
                styler,
                StatusLevel::Warn,
                format!(
                    "{label} permissions are too open: {} — reopen the database or run a record to tighten permissions",
                    path.display()
                ),
            ),
            Err(error) => say(
                styler,
                StatusLevel::Warn,
                format!("Could not inspect {label} permissions: {error}"),
            ),
        }
    }
}

fn check_config_directory_permissions(styler: &Styler, config_path: &Path) {
    let Some(dir) = config_path.parent() else {
        return;
    };
    if !dir.exists() {
        return;
    }
    match has_restricted_directory_permissions(dir) {
        Ok(true) => say(
            styler,
            StatusLevel::Ok,
            "Config directory permissions are restricted",
        ),
        Ok(false) => say(
            styler,
            StatusLevel::Warn,
            format!(
                "Config directory permissions are too open: {} — run {}",
                dir.display(),
                styler.accent("mh config fix")
            ),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect config directory permissions: {error}"),
        ),
    }
}

fn check_file_permissions(styler: &Styler, config_path: &Path, db_path: &Path) {
    for (label, path) in [
        ("Config", config_path),
        ("Database", db_path),
        ("Private mode", private_mode_path().as_path()),
    ] {
        if !path.exists() {
            continue;
        }
        match has_restricted_permissions(path) {
            Ok(true) => say(
                styler,
                StatusLevel::Ok,
                format!("{label} permissions are restricted"),
            ),
            Ok(false) => say(
                styler,
                StatusLevel::Warn,
                format!(
                    "{label} permissions are too open: {} — run {}",
                    path.display(),
                    styler.accent("mh config fix")
                ),
            ),
            Err(error) => say(
                styler,
                StatusLevel::Warn,
                format!("Could not inspect {label} permissions: {error}"),
            ),
        }
    }
}

fn check_symlinks(styler: &Styler, config_path: &Path, db_path: &Path) {
    for (label, path) in [("Config", config_path), ("Database", db_path)] {
        if path.exists() && path_is_symlink(path) {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "{label} path is a symlink ({}); symlink attacks can redirect mh data",
                    path.display()
                ),
            );
        }
    }
}

#[cfg(unix)]
fn path_is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn path_is_symlink(_path: &Path) -> bool {
    false
}

fn check_integrity(styler: &Styler, database: &Database) {
    match database.integrity_check() {
        Ok(integrity) if integrity == "ok" => {
            say(styler, StatusLevel::Ok, "Database integrity check passed");
        }
        Ok(integrity) => say(
            styler,
            StatusLevel::Warn,
            format!("Database integrity check returned: {integrity}"),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Database integrity check failed: {error}"),
        ),
    }
}

fn check_schema(styler: &Styler, database: &Database) {
    match database.schema_version() {
        Ok(current) if current < EXPECTED_SCHEMA_VERSION => {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "Pending migrations: schema at {current}, expected {EXPECTED_SCHEMA_VERSION} — run any mh command that opens the database"
                ),
            );
        }
        Ok(current) if current > EXPECTED_SCHEMA_VERSION => {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "Database schema {current} is newer than this mh build ({EXPECTED_SCHEMA_VERSION}); upgrade the mh binary"
                ),
            );
        }
        Ok(current) => say(
            styler,
            StatusLevel::Ok,
            format!("Database schema is current (version {current})"),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not read schema version: {error}"),
        ),
    }
}

fn check_audit_chain(styler: &Styler, database: &Database) {
    match database.verify_audit_chain() {
        Ok(()) => say(styler, StatusLevel::Ok, "Audit hash chain verified"),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!(
                "Audit hash chain verification failed: {error:#} — run {}",
                styler.accent("mh audit --rebuild-chain --yes")
            ),
        ),
    }

    match database.audit_rows_chronological(usize::MAX) {
        Ok(rows) => {
            let unsealed = crate::audit_chain::count_unsealed_entries(&rows);
            if unsealed > 0 {
                say(
                    styler,
                    StatusLevel::Warn,
                    format!(
                        "{unsealed} audit entries lack sealed hashes (legacy rows are not tamper-evident) — run {}",
                        styler.accent("mh audit --rebuild-chain --yes")
                    ),
                );
            }
        }
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect audit log: {error}"),
        ),
    }
}

fn check_vault_passphrase_exposure(styler: &Styler) {
    if std::env::var("MH_VAULT_PASSPHRASE")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        say(
            styler,
            StatusLevel::Warn,
            "MH_VAULT_PASSPHRASE is set; vault passphrase is visible to other local processes",
        );
    }
}

fn check_private_mode(styler: &Styler, config: &AppConfig) {
    let enabled = security::private_mode_enabled(config);
    set_summary_private_mode(enabled);
    if enabled {
        say(
            styler,
            StatusLevel::Warn,
            "Private mode is enabled; new commands are not being recorded",
        );
        say(
            styler,
            StatusLevel::Info,
            format!("Disable with: {}", styler.accent("mh private off")),
        );
    } else {
        say(styler, StatusLevel::Ok, "Private mode is disabled");
    }
}

fn check_shell_hook_diagnostics(styler: &Styler, config: &AppConfig) {
    if env::var("MH_POLICY_VERBOSE").is_ok() {
        say(
            styler,
            StatusLevel::Info,
            "MH_POLICY_VERBOSE is set; policy denials print to shell hook stderr",
        );
    } else if policy_has_deny_rules(config) {
        say(
            styler,
            StatusLevel::Info,
            format!(
                "Policy deny rules are active; set {} to show blocked commands in shell hooks",
                styler.accent("MH_POLICY_VERBOSE=1")
            ),
        );
    }

    if env::var("MH_RECORD_VERBOSE").is_ok() {
        say(
            styler,
            StatusLevel::Info,
            "MH_RECORD_VERBOSE is set; record diagnostics print to shell hook stderr",
        );
    }
}

fn policy_has_deny_rules(config: &AppConfig) -> bool {
    config.policy.default_action.eq_ignore_ascii_case("deny")
        || config
            .policy
            .rules
            .iter()
            .any(|rule| rule.action.eq_ignore_ascii_case("deny"))
}

fn check_record_daemon(styler: &Styler) {
    if env::var("MH_NO_DAEMON").is_ok() {
        say(
            styler,
            StatusLevel::Info,
            "Record daemon bypass is enabled (MH_NO_DAEMON)",
        );
        return;
    }

    let socket_path = crate::daemon::record_socket_path();
    match crate::daemon::daemon_status() {
        Ok(status) if status.running => {
            set_summary_daemon_running(true);
            let pid = status
                .pid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            say(
                styler,
                StatusLevel::Ok,
                format!(
                    "Record daemon is running (pid {pid}, socket {})",
                    status.socket_path.display()
                ),
            );
            check_daemon_socket_permissions(styler, &status.socket_path);
            check_daemon_pid_permissions(styler);
        }
        Ok(status)
            if status.socket_path.exists() && status.pid.is_some_and(|pid| !pid_is_alive(pid)) =>
        {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "Stale record daemon pid file (pid {} is not running) — run `{}`",
                    status.pid.unwrap_or_default(),
                    styler.accent("mh daemon stop")
                ),
            );
        }
        Ok(_) if socket_path.exists() => {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "Stale record daemon socket at {} — run `{}` or remove the socket",
                    socket_path.display(),
                    styler.accent("mh daemon stop")
                ),
            );
        }
        Ok(status) => {
            set_summary_daemon_running(false);
            say(
                styler,
                StatusLevel::Info,
                format!(
                    "Record daemon is not running (socket: {}). Hooks use direct SQLite; run `{}` or `{}` for lower overhead",
                    status.socket_path.display(),
                    styler.accent("mh daemon start"),
                    styler.accent("mh daemon install")
                ),
            );
        }
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect record daemon: {error}"),
        ),
    }
}

fn print_record_count(styler: &Styler, database: &Database) {
    match database.count_commands() {
        Ok(count) => say(styler, StatusLevel::Info, format!("Command count: {count}")),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not count commands: {error}"),
        ),
    }
}

fn print_sync_status(styler: &Styler, config: &AppConfig) {
    let state = if config.sync.enabled {
        styler.success("enabled")
    } else {
        styler.muted("disabled")
    };
    let server = if config.sync.server_url.trim().is_empty() {
        styler.muted("no server configured")
    } else {
        styler.accent(&config.sync.server_url)
    };
    say(
        styler,
        StatusLevel::Info,
        styler.label_value("Sync", format!("{state} ({server})")),
    );
}

fn print_vault_status(styler: &Styler, config: &AppConfig) {
    let vault = if config.vault.enabled {
        styler.success("enabled")
    } else {
        styler.muted("disabled")
    };
    let keyring = if config.vault.use_keyring {
        styler.success("enabled")
    } else {
        styler.muted("disabled")
    };
    say(
        styler,
        StatusLevel::Info,
        styler.label_value("Vault config", format!("{vault} (keyring: {keyring})")),
    );
}

fn print_shell_info(styler: &Styler) {
    match env::var("SHELL") {
        Ok(shell) => say(styler, StatusLevel::Info, format!("Current shell: {shell}")),
        Err(_) => say(
            styler,
            StatusLevel::Warn,
            "SHELL environment variable is not set",
        ),
    }
}

fn check_shell_integration(styler: &Styler) {
    let Some(home) = dirs::home_dir() else {
        say(
            styler,
            StatusLevel::Warn,
            "Could not determine home directory for shell integration check",
        );
        return;
    };

    let Some(shell) = detect_shell_kind() else {
        say(
            styler,
            StatusLevel::Warn,
            "Could not determine shell type from $SHELL (supported: bash, zsh, fish, nushell)",
        );
        return;
    };

    let config_path = resolve_config_path(shell, &home);
    let candidates = config_candidates(shell, &home);

    if !config_path.exists() {
        let hint = candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        say(
            styler,
            StatusLevel::Warn,
            format!(
                "Shell integration not installed (expected one of: {hint}) — run {}",
                styler.accent(format!("mh init {} --install", shell_cli_name(shell)))
            ),
        );
        return;
    }

    let content = fs::read_to_string(&config_path).unwrap_or_default();
    if shell_integration_active(&content) {
        say(
            styler,
            StatusLevel::Ok,
            format!(
                "Shell integration is active in {} ({})",
                config_path.display(),
                shell_cli_name(shell)
            ),
        );
        check_duplicate_hooks(styler, shell, &content);
    } else {
        say(
            styler,
            StatusLevel::Warn,
            format!(
                "Shell integration not detected in {} — run {}",
                config_path.display(),
                styler.accent(format!("mh init {} --install", shell_cli_name(shell)))
            ),
        );
    }
}

fn shell_cli_name(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::Fish => "fish",
        ShellKind::Nushell => "nushell",
    }
}

fn detect_shell_kind() -> Option<ShellKind> {
    let shell = std::env::var("SHELL").ok()?;
    let name = Path::new(&shell).file_name()?.to_string_lossy();
    match name.as_ref() {
        "bash" => Some(ShellKind::Bash),
        "zsh" => Some(ShellKind::Zsh),
        "fish" => Some(ShellKind::Fish),
        "nu" | "nushell" => Some(ShellKind::Nushell),
        _ => None,
    }
}

fn check_daemon_socket_permissions(styler: &Styler, socket_path: &Path) {
    if !socket_path.exists() {
        return;
    }
    match has_restricted_permissions(socket_path) {
        Ok(true) => say(
            styler,
            StatusLevel::Ok,
            "Record daemon socket permissions are restricted",
        ),
        Ok(false) => say(
            styler,
            StatusLevel::Warn,
            format!(
                "Record daemon socket permissions are too open: {}",
                socket_path.display()
            ),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect daemon socket permissions: {error}"),
        ),
    }
}

fn check_daemon_pid_permissions(styler: &Styler) {
    let pid_path = record_pid_path();
    if !pid_path.exists() {
        return;
    }
    match has_restricted_permissions(&pid_path) {
        Ok(true) => say(
            styler,
            StatusLevel::Ok,
            "Record daemon pid file permissions are restricted",
        ),
        Ok(false) => say(
            styler,
            StatusLevel::Warn,
            format!(
                "Record daemon pid file permissions are too open: {}",
                pid_path.display()
            ),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect daemon pid file permissions: {error}"),
        ),
    }
}

fn check_break_glass_marker(styler: &Styler) {
    let path = break_glass::break_glass_path();
    if !path.exists() {
        return;
    }
    match has_restricted_permissions(&path) {
        Ok(true) => say(
            styler,
            StatusLevel::Warn,
            "Break-glass mode marker is present (recording override active)",
        ),
        Ok(false) => say(
            styler,
            StatusLevel::Warn,
            format!(
                "Break-glass marker exists but permissions are too open: {}",
                path.display()
            ),
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect break-glass marker: {error}"),
        ),
    }
}

#[cfg(unix)]
fn has_restricted_directory_permissions(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .with_context(|| format!("failed to read permissions for {}", path.display()))?
        .permissions()
        .mode();
    Ok(mode & 0o077 == 0)
}

#[cfg(not(unix))]
fn has_restricted_directory_permissions(path: &Path) -> Result<bool> {
    let _ = path;
    Ok(true)
}

fn check_binary_in_path(styler: &Styler) {
    if let Some(path) = find_binary_in_path("mh") {
        say(
            styler,
            StatusLevel::Ok,
            format!("Binary is in PATH: {}", path.display()),
        );
    } else {
        say(styler, StatusLevel::Warn, "Binary is not available in PATH");
    }
}

fn shell_integration_active(content: &str) -> bool {
    content.contains(BEGIN_MARKER)
        || content.contains("_mh_preexec")
        || content.contains("mh_preexec")
        || content.contains("function mh_preexec")
}

fn check_legacy_ignore_patterns(styler: &Styler, config: &AppConfig) {
    for warning in crate::config::legacy_ignore_pattern_warnings(config) {
        say(
            styler,
            StatusLevel::Warn,
            format!("{warning} — run {}", styler.accent("mh config fix")),
        );
    }
}

fn check_duplicate_hooks(styler: &Styler, shell: ShellKind, content: &str) {
    let duplicates = hooks::duplicate_hook_count(shell, content);
    if duplicates == 0 {
        return;
    }
    say(
        styler,
        StatusLevel::Warn,
        format!(
            "Detected {duplicates} duplicate mh hook registration(s) — run {}",
            styler.accent(format!("mh init {} --repair", shell_cli_name(shell)))
        ),
    );
}

fn check_env_override_paths(styler: &Styler) {
    if let Ok(path) = env::var("MH_CONFIG") {
        let expanded = crate::config::expand_tilde(&path);
        if !expanded.exists() {
            say(
                styler,
                StatusLevel::Warn,
                format!("MH_CONFIG points to missing file: {}", expanded.display()),
            );
        } else {
            check_symlinks(styler, &expanded, &expanded);
        }
    }

    if let Ok(path) = env::var("MH_DB") {
        let expanded = crate::config::expand_tilde(&path);
        if expanded.is_dir() {
            say(
                styler,
                StatusLevel::Warn,
                format!(
                    "MH_DB points to a directory, not a database file: {}",
                    expanded.display()
                ),
            );
        } else if !expanded.exists() {
            if let Some(parent) = expanded.parent() {
                check_disk_space(styler, parent);
            }
        } else {
            check_file_permissions(styler, &expanded, &expanded);
            check_symlinks(styler, &expanded, &expanded);
        }
        for warning in crate::config::database_path_warnings(&expanded) {
            say(styler, StatusLevel::Warn, warning);
        }
    }

    if env::var("MH_DB").is_err()
        && let Ok(config) = AppConfig::load()
        && let Ok(path) = config.database_path()
    {
        for warning in crate::config::database_path_warnings(&path) {
            say(styler, StatusLevel::Warn, warning);
        }
    }
}

fn validate_config_engines(styler: &Styler, config: &AppConfig) {
    match crate::security::SecurityEngine::from_config(config) {
        Ok(_) => say(
            styler,
            StatusLevel::Ok,
            "Security ignore patterns are valid",
        ),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Security config validation failed: {error:#}"),
        ),
    }
    match crate::policy::PolicyEngine::from_config(config) {
        Ok(_) => say(styler, StatusLevel::Ok, "Policy rules are valid"),
        Err(error) => say(
            styler,
            StatusLevel::Warn,
            format!("Policy config validation failed: {error:#}"),
        ),
    }
}

#[cfg(unix)]
fn check_disk_space(styler: &Styler, path: &Path) {
    use std::ffi::CString;

    let target = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    let Ok(c_path) = CString::new(target.to_string_lossy().as_bytes()) else {
        return;
    };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        say(
            styler,
            StatusLevel::Warn,
            format!("Could not inspect free disk space for {}", target.display()),
        );
        return;
    }
    let available = stat.f_bavail as u64 * stat.f_frsize as u64;
    const MIN_FREE_BYTES: u64 = 100 * 1024 * 1024;
    if available < MIN_FREE_BYTES {
        say(
            styler,
            StatusLevel::Warn,
            format!(
                "Low disk space for database directory ({} MB free; recommend at least 100 MB)",
                available / 1_048_576
            ),
        );
    } else {
        say(
            styler,
            StatusLevel::Ok,
            format!("Disk space available: {} MB", available / 1_048_576),
        );
    }
}

#[cfg(not(unix))]
fn check_disk_space(_styler: &Styler, _path: &Path) {}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}

fn find_binary_in_path(binary: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    for path in env::split_paths(&paths) {
        let candidate = path.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::hooks;

    #[test]
    fn detects_managed_shell_block() {
        let content =
            format!("{BEGIN_MARKER}\neval \"$(mh init zsh)\"\n# <<< mh shell integration <<<");
        assert!(shell_integration_active(&content));
    }

    #[test]
    fn detects_zsh_hook_symbols() {
        assert!(shell_integration_active("function _mh_preexec() { }"));
    }

    #[test]
    fn counts_duplicate_zsh_hooks() {
        let content = "add-zsh-hook preexec _mh_preexec\nadd-zsh-hook preexec _mh_preexec";
        assert_eq!(hooks::duplicate_hook_count(ShellKind::Zsh, content), 1);
    }

    #[test]
    fn detects_policy_deny_rules() {
        let mut config = AppConfig::default();
        assert!(
            policy_has_deny_rules(&config),
            "default policy includes production deny rules"
        );
        config.policy.default_action = "allow".to_string();
        config
            .policy
            .rules
            .retain(|rule| !rule.action.eq_ignore_ascii_case("deny"));
        assert!(!policy_has_deny_rules(&config));
    }

    #[test]
    #[cfg(unix)]
    fn detects_symlink_without_following_target() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let target = temp_dir.path().join("target.db");
        let link = temp_dir.path().join("history.db");
        std::fs::write(&target, "not sqlite").expect("target");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        assert!(path_is_symlink(&link));
        assert!(!path_is_symlink(&target));
    }
}
