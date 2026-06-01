use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub ignore: IgnoreConfig,
    #[serde(default = "default_categories")]
    pub categories: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default = "default_policy_config")]
    pub policy: PolicyConfig,
    #[serde(default = "default_retention_config")]
    pub retention: RetentionConfig,
    #[serde(default = "default_environment_config")]
    pub environment: EnvironmentConfig,
    #[serde(default)]
    pub siem: SiemConfig,
    #[serde(default)]
    pub break_glass: BreakGlassConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub max_entries: usize,
    pub ignore_duplicates: bool,
    pub ignore_space_prefix: bool,
    pub save_failed_commands: bool,
    pub save_successful_commands: bool,
    pub auto_categorize: bool,
    pub dedupe_window_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mask_secrets: bool,
    pub skip_secret_commands: bool,
    pub private_mode_env: String,
    pub audit_log: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub path: String,
    pub auto_vacuum: bool,
    pub max_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub default_limit: usize,
    pub color: bool,
    pub date_format: String,
    pub show_duration: bool,
    pub show_exit_code: bool,
    /// Rank picker results by cwd, exit code, and recency (McFly-style).
    #[serde(default = "default_true")]
    pub context_ranking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IgnoreConfig {
    pub commands: Vec<String>,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncConfig {
    pub enabled: bool,
    pub server_url: String,
    pub token: String,
    pub auto_sync_interval_minutes: u64,
    #[serde(default)]
    pub last_synced_at: String,
    #[serde(default = "default_device_id")]
    pub device_id: String,
    /// Encrypt payloads before upload (AES-256-GCM derived from sync token).
    #[serde(default = "default_true")]
    pub encrypt_payload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    pub enabled: bool,
    pub use_keyring: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub default_action: String,
    pub rules: Vec<PolicyRuleConfig>,
    /// When true, shell hooks call `mh policy check` before running interactive commands.
    #[serde(default = "default_true")]
    pub enforce_in_shell: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRuleConfig {
    pub id: String,
    pub action: String,
    pub risk_level: Option<String>,
    pub pattern: Option<String>,
    pub environment: Option<String>,
    pub hostname_pattern: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    pub enabled: bool,
    pub retention_days: u64,
    pub respect_legal_hold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub rules: Vec<EnvironmentRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentRuleConfig {
    pub tier: String,
    pub hostname_contains: Option<String>,
    pub cwd_contains: Option<String>,
    pub git_repo_contains: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SiemConfig {
    pub enabled: bool,
    pub format: String,
    pub syslog_url: Option<String>,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BreakGlassConfig {
    pub default_ttl_hours: u64,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            ignore_duplicates: true,
            ignore_space_prefix: true,
            save_failed_commands: true,
            save_successful_commands: true,
            auto_categorize: true,
            dedupe_window_seconds: 5,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mask_secrets: true,
            skip_secret_commands: false,
            private_mode_env: "MH_PRIVATE".to_string(),
            audit_log: true,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_database_path().to_string_lossy().to_string(),
            auto_vacuum: true,
            max_size_mb: 512,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_limit: 50,
            color: true,
            date_format: "%Y-%m-%d %H:%M:%S".to_string(),
            show_duration: true,
            show_exit_code: true,
            context_ranking: true,
        }
    }
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            commands: vec![
                "history".to_string(),
                "clear".to_string(),
                "exit".to_string(),
                "logout".to_string(),
                "mh record".to_string(),
            ],
            patterns: Vec::new(),
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: String::new(),
            token: String::new(),
            auto_sync_interval_minutes: 60,
            last_synced_at: String::new(),
            device_id: default_device_id(),
            encrypt_payload: true,
        }
    }
}

fn default_device_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            use_keyring: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_policy_config() -> PolicyConfig {
    PolicyConfig {
        default_action: "allow".to_string(),
        enforce_in_shell: true,
        rules: vec![
            PolicyRuleConfig {
                id: "deny-critical-prod".to_string(),
                action: "deny".to_string(),
                risk_level: Some("critical".to_string()),
                pattern: None,
                environment: Some("production".to_string()),
                hostname_pattern: None,
                message: "Critical commands are blocked in production".to_string(),
            },
            PolicyRuleConfig {
                id: "approval-critical".to_string(),
                action: "require_approval".to_string(),
                risk_level: Some("critical".to_string()),
                pattern: None,
                environment: None,
                hostname_pattern: None,
                message: "Critical commands require explicit approval".to_string(),
            },
            PolicyRuleConfig {
                id: "warn-high".to_string(),
                action: "warn".to_string(),
                risk_level: Some("high".to_string()),
                pattern: None,
                environment: None,
                hostname_pattern: None,
                message: "High risk command detected".to_string(),
            },
        ],
    }
}

fn default_retention_config() -> RetentionConfig {
    RetentionConfig {
        enabled: false,
        retention_days: 365,
        respect_legal_hold: true,
    }
}

fn default_environment_config() -> EnvironmentConfig {
    EnvironmentConfig {
        rules: vec![
            EnvironmentRuleConfig {
                tier: "production".to_string(),
                hostname_contains: Some("prod".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
            EnvironmentRuleConfig {
                tier: "staging".to_string(),
                hostname_contains: Some("stage".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
            EnvironmentRuleConfig {
                tier: "development".to_string(),
                hostname_contains: Some("dev".to_string()),
                cwd_contains: None,
                git_repo_contains: None,
            },
        ],
    }
}

impl Default for SiemConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: "syslog".to_string(),
            syslog_url: None,
            webhook_url: None,
        }
    }
}

impl Default for BreakGlassConfig {
    fn default() -> Self {
        Self {
            default_ttl_hours: 4,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            history: HistoryConfig::default(),
            security: SecurityConfig::default(),
            database: DatabaseConfig::default(),
            display: DisplayConfig::default(),
            ignore: IgnoreConfig::default(),
            categories: default_categories(),
            sync: SyncConfig::default(),
            vault: VaultConfig::default(),
            policy: default_policy_config(),
            retention: default_retention_config(),
            environment: default_environment_config(),
            siem: SiemConfig::default(),
            break_glass: BreakGlassConfig::default(),
        }
    }
}

fn default_categories() -> BTreeMap<String, Vec<String>> {
    let mut categories = BTreeMap::new();
    categories.insert(
        "git".to_string(),
        vec!["git ".to_string(), "gh ".to_string()],
    );
    categories.insert(
        "docker".to_string(),
        vec![
            "docker ".to_string(),
            "docker-compose ".to_string(),
            "podman ".to_string(),
        ],
    );
    categories.insert(
        "network".to_string(),
        vec![
            "curl ".to_string(),
            "wget ".to_string(),
            "ssh ".to_string(),
            "nc ".to_string(),
            "nmap ".to_string(),
            "ping ".to_string(),
        ],
    );
    categories.insert(
        "system".to_string(),
        vec![
            "systemctl ".to_string(),
            "journalctl ".to_string(),
            "top ".to_string(),
            "htop ".to_string(),
        ],
    );
    categories.insert(
        "package".to_string(),
        vec![
            "apt ".to_string(),
            "apt-get ".to_string(),
            "dpkg ".to_string(),
            "snap ".to_string(),
            "cargo ".to_string(),
            "pip ".to_string(),
        ],
    );
    categories
}

struct CachedConfig {
    path: PathBuf,
    modified: Option<SystemTime>,
    env_fingerprint: String,
    config: AppConfig,
}

static CONFIG_CACHE: OnceLock<RwLock<Option<CachedConfig>>> = OnceLock::new();

fn config_env_fingerprint() -> String {
    format!(
        "{}|{}|{}",
        env::var("MH_CONFIG").unwrap_or_default(),
        env::var("MH_DB").unwrap_or_default(),
        env::var("XDG_CONFIG_HOME").unwrap_or_default(),
    )
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        if env::var("MH_CONFIG_NO_CACHE").is_ok() {
            return Self::load_from_disk();
        }

        let path = config_path();
        let modified = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok());
        let env_fingerprint = config_env_fingerprint();
        let cache = CONFIG_CACHE.get_or_init(|| RwLock::new(None));

        if let Ok(read_guard) = cache.read()
            && let Some(entry) = read_guard.as_ref()
            && entry.path == path
            && entry.modified == modified
            && entry.env_fingerprint == env_fingerprint
        {
            return Ok(entry.config.clone());
        }

        let config = Self::load_from_disk()?;
        if let Ok(mut write_guard) = cache.write() {
            *write_guard = Some(CachedConfig {
                path,
                modified,
                env_fingerprint,
                config: config.clone(),
            });
        }
        Ok(config)
    }

    pub fn invalidate_cache() {
        if let Some(cache) = CONFIG_CACHE.get()
            && let Ok(mut guard) = cache.write()
        {
            *guard = None;
        }
        crate::record_engines::invalidate_cache();
    }

    fn load_from_disk() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let config = Self::default();
            config.write_to_path(&path)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("failed to parse config at {}", path.display()))
    }

    pub fn write_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            ensure_private_directory(parent)?;
        }

        let content = toml::to_string_pretty(self).context("failed to serialize default config")?;
        write_private_file(path, content.as_bytes())
            .with_context(|| format!("failed to write config at {}", path.display()))?;
        Self::invalidate_cache();
        Ok(())
    }

    pub fn database_path(&self) -> Result<PathBuf> {
        if let Ok(path) = env::var("MH_DB") {
            return Ok(expand_tilde(&path));
        }

        if self.database.path.trim().is_empty() {
            if env::var("MH_DB").is_ok() {
                bail!("MH_DB is set but empty; unset it or provide a valid database file path");
            }
            bail!(
                "database.path must not be empty; set [database].path in config or use MH_DB override"
            );
        }

        Ok(expand_tilde(&self.database.path))
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = env::var("MH_CONFIG") {
        return expand_tilde(&path);
    }

    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mh")
        .join("config.toml")
}

pub fn default_database_path() -> PathBuf {
    if let Ok(path) = env::var("MH_DB") {
        return expand_tilde(&path);
    }

    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mh")
        .join("history.db")
}

pub fn private_mode_path() -> PathBuf {
    config_path()
        .parent()
        .map(|path| path.join("private"))
        .unwrap_or_else(|| PathBuf::from(".mh-private"))
}

/// Warnings when `MH_DB` or the configured database path may cross user/privilege boundaries.
pub fn database_path_warnings(path: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let current_uid = unsafe { libc::geteuid() };
        let current_user = whoami::username();
        if path.exists()
            && let Ok(meta) = fs::metadata(path)
            && meta.uid() != current_uid
        {
            warnings.push(format!(
                "database file is owned by uid {} but current euid is {current_uid}",
                meta.uid()
            ));
        }

        if let Some(path_str) = path.to_str() {
            if let Some(rest) = path_str.strip_prefix("/home/")
                && let Some(owner) = rest.split('/').next().filter(|name| !name.is_empty())
                && owner != current_user
            {
                warnings.push(format!(
                    "database path is under /home/{owner} but current user is {current_user}"
                ));
            } else if path_str.starts_with("/root/") && current_uid != 0 {
                warnings
                    .push("database path is under /root but current user is not root".to_string());
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    warnings
}

pub fn restrict_file_permissions(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to read permissions for {}", path.display()))?
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to restrict permissions for {}", path.display()))?;
    }

    Ok(())
}

/// Restricts a directory to owner-only access (mode 0700).
pub fn restrict_directory_permissions(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to read permissions for {}", path.display()))?
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).with_context(|| {
            format!(
                "failed to restrict directory permissions for {}",
                path.display()
            )
        })?;
    }

    Ok(())
}

/// Creates a private temporary directory for tests and benchmarks (mode 0700 on Unix).
pub fn private_tempdir() -> Result<tempfile::TempDir> {
    let dir = tempfile::tempdir().context("failed to create temporary directory")?;
    restrict_directory_permissions(dir.path())?;
    Ok(dir)
}

/// Creates a directory and applies owner-only permissions.
pub fn ensure_private_directory(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    restrict_directory_permissions(path)
}

/// Creates a missing application data directory privately, but refuses insecure existing parents.
///
/// This avoids changing permissions on broad directories such as /tmp or $HOME while still
/// preventing database/socket files from being created in group/world-writable locations.
pub fn ensure_secure_data_directory(path: &Path, label: &str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }

    match path.symlink_metadata() {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("{label} directory is a symlink: {}", path.display());
            }
            if !metadata.is_dir() {
                anyhow::bail!("{label} path is not a directory: {}", path.display());
            }
            ensure_not_group_or_other_writable(path, label)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).with_context(|| {
                format!("failed to create {label} directory {}", path.display())
            })?;
            restrict_directory_permissions(path)
        }
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect {label} directory {}", path.display())),
    }
}

/// Returns true when group or others can write to the directory (Unix only).
#[cfg(unix)]
pub fn is_group_or_other_writable(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .with_context(|| format!("failed to read permissions for {}", path.display()))?
        .permissions()
        .mode();
    Ok(mode & 0o022 != 0)
}

#[cfg(not(unix))]
pub fn is_group_or_other_writable(_path: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn ensure_not_group_or_other_writable(path: &Path, label: &str) -> Result<()> {
    if is_group_or_other_writable(path)? {
        anyhow::bail!(
            "{label} directory {} is writable by group or others; choose a private directory",
            path.display()
        );
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_not_group_or_other_writable(_path: &Path, _label: &str) -> Result<()> {
    Ok(())
}

/// Creates a directory without changing permissions on an existing parent.
pub fn ensure_directory(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))
}

/// Refuses when the path already exists as a symlink.
pub fn ensure_not_symlink(path: &Path) -> Result<()> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("refusing to write through symlink: {}", path.display());
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Writes atomically with owner-only permissions; refuses symlink destinations on Unix.
pub fn write_private_file(path: &Path, content: &[u8]) -> Result<()> {
    ensure_not_symlink(path)?;

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_directory(parent)?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("export");
        let temp_path = path.with_file_name(format!(
            ".{file_name}.mh-{}-{}.tmp",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp_path)
                .with_context(|| {
                    format!("failed to create temporary file {}", temp_path.display())
                })?;
            file.write_all(content)
                .with_context(|| format!("failed to write payload to {}", temp_path.display()))?;
            file.sync_all()?;
        }
        restrict_file_permissions(&temp_path)?;
        ensure_not_symlink(path)?;
        if path.symlink_metadata().is_ok() {
            fs::remove_file(path)?;
        }
        fs::rename(&temp_path, path)
            .with_context(|| format!("failed to finalize write to {}", path.display()))?;
        restrict_file_permissions(path)?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, content)
            .with_context(|| format!("failed to write file {}", path.display()))?;
        Ok(())
    }
}

/// Copies a file without following a symlink at the destination path.
pub fn copy_file_safely(source: &Path, destination: &Path) -> Result<()> {
    let content =
        fs::read(source).with_context(|| format!("failed to read file {}", source.display()))?;
    write_private_file(destination, &content)
}

pub fn has_restricted_permissions(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path)
            .with_context(|| format!("failed to read permissions for {}", path.display()))?
            .permissions()
            .mode();
        Ok(mode & 0o077 == 0)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(true)
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(path));
    }

    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(path)
}

/// Legacy broad ignore patterns removed from defaults in v0.1+.
pub const LEGACY_BROAD_IGNORE_PATTERNS: &[&str] = &[
    ".*password.*",
    ".*token.*",
    ".*secret.*",
    ".*api[_-]?key.*",
    ".*bearer.*",
];

pub fn legacy_ignore_pattern_warnings(config: &AppConfig) -> Vec<String> {
    config
        .ignore
        .patterns
        .iter()
        .filter(|pattern| LEGACY_BROAD_IGNORE_PATTERNS.contains(&pattern.as_str()))
        .map(|pattern| {
            format!(
                "ignore.patterns contains legacy broad rule `{pattern}` — it bypasses security masking; remove it and rely on [security] settings"
            )
        })
        .collect()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigFixReport {
    pub tightened_config_dir: bool,
    pub tightened_config_file: bool,
    pub removed_legacy_patterns: usize,
}

pub fn has_restricted_directory_permissions(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path)
            .with_context(|| format!("failed to read permissions for {}", path.display()))?
            .permissions()
            .mode();
        Ok(mode & 0o077 == 0)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(true)
    }
}

/// Tightens config file permissions and removes legacy broad ignore patterns.
pub fn fix_local_config() -> Result<ConfigFixReport> {
    let path = config_path();
    let mut config = AppConfig::load()?;
    let mut report = ConfigFixReport::default();

    let before_len = config.ignore.patterns.len();
    config
        .ignore
        .patterns
        .retain(|pattern| !LEGACY_BROAD_IGNORE_PATTERNS.contains(&pattern.as_str()));
    report.removed_legacy_patterns = before_len.saturating_sub(config.ignore.patterns.len());

    if report.removed_legacy_patterns > 0 || !path.exists() {
        config.write_to_path(&path)?;
        report.tightened_config_dir = true;
        report.tightened_config_file = true;
        return Ok(report);
    }

    if let Some(parent) = path.parent() {
        if parent.exists() {
            if !has_restricted_directory_permissions(parent)? {
                restrict_directory_permissions(parent)?;
                report.tightened_config_dir = true;
            }
        } else {
            ensure_private_directory(parent)?;
            report.tightened_config_dir = true;
        }
    }

    if path.exists() && !has_restricted_permissions(&path)? {
        restrict_file_permissions(&path)?;
        report.tightened_config_file = true;
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn fills_missing_top_level_sections_with_defaults() {
        let config: AppConfig = toml::from_str("").expect("empty config should use defaults");

        assert_eq!(config.history.max_entries, 100_000);
        assert!(!config.sync.enabled);
        assert_eq!(config.sync.auto_sync_interval_minutes, 60);
        assert!(!config.vault.enabled);
        assert!(config.vault.use_keyring);
        assert!(config.categories.contains_key("git"));
    }

    #[test]
    fn partial_database_section_uses_defaults() {
        let config: AppConfig =
            toml::from_str("[database]\npath = \"/tmp/history.db\"\n").expect("partial database");
        assert_eq!(config.database.path, "/tmp/history.db");
        assert!(config.database.auto_vacuum);
        assert_eq!(config.database.max_size_mb, 512);
    }

    #[test]
    fn detects_legacy_ignore_patterns() {
        let mut config = AppConfig::default();
        config.ignore.patterns.push(".*password.*".to_string());
        let warnings = legacy_ignore_pattern_warnings(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("legacy broad rule"));
    }

    #[test]
    #[cfg(unix)]
    fn fix_local_config_tightens_permissions_and_removes_legacy_patterns() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let temp_dir = private_tempdir().expect("temp dir");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }
        AppConfig::invalidate_cache();

        let config_dir = temp_dir.path().join("mh");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o755))
            .expect("chmod config dir");

        let path = config_dir.join("config.toml");
        let mut config = AppConfig::default();
        config.ignore.patterns.push(".*password.*".to_string());
        std::fs::write(&path, toml::to_string_pretty(&config).expect("serialize"))
            .expect("write config");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("chmod config");

        let report = fix_local_config().expect("fix config");
        assert!(report.tightened_config_dir);
        assert!(report.tightened_config_file);
        assert_eq!(report.removed_legacy_patterns, 1);

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        unsafe {
            match original_xdg {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
        AppConfig::invalidate_cache();
    }

    #[test]
    fn writes_config_with_restrictive_permissions() {
        let temp_dir = private_tempdir().expect("temp dir");
        let path = temp_dir.path().join("config.toml");
        AppConfig::default()
            .write_to_path(&path)
            .expect("config should be written");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path)
                .expect("config metadata should be readable")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    #[cfg(unix)]
    fn write_private_file_rejects_symlink_destination() {
        let temp_dir = private_tempdir().expect("temp dir");
        let target = temp_dir.path().join("target.json");
        let link = temp_dir.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let result = write_private_file(&link, b"{}");
        let error = result.expect_err("symlink write should fail");
        assert!(
            format!("{error:#}").contains("symlink"),
            "expected symlink rejection"
        );
    }

    #[test]
    #[cfg(unix)]
    fn write_private_file_does_not_chmod_existing_parent_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = private_tempdir().expect("temp dir");
        let parent = temp_dir.path().join("exports");
        std::fs::create_dir_all(&parent).expect("parent dir");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))
            .expect("chmod parent");

        write_private_file(&parent.join("history.json"), b"[]").expect("write export");

        let parent_mode = fs::metadata(&parent)
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(parent_mode, 0o755);

        let file_mode = fs::metadata(parent.join("history.json"))
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
    }

    #[test]
    #[cfg(unix)]
    fn database_path_warnings_detect_foreign_home() {
        let current = whoami::username();
        let foreign = if current == "mh_foreign_db_user" {
            "otheruser"
        } else {
            "mh_foreign_db_user"
        };
        let path = PathBuf::from(format!("/home/{foreign}/.local/share/mh/history.db"));
        let warnings = database_path_warnings(&path);
        assert!(
            warnings.iter().any(|warning| warning.contains(foreign)),
            "expected foreign home warning for {foreign}, got: {warnings:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn secure_data_directory_does_not_chmod_existing_private_parent() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = private_tempdir().expect("temp dir");
        let parent = temp_dir.path().join("existing");
        std::fs::create_dir_all(&parent).expect("parent dir");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))
            .expect("chmod parent");

        ensure_secure_data_directory(&parent, "database parent").expect("secure parent");

        let mode = fs::metadata(&parent)
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    #[cfg(unix)]
    fn secure_data_directory_rejects_world_writable_parent() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = private_tempdir().expect("temp dir");
        let parent = temp_dir.path().join("unsafe");
        std::fs::create_dir_all(&parent).expect("parent dir");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
            .expect("chmod parent");

        let error = ensure_secure_data_directory(&parent, "database parent")
            .expect_err("world-writable parent should fail");
        assert!(format!("{error:#}").contains("writable by group or others"));
    }

    #[test]
    #[cfg(unix)]
    fn is_group_or_other_writable_detects_insecure_modes() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = private_tempdir().expect("temp dir");
        let parent = temp_dir.path().join("modes");
        std::fs::create_dir_all(&parent).expect("parent dir");

        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .expect("chmod parent");
        assert!(!is_group_or_other_writable(&parent).expect("inspect"));

        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
            .expect("chmod parent");
        assert!(is_group_or_other_writable(&parent).expect("inspect"));
    }
}
