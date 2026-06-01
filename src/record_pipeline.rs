use std::env;
use std::path::Path;

use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::classifier;
use crate::config::AppConfig;
use crate::db::Database;
use crate::environment;
use crate::errors::MhError;
use crate::git_detect::{self, GitContext};
use crate::identity;
use crate::models::CommandRecord;
use crate::policy::PolicyAction;
use crate::record_engines;
use crate::risk;
use crate::security::{self, SecurityAction};

/// Payload for recording a command (CLI, daemon, and tests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPayload {
    pub command: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub session_id: Option<String>,
    pub tty: Option<String>,
    pub tags: Option<String>,
    pub env_context: Option<String>,
}

impl From<&crate::cli::RecordArgs> for RecordPayload {
    fn from(args: &crate::cli::RecordArgs) -> Self {
        Self {
            command: args.command.clone(),
            cwd: args.cwd.clone(),
            shell: args.shell.clone(),
            exit_code: args.exit_code,
            duration_ms: args.duration_ms,
            started_at: args.started_at.clone(),
            finished_at: args.finished_at.clone(),
            session_id: args.session_id.clone(),
            tty: args.tty.clone(),
            tags: args.tags.clone(),
            env_context: args.env_context.clone(),
        }
    }
}

/// Controls record-path behavior for CLI vs daemon callers.
#[derive(Debug, Clone)]
pub struct RecordOptions {
    pub skip_git_detect: bool,
    pub use_git_cache: bool,
    /// When set, skips git subprocess lookup (daemon precomputes this before DB lock).
    pub precomputed_git: Option<Option<GitContext>>,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            skip_git_detect: env::var("MH_SKIP_GIT_DETECT").is_ok(),
            use_git_cache: false,
            precomputed_git: None,
        }
    }
}

impl RecordOptions {
    pub fn for_daemon() -> Self {
        Self {
            skip_git_detect: false,
            use_git_cache: true,
            precomputed_git: None,
        }
    }

    pub fn with_precomputed_git(mut self, git: Option<GitContext>) -> Self {
        self.precomputed_git = Some(git);
        self
    }
}

pub fn execute(config: &AppConfig, database: &Database, payload: &RecordPayload) -> Result<()> {
    execute_with_options(config, database, payload, RecordOptions::default())
}

pub fn execute_with_options(
    config: &AppConfig,
    database: &Database,
    payload: &RecordPayload,
    options: RecordOptions,
) -> Result<()> {
    let username = Some(whoami::username());
    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());

    let (security_engine, policy) = record_engines::engines_for(config)?;
    let decision = security_engine.process(&payload.command, config)?;
    let audit_command = security::command_for_audit(&payload.command, &decision, config)?;
    match &decision.action {
        SecurityAction::Skipped(reason) => {
            if config.security.audit_log {
                let row = database.insert_audit_log(
                    "skipped",
                    &audit_command,
                    reason,
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(config, &row);
            }
            return Ok(());
        }
        SecurityAction::Masked if config.security.audit_log => {
            let row = database.insert_audit_log(
                "masked",
                &audit_command,
                "command contains sensitive data",
                username.as_deref(),
                hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(config, &row);
        }
        _ => {}
    }

    if let Some(exit_code) = payload.exit_code {
        if exit_code == 0 && !config.history.save_successful_commands {
            return Ok(());
        }
        if exit_code != 0 && !config.history.save_failed_commands {
            return Ok(());
        }
    }

    let cwd = match &payload.cwd {
        Some(cwd) => Some(cwd.clone()),
        None => env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
    };

    let git = match &options.precomputed_git {
        Some(context) => context.clone(),
        None if options.skip_git_detect => None,
        None if options.use_git_cache => cwd
            .as_deref()
            .and_then(git_detect::detect_git_context_cached),
        None => cwd.as_deref().and_then(git_detect::detect_git_context),
    };
    let environment_tier = Some(environment::classify_label(
        config,
        hostname.as_deref(),
        cwd.as_deref(),
        git.as_ref().map(|context| context.repo.as_str()),
    ));

    let started_at =
        crate::timestamp::parse_optional_rfc3339("started_at", payload.started_at.as_ref())?
            .unwrap_or_else(now_rfc3339);
    let finished_at =
        crate::timestamp::parse_optional_rfc3339("finished_at", payload.finished_at.as_ref())?;

    let policy_decision = policy.evaluate(
        &decision.command,
        hostname.as_deref(),
        environment_tier.as_deref(),
    );
    match policy_decision.action {
        PolicyAction::Deny => {
            if config.security.audit_log {
                let row = database.insert_audit_log(
                    "policy_violation",
                    &audit_command,
                    &policy_decision.message,
                    username.as_deref(),
                    hostname.as_deref(),
                )?;
                crate::siem::emit_audit_event(config, &row);
            }
            return Err(MhError::PolicyDenied(policy_decision.message).into());
        }
        PolicyAction::Warn if config.security.audit_log => {
            let row = database.insert_audit_log(
                "policy_warn",
                &audit_command,
                &policy_decision.message,
                username.as_deref(),
                hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(config, &row);
        }
        PolicyAction::RequireApproval if config.security.audit_log => {
            let row = database.insert_audit_log(
                "policy_approval",
                &audit_command,
                &policy_decision.message,
                username.as_deref(),
                hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(config, &row);
        }
        _ => {}
    }

    if database.exceeds_size_limit(config.database.max_size_mb)? {
        if config.security.audit_log {
            let row = database.insert_audit_log(
                "skipped",
                &audit_command,
                "database size limit reached",
                username.as_deref(),
                hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(config, &row);
        }
        return Ok(());
    }

    let category = if config.history.auto_categorize {
        classifier::classify_command(&decision.command, &config.categories)
    } else {
        None
    };

    let record = CommandRecord {
        command_hash: hash_command(&decision.command),
        command: decision.command,
        cwd,
        shell: payload.shell.clone().or_else(detect_shell),
        username,
        hostname,
        exit_code: payload.exit_code,
        duration_ms: payload.duration_ms.map(|duration| duration.max(0)),
        started_at,
        finished_at,
        session_id: payload
            .session_id
            .clone()
            .or_else(|| env::var("MH_SESSION_ID").ok())
            .or_else(|| Some(Uuid::new_v4().to_string())),
        tty: payload.tty.clone().or_else(|| env::var("TTY").ok()),
        is_ssh: env::var("SSH_CONNECTION").is_ok() || env::var("SSH_CLIENT").is_ok(),
        is_root: identity::is_effective_root(),
        git_repo: git.as_ref().map(|context| context.repo.clone()),
        git_branch: git.as_ref().and_then(|context| context.branch.clone()),
        git_commit: git.as_ref().and_then(|context| context.commit.clone()),
        category,
        env_context: payload.env_context.clone().or_else(detect_env_context),
        is_pinned: false,
        is_masked: matches!(decision.action, SecurityAction::Masked),
        tags: parse_tags(payload.tags.as_deref()),
        environment_tier,
    };

    let inserted_id = if config.history.ignore_duplicates {
        database
            .insert_command_unless_recent_duplicate(&record, config.history.dedupe_window_seconds)?
    } else {
        Some(database.insert_command(&record)?)
    };

    if inserted_id.is_none() {
        return Ok(());
    }

    if let Some(inserted_id) = inserted_id {
        if config.security.audit_log
            && let Some(assessment) = risk::assess_command(&record.command)
        {
            let row = database.insert_audit_log(
                "risky",
                &audit_command,
                &format!("{} ({})", assessment.description, assessment.level.label()),
                record.username.as_deref(),
                record.hostname.as_deref(),
            )?;
            crate::siem::emit_audit_event(config, &row);
        }
        database.maybe_enforce_max_entries(
            config.history.max_entries,
            config.database.auto_vacuum,
            inserted_id,
        )?;
    }
    Ok(())
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn hash_command(command: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn detect_shell() -> Option<String> {
    env::var("SHELL").ok().and_then(|path| {
        Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
    })
}

fn parse_tags(tags: Option<&str>) -> Vec<String> {
    tags.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn detect_env_context() -> Option<String> {
    if env::var("container").is_ok() || Path::new("/.dockerenv").exists() {
        return Some("docker".to_string());
    }
    if env::var("VIRTUAL_ENV").is_ok() {
        return Some("virtualenv".to_string());
    }
    if env::var("IN_NIX_SHELL").is_ok() {
        return Some("nix-shell".to_string());
    }
    None
}
