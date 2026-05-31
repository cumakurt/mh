use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "mh")]
#[command(version)]
#[command(author)]
#[command(about = "A modern command history manager")]
#[command(
    after_help = "Author: Cuma Kurt <cumakurt@gmail.com>\nGitHub: https://github.com/cumakurt/mh\nLinkedIn: https://www.linkedin.com/in/cuma-kurt-34414917/"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate shell integration code.
    Init(InitArgs),
    /// Record one executed command.
    Record(RecordArgs),
    /// Run the background record daemon (keeps one database connection open).
    Daemon(DaemonArgs),
    /// Search command history.
    Search(SearchArgs),
    /// Show recent commands.
    Last(LastArgs),
    /// Show usage statistics.
    Stats(StatsArgs),
    /// Delete command history records.
    Delete(DeleteArgs),
    /// Clear command history.
    Clear(ClearArgs),
    /// Export command history.
    Export(ExportArgs),
    /// Import command history.
    Import(ImportArgs),
    /// Check local configuration and database health.
    Doctor(DoctorArgs),
    /// Inspect configuration.
    Config(ConfigArgs),
    /// Add tags to commands.
    Tag(TagArgs),
    /// Remove tags from a command.
    Untag(UntagArgs),
    /// List known tags.
    Tags(TagsArgs),
    /// Pin commands.
    Pin(PinArgs),
    /// Unpin commands.
    Unpin(PinArgs),
    /// Show pinned commands.
    Pinned(PinnedArgs),
    /// Pick a command interactively and print it.
    Pick(PickArgs),
    /// Launch the interactive terminal interface.
    Tui(TuiArgs),
    /// Manage reusable command snippets.
    Snippet(SnippetArgs),
    /// Re-execute a command from history.
    Replay(ReplayArgs),
    /// Assess and scan risky commands in history.
    Risk(RiskArgs),
    /// Show git repository context for commands.
    Context(ContextArgs),
    /// Compare history groups.
    Diff(DiffArgs),
    /// Show security audit log entries.
    Audit(AuditArgs),
    /// Manage private mode.
    Private(PrivateArgs),
    /// Manage encrypted command vault.
    Vault(VaultArgs),
    /// Manage optional remote sync.
    Sync(SyncArgs),
    /// Generate shell completion scripts.
    Completions(CompletionArgs),
    /// Generate the mh man page.
    Man(ManArgs),
    /// Show application and developer information.
    About,
    /// Evaluate and inspect policy rules.
    Policy(PolicyArgs),
    /// Session timeline and incident forensics.
    Timeline(TimelineArgs),
    /// Manage legal holds and retention.
    Hold(HoldArgs),
    /// Stream audit events for SIEM integration.
    Watch(WatchArgs),
    /// Manage reusable runbooks from sessions.
    Runbook(RunbookArgs),
    /// Emergency recording override with mandatory reason.
    BreakGlass(BreakGlassArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(value_enum)]
    pub shell: ShellKind,
    #[arg(long, conflicts_with = "repair")]
    pub install: bool,
    /// Remove duplicate mh shell hook registrations and managed blocks.
    #[arg(long, conflicts_with = "install")]
    pub repair: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Nushell,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    #[arg(value_enum)]
    pub shell: CompletionShell,
    #[arg(long, short)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

#[derive(Debug, Args)]
pub struct ManArgs {
    #[arg(long, short)]
    pub output: Option<String>,
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: DaemonAction,
}

#[derive(Debug, Subcommand)]
pub enum DaemonAction {
    /// Run the daemon in the foreground.
    Run,
    /// Start the daemon in the background.
    Start,
    /// Stop a running daemon.
    Stop,
    /// Show daemon status.
    Status,
    /// Write a systemd user unit to ~/.config/systemd/user/.
    Install,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Exit with a non-zero status when warnings are reported.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    /// Always write through the database instead of the record daemon.
    #[arg(long)]
    pub no_daemon: bool,
    #[arg(long)]
    pub command: String,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub shell: Option<String>,
    #[arg(long)]
    pub exit_code: Option<i32>,
    #[arg(long)]
    pub duration_ms: Option<i64>,
    #[arg(long)]
    pub started_at: Option<String>,
    #[arg(long)]
    pub finished_at: Option<String>,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub tty: Option<String>,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long)]
    pub env_context: Option<String>,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: Option<String>,
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub failed: bool,
    #[arg(long)]
    pub success: bool,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub shell: Option<String>,
    #[arg(long)]
    pub after: Option<String>,
    #[arg(long)]
    pub before: Option<String>,
    #[arg(long)]
    pub regex: bool,
    #[arg(long)]
    pub fuzzy: bool,
    #[arg(long)]
    pub fts: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub pinned: bool,
    #[arg(long = "duration-gt")]
    pub duration_gt: Option<i64>,
    #[arg(long = "duration-lt")]
    pub duration_lt: Option<i64>,
    #[arg(long)]
    pub hostname: Option<String>,
    #[arg(long)]
    pub ssh: bool,
    #[arg(long)]
    pub root: bool,
    #[arg(long = "git-repo")]
    pub git_repo: Option<String>,
    #[arg(long = "git-branch")]
    pub git_branch: Option<String>,
    #[arg(long = "git-commit")]
    pub git_commit: Option<String>,
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub csv: bool,
    #[arg(long)]
    pub plain: bool,
}

#[derive(Debug, Args)]
pub struct LastArgs {
    pub limit: Option<usize>,
    #[arg(long)]
    pub failed: bool,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub session: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub pinned: bool,
    #[arg(long = "git-repo")]
    pub git_repo: Option<String>,
    #[arg(long = "git-branch")]
    pub git_branch: Option<String>,
    #[arg(long = "git-commit")]
    pub git_commit: Option<String>,
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub markdown: bool,
    #[arg(long)]
    pub plain: bool,
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long)]
    pub today: bool,
    #[arg(long)]
    pub week: bool,
    #[arg(long)]
    pub month: bool,
    #[arg(long)]
    pub category: bool,
    #[arg(long)]
    pub heatmap: bool,
    #[arg(long, default_value_t = 10)]
    pub top: usize,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub id: Option<i64>,
    #[arg(long)]
    pub older_than: Option<String>,
    #[arg(long)]
    pub contains: Option<String>,
    #[arg(long)]
    pub failed: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ClearArgs {
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub before: Option<String>,
    #[arg(long)]
    pub keep_pinned: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(long, value_name = "FILE")]
    pub json: Option<String>,
    #[arg(long, value_name = "FILE")]
    pub csv: Option<String>,
    #[arg(long, value_name = "FILE")]
    pub markdown: Option<String>,
    #[arg(long, value_name = "FILE")]
    pub compressed: Option<String>,
    #[arg(long, value_name = "FILE")]
    pub sqlite: Option<String>,
    #[arg(long)]
    pub after: Option<String>,
    #[arg(long)]
    pub before: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    /// Omit all rows from audit_log in SQLite exports.
    #[arg(long)]
    pub without_audit: bool,
    /// Redact sensitive values in audit_log when exporting SQLite.
    #[arg(long)]
    pub sanitize_audit: bool,
    /// Redact sensitive values in exported command text (all formats). On by default.
    #[arg(long, default_value_t = true)]
    pub sanitize: bool,
    /// Export full command text including secrets (disables default redaction).
    #[arg(long)]
    pub include_secrets: bool,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    pub file: String,
    #[arg(long)]
    pub merge: bool,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the active configuration.
    Show,
    /// Print the active configuration path.
    Path,
    /// Open the configuration in $EDITOR.
    Edit,
    /// Set a configuration value using dotted path syntax.
    Set { key: String, value: String },
    /// Reset configuration to defaults.
    Reset,
    /// Validate the active configuration.
    Validate,
    /// Tighten config permissions and remove legacy ignore patterns.
    Fix,
}

#[derive(Debug, Args)]
pub struct TagArgs {
    #[arg(value_name = "ARG")]
    pub args: Vec<String>,
    #[arg(long, value_name = "COUNT")]
    pub last: Option<usize>,
}

#[derive(Debug, Args)]
pub struct UntagArgs {
    #[arg(value_name = "ID")]
    pub command_id: i64,
    #[arg(value_name = "TAG", required = true)]
    pub tags: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TagsArgs {
    #[command(subcommand)]
    pub command: TagsCommand,
}

#[derive(Debug, Subcommand)]
pub enum TagsCommand {
    /// List all tags with command counts.
    List,
}

#[derive(Debug, Args)]
pub struct PinArgs {
    #[arg(value_name = "ID", required = true)]
    pub ids: Vec<i64>,
}

#[derive(Debug, Args)]
pub struct PinnedArgs {
    pub limit: Option<usize>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub plain: bool,
}

#[derive(Debug, Args)]
pub struct PickArgs {
    #[arg(long, short = 'n', default_value_t = 100)]
    pub limit: usize,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub failed: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub pinned: bool,
    #[arg(long)]
    pub fuzzy: bool,
}

#[derive(Debug, Args)]
pub struct TuiArgs {
    #[arg(long, short = 'n', default_value_t = 500)]
    pub limit: usize,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long)]
    pub failed: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub pinned: bool,
}

#[derive(Debug, Args)]
pub struct SnippetArgs {
    #[command(subcommand)]
    pub command: SnippetCommand,
}

#[derive(Debug, Subcommand)]
pub enum SnippetCommand {
    /// Save a reusable command snippet.
    Save(SnippetSaveArgs),
    /// List snippets.
    List,
    /// Run a snippet with placeholders replaced.
    Run(SnippetRunArgs),
    /// Delete a snippet.
    Delete(SnippetDeleteArgs),
    /// Export snippets as JSON.
    Export(SnippetExportArgs),
}

#[derive(Debug, Args)]
pub struct SnippetSaveArgs {
    pub name: String,
    pub command: String,
    #[arg(long)]
    pub desc: Option<String>,
    #[arg(long)]
    pub tags: Option<String>,
}

#[derive(Debug, Args)]
pub struct SnippetRunArgs {
    pub name: String,
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct SnippetDeleteArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct SnippetExportArgs {
    pub file: String,
}

#[derive(Debug, Args)]
pub struct ReplayArgs {
    pub id: i64,
    #[arg(long)]
    pub dry_run: bool,
    /// Skip interactive confirmation before executing the command.
    #[arg(long, short = 'y')]
    pub yes: bool,
    #[arg(long, hide = true)]
    pub confirm: bool,
    /// Reason required for policy approval on replay.
    #[arg(long)]
    pub reason: Option<String>,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    #[arg(long = "session")]
    pub sessions: Vec<String>,
    #[arg(long = "host")]
    pub hosts: Vec<String>,
    #[arg(long)]
    pub today: bool,
    #[arg(long)]
    pub yesterday: bool,
}

#[derive(Debug, Args)]
pub struct AuditArgs {
    /// Verify tamper-evident audit hash chain integrity.
    #[arg(long = "verify-chain")]
    pub verify_chain: bool,
    /// Recompute audit hash chain entries (legacy repair; requires --yes).
    #[arg(long = "rebuild-chain")]
    pub rebuild_chain: bool,
    /// Confirm destructive or repair operations without prompting.
    #[arg(long)]
    pub yes: bool,
    #[arg(long)]
    pub today: bool,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, value_enum, default_value_t = AuditFormat::Table)]
    pub format: AuditFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AuditFormat {
    Table,
    Json,
}

#[derive(Debug, Args)]
pub struct PrivateArgs {
    #[command(subcommand)]
    pub command: PrivateCommand,
}

#[derive(Debug, Subcommand)]
pub enum PrivateCommand {
    /// Enable private mode.
    On,
    /// Disable private mode.
    Off,
    /// Show private mode status.
    Status,
}

#[derive(Debug, Args)]
pub struct VaultArgs {
    #[command(subcommand)]
    pub command: VaultCommand,
}

#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// Add an encrypted command to the vault.
    Add {
        command: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// List encrypted vault entries.
    List,
    /// Decrypt and run a vault command.
    Run {
        id: i64,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a vault entry.
    Delete { id: i64 },
    /// Check vault unlock behavior.
    Unlock,
    /// Clear process-local vault state.
    Lock,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Debug, Args)]
pub struct RiskArgs {
    #[command(subcommand)]
    pub command: RiskCommand,
}

#[derive(Debug, Subcommand)]
pub enum RiskCommand {
    /// List risk detection rules.
    List,
    /// Check a single command string.
    Check {
        command: String,
        #[arg(long)]
        json: bool,
    },
    /// Scan saved history for risky commands.
    Scan(RiskScanArgs),
}

#[derive(Debug, Args)]
pub struct RiskScanArgs {
    #[arg(long)]
    pub critical: bool,
    #[arg(long)]
    pub high: bool,
    #[arg(long)]
    pub today: bool,
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: Option<ContextCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// List tracked git repositories.
    Repos(ContextListArgs),
    /// List branches recorded in history.
    Branches(ContextBranchArgs),
    /// Show command history for a git context.
    History(ContextHistoryArgs),
}

#[derive(Debug, Args)]
pub struct ContextListArgs {
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ContextBranchArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ContextHistoryArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub commit: Option<String>,
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub plain: bool,
}

#[derive(Debug, Subcommand)]
pub enum SyncCommand {
    /// Print sync status.
    Status,
    /// Store sync setup values.
    Setup { url: String, token: String },
    /// Push local history to a remote server.
    Push,
    /// Pull remote history.
    Pull,
    /// Enable sync.
    Enable,
    /// Disable sync.
    Disable,
}

#[derive(Debug, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub command: PolicyCommand,
}

#[derive(Debug, Subcommand)]
pub enum PolicyCommand {
    /// List configured policy rules.
    List,
    /// Evaluate a command against policy rules.
    Check {
        command: String,
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        env: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
pub struct TimelineArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub plain: bool,
}

#[derive(Debug, Args)]
pub struct HoldArgs {
    #[command(subcommand)]
    pub command: HoldCommand,
}

#[derive(Debug, Subcommand)]
pub enum HoldCommand {
    /// Create a legal hold.
    Add(HoldAddArgs),
    /// List legal holds.
    List,
    /// Remove a legal hold.
    Remove { id: i64 },
    /// Purge history older than retention policy.
    Purge {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Args)]
pub struct HoldAddArgs {
    pub label: String,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub command: Option<i64>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long = "git-repo")]
    pub git_repo: Option<String>,
    #[arg(long)]
    pub reason: Option<String>,
}

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long)]
    pub follow: bool,
    #[arg(long, value_enum, default_value_t = AuditFormat::Json)]
    pub format: AuditFormat,
}

#[derive(Debug, Args)]
pub struct RunbookArgs {
    #[command(subcommand)]
    pub command: RunbookCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunbookCommand {
    /// List saved runbooks.
    List,
    /// Show runbook steps.
    Show { name: String },
    /// Create a runbook from a session timeline.
    Create {
        name: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        desc: Option<String>,
    },
    /// Execute runbook steps.
    Run {
        name: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Args)]
pub struct BreakGlassArgs {
    #[command(subcommand)]
    pub command: BreakGlassCommand,
}

#[derive(Debug, Subcommand)]
pub enum BreakGlassCommand {
    /// Activate break-glass mode.
    On {
        #[arg(long)]
        reason: String,
        #[arg(long = "ttl-hours")]
        ttl_hours: Option<u64>,
    },
    /// Deactivate break-glass mode.
    Off,
    /// Show break-glass status.
    Status,
}
