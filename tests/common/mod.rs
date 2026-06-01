//! Shared integration-test helpers for environment isolation.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use mh::config::AppConfig;

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Serializes env mutations and restores `XDG_CONFIG_HOME` / `MH_CONFIG` on drop.
pub struct IsolatedConfigHome {
    _env_lock: MutexGuard,
    original_xdg: Option<String>,
    original_mh_config: Option<String>,
    original_no_cache: Option<String>,
    _temp_dir: tempfile::TempDir,
}

type MutexGuard = std::sync::MutexGuard<'static, ()>;

impl IsolatedConfigHome {
    pub fn new() -> Self {
        let env_lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let config_path = temp_dir.path().join("mh").join("config.toml");
        std::fs::create_dir_all(config_path.parent().expect("config parent"))
            .expect("config dir should be created");
        AppConfig::default()
            .write_to_path(&config_path)
            .expect("default config should be written");

        let original_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let original_mh_config = std::env::var("MH_CONFIG").ok();
        let original_no_cache = std::env::var("MH_CONFIG_NO_CACHE").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
            std::env::set_var("MH_CONFIG", &config_path);
            std::env::set_var("MH_CONFIG_NO_CACHE", "1");
        }
        AppConfig::invalidate_cache();

        Self {
            _env_lock: env_lock,
            original_xdg,
            original_mh_config,
            original_no_cache,
            _temp_dir: temp_dir,
        }
    }

    pub fn config_dir(&self) -> PathBuf {
        self._temp_dir.path().join("mh")
    }

    pub fn temp_path(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }
}

impl Drop for IsolatedConfigHome {
    fn drop(&mut self) {
        unsafe {
            match &self.original_xdg {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            match &self.original_mh_config {
                Some(value) => std::env::set_var("MH_CONFIG", value),
                None => std::env::remove_var("MH_CONFIG"),
            }
            match &self.original_no_cache {
                Some(value) => std::env::set_var("MH_CONFIG_NO_CACHE", value),
                None => std::env::remove_var("MH_CONFIG_NO_CACHE"),
            }
        }
        AppConfig::invalidate_cache();
    }
}

/// Saves and restores an arbitrary set of environment variables under the global lock.
pub struct EnvGuard {
    _lock: MutexGuard,
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    pub fn save(vars: &[&str]) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = vars
            .iter()
            .map(|key| ((*key).to_string(), std::env::var(key).ok()))
            .collect();
        Self { _lock: lock, saved }
    }

    pub fn clear_mh_env(&self) {
        unsafe {
            std::env::remove_var("MH_DB");
            std::env::remove_var("MH_CONFIG");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("SSH_CONNECTION");
            std::env::remove_var("SSH_CLIENT");
            std::env::set_var("MH_CONFIG_NO_CACHE", "1");
        }
        AppConfig::invalidate_cache();
    }

    pub fn use_isolated_config(&self, temp_dir: &tempfile::TempDir) {
        let config_path = temp_dir.path().join("mh").join("config.toml");
        std::fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
        AppConfig::default()
            .write_to_path(&config_path)
            .expect("write config");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
            std::env::set_var("MH_CONFIG", &config_path);
            std::env::set_var("MH_CONFIG_NO_CACHE", "1");
        }
        AppConfig::invalidate_cache();
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            for (key, value) in &self.saved {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
        AppConfig::invalidate_cache();
    }
}

/// Isolates `HOME` for shell init tests that write `~/.zshrc` and similar files.
pub struct IsolatedHome {
    _env_lock: MutexGuard,
    original_home: Option<String>,
    temp_dir: tempfile::TempDir,
}

impl IsolatedHome {
    pub fn new() -> Self {
        let env_lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let original_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }
        Self {
            _env_lock: env_lock,
            original_home,
            temp_dir,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        unsafe {
            match &self.original_home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}
