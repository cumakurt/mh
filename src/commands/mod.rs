pub mod about;
pub mod audit;
pub mod break_glass;
pub mod clear;
pub mod completions;
pub mod config;
pub mod context;
pub mod daemon;
pub mod delete;
pub mod diff;
pub mod doctor;
pub mod export;
pub mod hold;
pub mod import_history;
pub mod init;
pub mod last;
pub mod man_page;
pub mod pick;
pub mod pin;
pub mod policy;
pub mod private;
pub mod record;
pub mod replay;
pub mod risk;
pub mod runbook;
pub mod search;
pub mod snippet;
pub mod stats;
pub mod sync;
pub mod tag;
pub mod tags;
pub mod timeline;
pub mod tui;
pub mod vault;
pub mod watch;

use anyhow::Result;

use crate::cli::{Cli, Command};

pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => init::run(args),
        Command::Record(args) => record::run(args),
        Command::Daemon(args) => daemon::run(args),
        Command::Search(args) => search::run(args),
        Command::Last(args) => last::run(args),
        Command::Stats(args) => stats::run(args),
        Command::Delete(args) => delete::run(args),
        Command::Clear(args) => clear::run(args),
        Command::Export(args) => export::run(args),
        Command::Import(args) => import_history::run(args),
        Command::Doctor(args) => doctor::run(args),
        Command::Config(args) => config::run(args),
        Command::Tag(args) => tag::run(args),
        Command::Untag(args) => tag::run_untag(args),
        Command::Tags(args) => tags::run(args),
        Command::Pin(args) => pin::run(args, true),
        Command::Unpin(args) => pin::run(args, false),
        Command::Pinned(args) => pin::run_pinned(args),
        Command::Pick(args) => pick::run(args),
        Command::Tui(args) => tui::run(args),
        Command::Snippet(args) => snippet::run(args),
        Command::Replay(args) => replay::run(args),
        Command::Risk(args) => risk::run(args),
        Command::Context(args) => context::run(args),
        Command::Diff(args) => diff::run(args),
        Command::Audit(args) => audit::run(args),
        Command::Private(args) => private::run(args),
        Command::Vault(args) => vault::run(args),
        Command::Sync(args) => sync::run(args),
        Command::Completions(args) => completions::run(args),
        Command::Man(args) => man_page::run(args),
        Command::About => about::run(),
        Command::Policy(args) => policy::run(args),
        Command::Timeline(args) => timeline::run(args),
        Command::Hold(args) => hold::run(args),
        Command::Watch(args) => watch::run(args),
        Command::Runbook(args) => runbook::run(args),
        Command::BreakGlass(args) => break_glass::run(args),
    }
}
