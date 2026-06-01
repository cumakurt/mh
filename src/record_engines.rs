use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;

use anyhow::Result;

use crate::config::{AppConfig, config_path};
use crate::policy::PolicyEngine;
use crate::security::SecurityEngine;

struct CachedEngines {
    config_path: PathBuf,
    config_mtime: Option<SystemTime>,
    config_fingerprint: u64,
    security: Arc<SecurityEngine>,
    policy: Arc<PolicyEngine>,
}

static CACHE: OnceLock<RwLock<Option<CachedEngines>>> = OnceLock::new();

fn engine_config_fingerprint(config: &AppConfig) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    toml::to_string(&config.security)?.hash(&mut hasher);
    toml::to_string(&config.policy)?.hash(&mut hasher);
    toml::to_string(&config.ignore)?.hash(&mut hasher);
    Ok(hasher.finish())
}

/// Returns cached security and policy engines keyed on config path, mtime, and engine-relevant fields.
pub fn engines_for(config: &AppConfig) -> Result<(Arc<SecurityEngine>, Arc<PolicyEngine>)> {
    let path = config_path();
    let modified = fs::metadata(&path)
        .ok()
        .and_then(|meta| meta.modified().ok());
    let fingerprint = engine_config_fingerprint(config)?;
    let cache = CACHE.get_or_init(|| RwLock::new(None));

    if let Ok(read_guard) = cache.read()
        && let Some(entry) = read_guard.as_ref()
        && entry.config_path == path
        && entry.config_mtime == modified
        && entry.config_fingerprint == fingerprint
    {
        return Ok((Arc::clone(&entry.security), Arc::clone(&entry.policy)));
    }

    let security = Arc::new(SecurityEngine::from_config(config)?);
    let policy = Arc::new(PolicyEngine::from_config(config)?);
    if let Ok(mut write_guard) = cache.write() {
        *write_guard = Some(CachedEngines {
            config_path: path,
            config_mtime: modified,
            config_fingerprint: fingerprint,
            security: Arc::clone(&security),
            policy: Arc::clone(&policy),
        });
    }

    Ok((security, policy))
}

pub fn invalidate_cache() {
    if let Some(cache) = CACHE.get()
        && let Ok(mut guard) = cache.write()
    {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_same_engine_instance_for_repeated_lookups() {
        invalidate_cache();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_dir = temp_dir.path().join("mh");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        let config_path = config_dir.join("config.toml");
        AppConfig::default()
            .write_to_path(&config_path)
            .expect("write config");

        let saved = [
            ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
            ("MH_CONFIG", std::env::var("MH_CONFIG").ok()),
            (
                "MH_CONFIG_NO_CACHE",
                std::env::var("MH_CONFIG_NO_CACHE").ok(),
            ),
        ];
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
            std::env::set_var("MH_CONFIG", &config_path);
            std::env::set_var("MH_CONFIG_NO_CACHE", "1");
        }

        let config = AppConfig::load().expect("load config");
        let (first_security, first_policy) = engines_for(&config).expect("engines");
        let (second_security, second_policy) = engines_for(&config).expect("engines");
        assert!(Arc::ptr_eq(&first_security, &second_security));
        assert!(Arc::ptr_eq(&first_policy, &second_policy));

        unsafe {
            for (key, value) in saved {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
