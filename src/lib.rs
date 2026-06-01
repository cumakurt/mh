pub mod audit_chain;
pub mod break_glass;
pub mod classifier;
pub mod cli;
pub mod command_exec;
pub mod commands;
pub mod config;
pub mod daemon;
pub mod db;
pub mod environment;
pub mod errors;
pub mod execution_policy;
pub mod git_detect;
pub mod identity;
pub mod incident_bundle;
pub mod models;
pub mod output;
pub mod policy;
pub mod policy_check;
pub mod policy_pack;
pub mod ranking;
pub mod record_engines;
pub mod record_pipeline;
pub mod risk;
pub mod security;
pub mod semantic_search;
pub mod shell;
pub mod siem;
#[cfg(feature = "sync")]
pub mod sync;
pub mod timestamp;

use anyhow::Result;
use clap::Parser;

pub fn run() -> Result<()> {
    let cli = cli::Cli::parse();
    commands::dispatch(cli)
}
