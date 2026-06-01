use thiserror::Error;

#[derive(Debug, Error)]
pub enum MhError {
    #[error("database is locked; retry later")]
    DatabaseLocked,
    #[error("database schema version {found} is older than expected {expected}")]
    SchemaOutdated { found: i64, expected: i64 },
    #[error("configuration error: {0}")]
    Config(String),
    #[error("policy denied: {0}")]
    PolicyDenied(String),
}

impl MhError {
    pub fn is_retryable_database_lock(error: &anyhow::Error) -> bool {
        error.chain().any(|cause| {
            if matches!(
                cause.downcast_ref::<MhError>(),
                Some(MhError::DatabaseLocked)
            ) {
                return true;
            }
            let message = cause.to_string();
            message.contains("database is locked") || message.contains("database is busy")
        })
    }
}

pub fn map_sqlite_error(error: rusqlite::Error) -> anyhow::Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            MhError::DatabaseLocked.into()
        }
        _ => error.into(),
    }
}

pub fn format_user_error(error: &anyhow::Error) -> String {
    for cause in error.chain() {
        match cause.downcast_ref::<MhError>() {
            Some(MhError::DatabaseLocked) => {
                return "database is locked; retry later or run: mh doctor".to_string();
            }
            Some(MhError::SchemaOutdated { found, expected }) => {
                return format!(
                    "database schema version {found} is older than expected {expected}; run: mh doctor"
                );
            }
            Some(MhError::Config(message)) => {
                return format!("configuration error: {message} (run: mh config validate)");
            }
            Some(MhError::PolicyDenied(message)) => {
                return format!("policy denied: {message}");
            }
            None => {}
        }
    }

    let message = format!("{error:#}");
    if message.contains("database is locked") || message.contains("database is busy") {
        return format!("{message}\ntry: mh doctor");
    }
    if message.contains("invalid ignore regex") || message.contains("config") {
        return format!("{message}\ntry: mh config validate");
    }
    message
}
