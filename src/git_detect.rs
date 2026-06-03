use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitContext {
    pub repo: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
}

struct CacheEntry {
    context: Option<GitContext>,
    head_marker: Option<String>,
    cached_at: Instant,
}

static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
const CACHE_TTL: Duration = Duration::from_secs(30);
const MAX_CACHE_ENTRIES: usize = 256;

/// Fast check for a `.git` directory or gitfile without spawning subprocesses.
pub fn is_git_repository(cwd: &str) -> bool {
    git_metadata_path(cwd).is_some()
}

/// Cached git context lookup; intended for the record daemon hot path.
pub fn detect_git_context_cached(cwd: &str) -> Option<GitContext> {
    if !is_git_repository(cwd) {
        return None;
    }

    let key = cwd.to_string();
    let head_marker = git_head_marker(cwd);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(entry) = guard.get(&key)
        && entry.head_marker == head_marker
        && entry.cached_at.elapsed() < CACHE_TTL
    {
        return entry.context.clone();
    }

    let context = detect_git_context(cwd);
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= MAX_CACHE_ENTRIES {
            guard.clear();
        }
        guard.insert(
            key,
            CacheEntry {
                context: context.clone(),
                head_marker,
                cached_at: Instant::now(),
            },
        );
    }
    context
}

pub fn detect_git_context(cwd: &str) -> Option<GitContext> {
    if !is_git_repository(cwd) {
        return None;
    }
    let repo = run_git(cwd, &["rev-parse", "--show-toplevel"])?;
    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let commit = run_git(cwd, &["rev-parse", "--short", "HEAD"]);

    Some(GitContext {
        repo,
        branch,
        commit,
    })
}

pub fn detect_git_context_from_env() -> Option<GitContext> {
    env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .and_then(|cwd| detect_git_context(&cwd))
}

fn git_metadata_path(cwd: &str) -> Option<PathBuf> {
    let mut path = PathBuf::from(cwd);
    if !path.is_dir() {
        return None;
    }

    loop {
        let git_path = path.join(".git");
        if git_path.is_dir() || read_gitdir_from_file(&git_path).is_some() {
            return Some(git_path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn git_head_marker(cwd: &str) -> Option<String> {
    let git_path = git_metadata_path(cwd)?;
    let head_path = if git_path.is_dir() {
        git_path.join("HEAD")
    } else {
        read_gitdir_from_file(&git_path)?.join("HEAD")
    };
    let modified = fs::metadata(&head_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(system_time_key)
        .unwrap_or_default();
    let contents = fs::read_to_string(&head_path).ok().unwrap_or_default();
    Some(format!("{modified}:{contents}"))
}

fn read_gitdir_from_file(git_file: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(git_file).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("gitdir:") {
            let gitdir = PathBuf::from(rest.trim());
            if gitdir.is_absolute() {
                return Some(gitdir);
            }
            return git_file.parent().map(|parent| parent.join(gitdir));
        }
    }
    None
}

fn system_time_key(time: SystemTime) -> u128 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn run_git(cwd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    #[test]
    fn detects_git_context_in_repository() {
        let temp_dir = crate::config::private_tempdir().expect("temp dir");
        let repo_path = temp_dir.path();
        init_git_repo(repo_path);

        let context = detect_git_context(&repo_path.to_string_lossy())
            .expect("git context should be detected");
        assert!(
            context.repo.ends_with(
                repo_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
            )
        );
        assert_eq!(context.branch.as_deref(), Some("main"));
        assert!(context.commit.is_some());
    }

    fn init_git_repo(path: &Path) {
        for (command, args) in [
            ("git", vec!["init", "-b", "main"]),
            ("git", vec!["config", "user.email", "test@example.com"]),
            ("git", vec!["config", "user.name", "test"]),
            ("git", vec!["commit", "--allow-empty", "-m", "init"]),
        ] {
            let status = Command::new(command)
                .args(&args)
                .current_dir(path)
                .status()
                .expect("git command should run");
            assert!(status.success(), "git command failed: {command} {args:?}");
        }
    }

    #[test]
    fn is_git_repository_detects_nested_worktree() {
        let temp_dir = crate::config::private_tempdir().expect("temp dir");
        let repo_path = temp_dir.path();
        init_git_repo(repo_path);

        let nested = repo_path.join("src");
        std::fs::create_dir_all(&nested).expect("nested dir");
        assert!(is_git_repository(&nested.to_string_lossy()));
    }

    #[test]
    fn is_git_repository_returns_false_outside_repo() {
        let path = format!("/mh-definitely-not-a-git-repo-{}", std::process::id());
        assert!(!is_git_repository(&path));
    }

    #[test]
    fn cached_git_context_hits_second_lookup() {
        let temp_dir = crate::config::private_tempdir().expect("temp dir");
        let repo_path = temp_dir.path();
        init_git_repo(repo_path);
        let cwd = repo_path.to_string_lossy().to_string();

        let first = detect_git_context_cached(&cwd).expect("git context");
        let second = detect_git_context_cached(&cwd).expect("cached git context");
        assert_eq!(first, second);
    }
}
