use mh::db::{Database, EXPECTED_SCHEMA_VERSION};

#[test]
fn applies_all_schema_migrations() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    assert_eq!(
        database.schema_version().expect("schema version"),
        EXPECTED_SCHEMA_VERSION
    );
    assert_eq!(EXPECTED_SCHEMA_VERSION, 11);
}

#[test]
fn enterprise_migration_columns_are_idempotent() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let path = temp_dir.path().join("history.db");
    let database = Database::open_path(path.clone()).expect("database should open");

    assert_eq!(
        database.schema_version().expect("schema version"),
        EXPECTED_SCHEMA_VERSION
    );

    let database = Database::open_path(path).expect("reopen should succeed");
    assert_eq!(
        database
            .schema_version()
            .expect("schema version after reopen"),
        EXPECTED_SCHEMA_VERSION
    );
}

#[test]
fn open_rejects_database_with_newer_schema() {
    use mh::errors::MhError;

    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let path = temp_dir.path().join("history.db");
    Database::open_path(path.clone()).expect("initialize database");
    let connection = rusqlite::Connection::open(&path).expect("reopen");
    connection
        .pragma_update(None, "user_version", EXPECTED_SCHEMA_VERSION + 1)
        .expect("bump schema");

    match Database::open_path(path) {
        Err(error) => assert!(
            error
                .chain()
                .any(|cause| matches!(cause.downcast_ref::<MhError>(), Some(MhError::Config(_)))),
            "expected schema mismatch error, got: {error:#}"
        ),
        Ok(_) => panic!("newer schema database must be rejected"),
    }
}
