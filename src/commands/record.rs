use std::thread;
use std::time::Duration;

use anyhow::{Result, bail};

use crate::cli::RecordArgs;
use crate::config::AppConfig;
use crate::daemon::{self, DaemonError};
use crate::db::Database;
use crate::errors::{self, MhError};
use crate::record_pipeline;

const DB_LOCK_RETRIES: u32 = 3;

fn is_policy_denied(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| matches!(cause.downcast_ref::<MhError>(), Some(MhError::PolicyDenied(_))))
}

pub fn run(args: RecordArgs) -> Result<()> {
    if !args.no_daemon {
        match daemon::record_via_daemon(&args) {
            Ok(()) => return Ok(()),
            Err(DaemonError::Unavailable) => {}
            Err(DaemonError::Failed(message)) => bail!("{message}"),
        }
    }

    let config = AppConfig::load()?;
    let payload = record_pipeline::RecordPayload::from(&args);
    let database = Database::open(&config)?;

    let mut last_error = None;
    for attempt in 0..DB_LOCK_RETRIES {
        match record_pipeline::execute(&config, &database, &payload) {
            Ok(()) => return Ok(()),
            Err(error) if is_policy_denied(&error) => {
                if std::env::var("MH_POLICY_VERBOSE").is_ok() {
                    eprintln!("{}", errors::format_user_error(&error));
                }
                return Ok(());
            }
            Err(error) if MhError::is_retryable_database_lock(&error) && attempt + 1 < DB_LOCK_RETRIES => {
                thread::sleep(Duration::from_millis(25 * (attempt as u64 + 1)));
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("record failed after database lock retries")))
}
