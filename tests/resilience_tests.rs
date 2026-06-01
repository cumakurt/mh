use std::fs;

use mh::config::AppConfig;
use mh::db::Database;
use mh::record_pipeline::{RecordPayload, execute};

mod common;
use common::EnvGuard;

#[test]
fn opens_database_from_mh_db_override() {
    let _guard = EnvGuard::save(&[
        "MH_DB",
        "MH_CONFIG",
        "MH_CONFIG_NO_CACHE",
        "XDG_CONFIG_HOME",
    ]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("override.db");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        std::env::set_var("MH_DB", db_path.to_string_lossy().to_string());
    }

    assert_eq!(
        std::env::var("MH_DB").expect("MH_DB"),
        db_path.to_string_lossy()
    );

    let config = AppConfig::load().expect("load config");
    assert_eq!(config.database_path().expect("path"), db_path);

    let database = Database::open(&config).expect("open override db");
    execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo override-db".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("record");

    assert!(db_path.exists());
    assert_eq!(database.count_commands().expect("count"), 1);
}

#[test]
fn rejects_foreign_sqlite_without_mh_schema() {
    let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("foreign.db");

    let connection = rusqlite::Connection::open(&db_path).expect("create sqlite");
    connection
        .execute("CREATE TABLE notes (body TEXT)", [])
        .expect("create table");
    drop(connection);

    let mut config = AppConfig::default();
    config.database.path = db_path.to_string_lossy().to_string();

    let database = Database::open(&config).expect("migrations should create mh schema");
    assert!(database.count_commands().is_ok());
    assert!(fs::metadata(&db_path).is_ok());
}

#[test]
fn record_sets_ssh_flag_from_env() {
    let _guard = EnvGuard::save(&[
        "MH_DB",
        "MH_CONFIG_NO_CACHE",
        "SSH_CONNECTION",
        "SSH_CLIENT",
        "MH_CONFIG",
        "XDG_CONFIG_HOME",
    ]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    _guard.use_isolated_config(&temp_dir);
    let mut config = AppConfig::load().expect("load config");
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();

    unsafe {
        std::env::set_var("SSH_CONNECTION", "127.0.0.1 12345 22");
    }

    let database = Database::open(&config).expect("open database");
    execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo ssh-session".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    )
    .expect("record");

    let ssh_rows = database
        .search_commands(&mh::models::SearchFilters {
            query: None,
            cwd: None,
            failed: false,
            success: false,
            user: None,
            shell: None,
            after: None,
            before: None,
            regex: false,
            fuzzy: false,
            fts: false,
            tag: None,
            category: None,
            pinned: false,
            duration_gt: None,
            duration_lt: None,
            hostname: None,
            ssh: true,
            root: false,
            limit: 5,
            session_id: None,
            git_repo: None,
            git_branch: None,
            git_commit: None,
            environment: None,
        })
        .expect("ssh search");
    assert_eq!(ssh_rows.len(), 1);
}

#[test]
fn rejects_invalid_config_toml() {
    let _guard = EnvGuard::save(&["MH_CONFIG", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(&config_path, "history = not-a-table").expect("write config");

    unsafe {
        std::env::set_var("MH_CONFIG", &config_path);
    }

    let result = AppConfig::load();
    assert!(result.is_err(), "invalid TOML should fail to load");
}

#[test]
fn open_fails_on_corrupt_database_file() {
    let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("history.db");
    std::fs::write(&db_path, b"not-a-sqlite-database").expect("write corrupt db");

    let result = Database::open_path(db_path);
    assert!(result.is_err(), "corrupt database should fail to open");
}

#[test]
fn open_creates_missing_database_directory() {
    let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("nested").join("history.db");

    let mut config = AppConfig::default();
    config.database.path = db_path.to_string_lossy().to_string();

    let database = Database::open(&config).expect("database should be created");
    assert!(database.path().exists());
}

#[test]
fn record_pipeline_policy_deny_does_not_error() {
    let _guard = EnvGuard::save(&[
        "MH_DB",
        "MH_CONFIG_NO_CACHE",
        "MH_CONFIG",
        "XDG_CONFIG_HOME",
    ]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    _guard.use_isolated_config(&temp_dir);
    let mut config = AppConfig::load().expect("load config");
    config.database.path = temp_dir
        .path()
        .join("history.db")
        .to_string_lossy()
        .to_string();
    config.policy.default_action = "deny".to_string();

    let database = Database::open(&config).expect("open database");
    let result = execute(
        &config,
        &database,
        &RecordPayload {
            command: "echo policy-test".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("test".to_string()),
            exit_code: Some(0),
            duration_ms: Some(1),
            started_at: None,
            finished_at: None,
            session_id: Some("sess".to_string()),
            tty: None,
            tags: None,
            env_context: None,
        },
    );
    match result {
        Err(error) => assert!(
            error.chain().any(|cause| matches!(
                cause.downcast_ref::<mh::errors::MhError>(),
                Some(mh::errors::MhError::PolicyDenied(_))
            )),
            "policy deny should return PolicyDenied, got: {error:#}"
        ),
        Ok(()) => panic!("policy deny should return an error"),
    }
    assert_eq!(
        database.count_commands().expect("count"),
        0,
        "denied command must not be stored"
    );
}

#[test]
fn rejects_empty_database_path() {
    let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let mut config = AppConfig::default();
    config.database.path = String::new();
    assert!(config.database_path().is_err());

    config.database.path = "   ".to_string();
    assert!(config.database_path().is_err());
}

#[test]
fn rejects_mh_db_directory_path() {
    let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
    _guard.clear_mh_env();

    let temp_dir = tempfile::tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("MH_DB", temp_dir.path().to_string_lossy().to_string());
    }

    let config = AppConfig::default();
    let db_path = config.database_path().expect("MH_DB path");
    let result = Database::open_path(db_path);
    assert!(
        result.is_err(),
        "directory path should fail to open as database"
    );
    let message = result.err().expect("error message").to_string();
    assert!(
        message.contains("directory"),
        "error should mention directory"
    );
}

#[test]
fn rejects_unwritable_database_directory() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if mh::identity::is_effective_root() {
            return;
        }

        let _guard = EnvGuard::save(&["MH_DB", "MH_CONFIG_NO_CACHE"]);
        _guard.clear_mh_env();

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let db_dir = temp_dir.path().join("readonly");
        std::fs::create_dir_all(&db_dir).expect("dir");
        let mut perms = std::fs::metadata(&db_dir).expect("metadata").permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&db_dir, perms).expect("chmod");

        let mut config = AppConfig::default();
        config.database.path = db_dir.join("history.db").to_string_lossy().to_string();

        let result = Database::open(&config);
        assert!(
            result.is_err(),
            "creating a database in a read-only directory should fail"
        );
    }
}

#[test]
fn format_user_error_suggests_doctor_for_lock() {
    let error = anyhow::anyhow!(mh::errors::MhError::DatabaseLocked);
    let message = mh::errors::format_user_error(&error);
    assert!(message.contains("mh doctor"));
}

#[test]
fn map_sqlite_error_maps_busy_to_database_locked() {
    use mh::errors::MhError;

    let error = mh::errors::map_sqlite_error(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ErrorCode::DatabaseBusy,
            extended_code: 5,
        },
        None,
    ));
    assert!(
        error.chain().any(|cause| matches!(
            cause.downcast_ref::<MhError>(),
            Some(MhError::DatabaseLocked)
        )),
        "expected DatabaseLocked, got: {error:#}"
    );
}

#[test]
fn database_open_refuses_symlink_path() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let real_db = temp_dir.path().join("real.db");
    std::fs::write(&real_db, b"").expect("real db placeholder");
    let link = temp_dir.path().join("history.db");
    std::os::unix::fs::symlink(&real_db, &link).expect("symlink");

    match Database::open_path(link) {
        Err(error) => {
            let message = format!("{error:#}");
            assert!(
                message.contains("symlink"),
                "expected symlink rejection, got: {message}"
            );
        }
        Ok(_) => panic!("symlink database path must be rejected"),
    }
}
