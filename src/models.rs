use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command: String,
    pub command_hash: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub session_id: Option<String>,
    pub tty: Option<String>,
    pub is_ssh: bool,
    pub is_root: bool,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub category: Option<String>,
    pub env_context: Option<String>,
    pub is_pinned: bool,
    pub is_masked: bool,
    pub tags: Vec<String>,
    pub environment_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRow {
    pub id: i64,
    pub command: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
    pub started_at: String,
    pub session_id: Option<String>,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub is_masked: bool,
    pub environment_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRow {
    pub id: i64,
    pub event_type: String,
    pub raw_command: Option<String>,
    pub reason: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub created_at: String,
    pub prev_hash: Option<String>,
    pub entry_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegalHoldRow {
    pub id: i64,
    pub label: String,
    pub session_id: Option<String>,
    pub command_id: Option<i64>,
    pub tag: Option<String>,
    pub git_repo: Option<String>,
    pub created_at: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunbookRow {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunbookStepRow {
    pub id: i64,
    pub runbook_id: i64,
    pub step_order: i32,
    pub command: String,
    pub cwd: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub id: i64,
    pub started_at: String,
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
    pub environment_tier: Option<String>,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetRow {
    pub id: i64,
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub use_count: i64,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultRow {
    pub id: i64,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EncryptedVaultRow {
    pub id: i64,
    pub encrypted_data: Vec<u8>,
    pub nonce: Vec<u8>,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SearchFilters {
    pub query: Option<String>,
    pub cwd: Option<String>,
    pub failed: bool,
    pub success: bool,
    pub user: Option<String>,
    pub shell: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub regex: bool,
    pub fuzzy: bool,
    pub fts: bool,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub pinned: bool,
    pub duration_gt: Option<i64>,
    pub duration_lt: Option<i64>,
    pub hostname: Option<String>,
    pub ssh: bool,
    pub root: bool,
    pub limit: usize,
    pub session_id: Option<String>,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub environment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsPeriod {
    All,
    Today,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatEntry {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatsSummary {
    pub period: StatsPeriod,
    pub total_commands: i64,
    pub successful_commands: i64,
    pub failed_commands: i64,
    pub average_duration_ms: Option<f64>,
    pub longest_duration_ms: Option<i64>,
    pub top_commands: Vec<StatEntry>,
    pub top_directories: Vec<StatEntry>,
    pub error_prone_commands: Vec<StatEntry>,
    pub shell_counts: Vec<StatEntry>,
    pub category_counts: Vec<StatEntry>,
    pub peak_hour: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSummary {
    pub tag: String,
    pub count: i64,
}
