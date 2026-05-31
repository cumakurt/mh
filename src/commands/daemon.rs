use anyhow::Result;

use crate::cli::DaemonArgs;
use crate::daemon::{
    daemon_status, install_systemd_unit, run_daemon, start_daemon, stop_daemon,
};

pub fn run(args: DaemonArgs) -> Result<()> {
    match args.action {
        crate::cli::DaemonAction::Run => run_daemon(),
        crate::cli::DaemonAction::Start => start_daemon(),
        crate::cli::DaemonAction::Stop => stop_daemon(),
        crate::cli::DaemonAction::Install => install_systemd_unit(),
        crate::cli::DaemonAction::Status => {
            let status = daemon_status()?;
            if status.running {
                println!(
                    "running (socket: {}, pid: {})",
                    status.socket_path.display(),
                    status
                        .pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                );
            } else {
                println!(
                    "not running (socket: {})",
                    status.socket_path.display()
                );
            }
            Ok(())
        }
    }
}
