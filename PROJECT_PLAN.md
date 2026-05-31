# Modern Linux History Management Tool — Rust Project Plan (v2.0)

> **⚠️ CRITICAL RULE: All source code, variable names, function names, struct names, comments, commit messages, and documentation strings MUST be written in English. Turkish is only permitted in this planning document. No exceptions.**

---

## 1. Project Purpose

The goal of this project is to build a modern, secure, fast, and feature-rich command history management tool that can replace the classic `history` command on Linux systems.

The application must support the following shell environments:

* Bash
* Zsh
* Fish
* Root shell
* SSH sessions
* Multiple user profiles
* Nushell *(new)*
* PowerShell on Linux *(new — optional future target)*

The project goal is not merely to list commands. It is to make command history **meaningful, searchable, filterable, secure, auditable, and analytically useful**.

---

## 2. Technology Stack

### Programming Language

**Rust** must be used.

Reasons for choosing Rust:

* Distributable as a single binary — no runtime dependency.
* High performance, even under millions of records.
* Memory-safe by design — no buffer overflows or use-after-free.
* Excellent for CLI tooling and Linux system programming.
* Can evolve into a professional-grade product over time.
* Cross-compilation support for ARM and x86_64.

> **Code Language Rule:** Every identifier, comment, doc-comment (`///`), module name, file name, test name, error message string, and git commit message MUST be in English. Example:
>
> ✅ `fn record_command(entry: &CommandEntry) -> Result<()>`  
> ❌ `fn komutu_kaydet(girdi: &KomutGirdisi) -> Result<()>`

### Database

**SQLite** must be used.

Reasons for SQLite:

* Single-file operation — easy to backup and migrate.
* No installation required.
* Fast indexed queries.
* Strong full-text search with FTS5.
* Ideal for CLI tools with local-first architecture.

### Recommended Rust Crates

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
dirs = "5"
uuid = { version = "1", features = ["v4"] }
regex = "1"
anyhow = "1"
thiserror = "1"
colored = "2"
comfy-table = "7"
ratatui = "0.26"
crossterm = "0.27"
hostname = "0.4"
whoami = "1"
shell-words = "1"
fuzzy-matcher = "0.3"       # NEW: fuzzy search engine
indicatif = "0.17"          # NEW: progress bars for long operations
arboard = "3"               # NEW: clipboard support in TUI
notify = "6"                # NEW: filesystem watcher for live reload
zstd = "0.13"               # NEW: compressed export/backup
sha2 = "0.10"               # NEW: command hashing (SHA-256)
aes-gcm = "0.10"            # NEW: encrypted vault support
keyring = "2"               # NEW: OS keyring integration
reqwest = { version = "0.11", features = ["json", "rustls-tls"], optional = true }  # NEW: sync feature
tokio = { version = "1", features = ["full"], optional = true }  # NEW: async runtime for sync
```

---

## 3. Project Name

Suggested names:

* `mh`
* `modern-history`
* `histx`
* `rhist`
* `cmdvault`
* `shelltrack`

This plan uses `mh` as the canonical command name.

---

## 4. Core Features

### 4.1 Command Recording

The application must record every executed command.

Fields to record:

* Command text
* Working directory (`cwd`)
* Username
* Hostname
* Shell type
* Session ID
* Start timestamp
* End timestamp
* Duration in milliseconds
* Exit code
* Whether it is an SSH session
* TTY device
* Whether root user
* Whether inside a Git repository
* Active Git branch
* Git commit hash at time of recording *(new)*
* Command hash (SHA-256)
* Environment tags *(new — e.g. `docker`, `virtualenv`, `nix-shell`)*
* Custom user-defined tags *(new)*
* Command category (auto-classified: `network`, `filesystem`, `git`, `docker`, etc.) *(new)*

Example record:

```json
{
  "command": "docker ps -a",
  "cwd": "/opt/project",
  "shell": "zsh",
  "user": "root",
  "hostname": "kali",
  "exit_code": 0,
  "duration_ms": 142,
  "started_at": "2026-05-31T17:30:00Z",
  "session_id": "8d9f2f",
  "git_branch": "main",
  "git_commit": "a3f9b1c",
  "category": "docker",
  "tags": ["pentest", "recon"],
  "env_context": "docker"
}
```

---

### 4.2 Shell Integration

The application must operate via shell hook mechanisms.

Supported shells:

* Bash
* Zsh
* Fish
* Nushell *(new)*

Commands:

```bash
mh init bash
mh init zsh
mh init fish
mh init nushell
```

These commands generate the integration code for the relevant shell.

Example usage:

```bash
eval "$(mh init zsh)"
```

Or permanently in `.zshrc`:

```bash
eval "$(mh init zsh)"
```

---

### 4.3 Bash Hook

Bash integration goals:

* Capture timestamp before command starts.
* Capture exit code after command completes.
* Write the command to the database.

Bash integration mechanisms:

```bash
PROMPT_COMMAND
DEBUG trap
history 1
```

Important considerations:

* `PROMPT_COMMAND` must not be overwritten — it must be appended.
* Must function independently for the root user.
* Handle the case where `sudo su` spawns a different shell.

---

### 4.4 Zsh Hook

Since Kali Linux primarily uses Zsh, Zsh support is the highest priority.

Available Zsh hooks:

```zsh
preexec()
precmd()
```

Goals:

* `preexec`: Capture the command text and timestamp before execution.
* `precmd`: Capture exit code and duration after execution.

Example concept:

```zsh
preexec() {
  MH_LAST_COMMAND="$1"
  MH_START_TIME=$(date +%s%3N)
}

precmd() {
  MH_EXIT_CODE=$?
  MH_END_TIME=$(date +%s%3N)
  mh record \
    --command "$MH_LAST_COMMAND" \
    --exit-code "$MH_EXIT_CODE" \
    --duration-ms "$((MH_END_TIME - MH_START_TIME))"
}
```

---

### 4.5 Fish Hook

Fish uses an event-based model:

```fish
function mh_preexec --on-event fish_preexec
  set -g MH_LAST_CMD $argv[1]
  set -g MH_START_TIME (date +%s%3N)
end

function mh_postexec --on-event fish_postexec
  set exit_code $status
  mh record --command "$MH_LAST_CMD" --exit-code $exit_code
end
```

---

### 4.6 Nushell Hook *(New)*

Nushell requires a hook block in `config.nu`:

```nu
$env.config = {
  hooks: {
    pre_prompt: [{ mh record --command $env.CMD_DURATION_MS }]
  }
}
```

---

## 5. Command-Line Interface

Main command:

```bash
mh
```

Subcommands:

```bash
mh init           # Shell integration setup
mh record         # Record a command (called by shell hooks)
mh search         # Search through history
mh last           # Show recent commands
mh stats          # Show usage statistics
mh delete         # Delete specific records
mh clear          # Clear all history
mh export         # Export history to file
mh import         # Import history from file
mh doctor         # System health check
mh config         # Configuration management
mh tui            # Launch interactive TUI
mh tag            # NEW: Tag commands
mh pin            # NEW: Pin important commands
mh snippet        # NEW: Save reusable command snippets
mh replay         # NEW: Re-execute a command by ID
mh diff           # NEW: Compare history across sessions or hosts
mh audit          # NEW: Security audit log
mh private        # NEW: Toggle private mode
mh sync           # NEW: Sync history to remote (optional)
mh vault          # NEW: Encrypted command vault
```

---

### 5.1 `mh init`

Generates shell integration code.

Usage:

```bash
mh init zsh
mh init bash
mh init fish
mh init nushell
```

Features:

* Generates shell-specific hooks.
* Displays instructions for `.zshrc`, `.bashrc`, or fish config.
* Optional automatic installation:

```bash
mh init zsh --install
```

---

### 5.2 `mh record`

Called internally by shell hooks. Not typically used by the user directly.

```bash
mh record \
  --command "docker ps" \
  --cwd "/opt/app" \
  --shell "zsh" \
  --exit-code 0 \
  --duration-ms 123 \
  --tags "pentest,recon"
```

Responsibilities:

* Validate the command.
* Detect and mask sensitive data.
* Apply ignore rules.
* Auto-classify command category.
* Write to SQLite database.

---

### 5.3 `mh search`

Searches through history.

```bash
mh search docker
mh search "ssh root"
mh search --cwd /opt/project
mh search --failed
mh search --success
mh search --user root
mh search --shell zsh
mh search --after 2026-05-01
mh search --before 2026-05-31
mh search --regex "docker .*"
mh search --fuzzy "dkr ps"
mh search --tag pentest
mh search --category git
mh search --pinned
mh search --duration-gt 5000
```

Supported filters:

* Command text (exact, substring, regex, fuzzy)
* Date range
* User
* Hostname
* Shell
* Working directory
* Exit code
* Duration
* Success / failure
* SSH sessions
* Sudo usage
* Tags *(new)*
* Category *(new)*
* Pinned commands *(new)*
* Minimum/maximum duration *(new)*

---

### 5.4 `mh last`

Shows recent commands.

```bash
mh last
mh last 20
mh last --failed
mh last --cwd .
mh last --session
mh last --json
```

Defaults to the last 50 records.

---

### 5.5 `mh stats`

Generates usage statistics.

```bash
mh stats
mh stats --today
mh stats --week
mh stats --month
mh stats --category
mh stats --heatmap
mh stats --top 20
```

Output includes:

* Total command count
* Most frequently used commands
* Most frequently used directories
* Most error-prone commands
* Average command duration
* Longest-running commands
* Usage breakdown by shell
* Usage breakdown by user
* Usage breakdown by host
* Daily command count over time
* Command category distribution *(new)*
* Hourly activity heatmap *(new)*
* Peak productivity hours *(new)*
* Command success rate over time *(new)*

Example output:

```text
Total commands    : 12,430
Today's commands  : 342
Top command       : ls (1,204 uses)
Top directory     : /opt/projects
Error rate        : 7.4%
Avg duration      : 84 ms
Peak hour         : 14:00–15:00
Top category      : git (32%)
```

---

### 5.6 `mh delete`

Deletes specific records.

```bash
mh delete 152
mh delete --older-than 90d
mh delete --contains password
mh delete --failed
mh delete --tag temp
```

Requires confirmation before deletion.

Force delete:

```bash
mh delete 152 --yes
```

---

### 5.7 `mh clear`

Clears all history.

```bash
mh clear
mh clear --user root
mh clear --before 2026-01-01
mh clear --keep-pinned
```

This command must always require confirmation.

---

### 5.8 `mh export`

Exports history data.

Supported formats:

* JSON
* CSV
* SQLite dump
* Markdown *(new)*
* Compressed archive (`.json.zst`) *(new)*

```bash
mh export --json history.json
mh export --csv history.csv
mh export --markdown history.md
mh export --compressed backup.json.zst
mh export --after 2026-01-01 --json filtered.json
```

---

### 5.9 `mh import`

Imports previously exported history.

```bash
mh import history.json
mh import history.csv
mh import backup.json.zst
mh import --merge           # merge without overwriting
mh import --dry-run         # preview what would be imported
```

---

### 5.10 `mh doctor`

Performs system health checks.

Checks performed:

* Database file exists and is not corrupted.
* Write permissions are available.
* Shell integration is active.
* Config file is readable.
* Hook is functioning correctly.
* `mh` binary is in `$PATH`.
* Root and regular user separation is correct.
* Database schema is up-to-date. *(new)*
* Pending migrations exist. *(new)*
* Database size and record count. *(new)*

Example output:

```text
[OK]   Database found at ~/.local/share/mh/history.db
[OK]   Config found at ~/.config/mh/config.toml
[OK]   Shell detected: zsh
[OK]   Hook is active (preexec/precmd)
[WARN] Root shell integration not installed
[OK]   Write permission available
[OK]   Database schema is current (version 5)
[INFO] Database size: 4.2 MB — 12,430 records
[OK]   Binary is in PATH: /usr/local/bin/mh
```

---

### 5.11 `mh config`

Configuration management.

```bash
mh config show
mh config path
mh config edit
mh config set history.max_entries 100000
mh config set security.mask_secrets true
mh config reset
mh config validate
```

---

### 5.12 `mh tui`

Launches the interactive terminal UI.

Features:

* Fuzzy search with live filtering
* Arrow key navigation
* Enter to select a command
* Ctrl+R style reverse search
* Print selected command to terminal
* Re-execute selected command
* Detailed command view panel
* Filter panel (by date, exit code, shell, tag)
* Copy to clipboard *(new)*
* Pin/unpin from TUI *(new)*
* Tag from TUI *(new)*
* Delete from TUI *(new)*
* Side-by-side detail view *(new)*

TUI crates:

```text
ratatui
crossterm
arboard (clipboard)
```

---

### 5.13 `mh tag` *(New)*

Tag commands for organization and later retrieval.

```bash
mh tag 152 pentest
mh tag 152 recon important
mh tag --last 5 debug
mh untag 152 debug
mh tags list
```

---

### 5.14 `mh pin` *(New)*

Pin important commands so they are never deleted by auto-cleanup.

```bash
mh pin 152
mh pin 152 153 154
mh unpin 152
mh pinned
```

---

### 5.15 `mh snippet` *(New)*

Save reusable command snippets with names and descriptions.

```bash
mh snippet save "docker-clean" "docker system prune -af"
mh snippet save "git-undo" "git reset --soft HEAD~1" --desc "Undo last commit softly"
mh snippet list
mh snippet run docker-clean
mh snippet delete docker-clean
mh snippet export snippets.json
```

Snippets support placeholders:

```bash
mh snippet save "ssh-host" "ssh {{user}}@{{host}}"
mh snippet run ssh-host --user admin --host 192.168.1.1
```

---

### 5.16 `mh replay` *(New)*

Re-execute a command by its ID from history.

```bash
mh replay 152
mh replay 152 --dry-run     # prints the command without executing
mh replay 152 --confirm     # asks for confirmation before running
```

---

### 5.17 `mh diff` *(New)*

Compare history between sessions or machines.

```bash
mh diff --session a3f9b1 --session 8d9f2f
mh diff --host kali --host workstation
mh diff --today --yesterday
```

---

### 5.18 `mh audit` *(New)*

Security audit log — shows commands that were masked, blocked, or flagged.

```bash
mh audit
mh audit --today
mh audit --format json
```

Displays:

* Commands that triggered secret detection
* Commands that were skipped due to ignore rules
* Commands run as root
* SSH session activity

---

### 5.19 `mh private` *(New)*

Toggle private mode — no commands are recorded while active.

```bash
mh private on
mh private off
mh private status
```

Or via environment variable:

```bash
export MH_PRIVATE=1
```

---

### 5.20 `mh sync` *(New — Optional Feature)*

Sync history to a remote server. Requires the `sync` feature flag.

```bash
mh sync setup --url https://my-mh-server.com --token xxx
mh sync push
mh sync pull
mh sync status
mh sync enable
mh sync disable
```

Data is encrypted before transmission using AES-256-GCM.  
The sync server is self-hostable (open source companion server planned).

---

### 5.21 `mh vault` *(New)*

Encrypted command vault — store commands you never want in plain-text history.

```bash
mh vault add "kubectl exec -it pod -- /bin/sh"
mh vault list
mh vault run 3
mh vault delete 3
mh vault unlock              # prompts for vault passphrase
mh vault lock
```

Vault uses AES-256-GCM encryption with a passphrase stored in the OS keyring.

---

## 6. Database Design

SQLite is used throughout.

Default database path:

```bash
~/.local/share/mh/history.db
```

For root:

```bash
/root/.local/share/mh/history.db
```

System-wide alternative:

```bash
/var/lib/mh/history.db
```

---

### 6.1 Commands Table

```sql
CREATE TABLE commands (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    command         TEXT NOT NULL,
    command_hash    TEXT NOT NULL,
    cwd             TEXT,
    shell           TEXT,
    username        TEXT,
    hostname        TEXT,
    exit_code       INTEGER,
    duration_ms     INTEGER,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    session_id      TEXT,
    tty             TEXT,
    is_ssh          INTEGER DEFAULT 0,
    is_root         INTEGER DEFAULT 0,
    git_repo        TEXT,
    git_branch      TEXT,
    git_commit      TEXT,
    category        TEXT,
    env_context     TEXT,
    is_pinned       INTEGER DEFAULT 0,
    is_masked       INTEGER DEFAULT 0,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);
```

---

### 6.2 Sessions Table

```sql
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    username    TEXT,
    hostname    TEXT,
    shell       TEXT,
    tty         TEXT,
    is_ssh      INTEGER DEFAULT 0,
    started_at  TEXT,
    ended_at    TEXT
);
```

---

### 6.3 Tags Table *(New)*

```sql
CREATE TABLE tags (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    command_id  INTEGER NOT NULL REFERENCES commands(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
```

---

### 6.4 Snippets Table *(New)*

```sql
CREATE TABLE snippets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT UNIQUE NOT NULL,
    command     TEXT NOT NULL,
    description TEXT,
    tags        TEXT,
    use_count   INTEGER DEFAULT 0,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at  TEXT
);
```

---

### 6.5 Audit Log Table *(New)*

```sql
CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type  TEXT NOT NULL,  -- 'masked', 'skipped', 'blocked'
    raw_command TEXT,
    reason      TEXT,
    username    TEXT,
    hostname    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
```

---

### 6.6 Vault Table *(New)*

```sql
CREATE TABLE vault (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    encrypted_data  BLOB NOT NULL,
    nonce           BLOB NOT NULL,
    label           TEXT,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);
```

---

### 6.7 Indexes

```sql
CREATE INDEX idx_commands_command     ON commands(command);
CREATE INDEX idx_commands_cwd         ON commands(cwd);
CREATE INDEX idx_commands_started_at  ON commands(started_at);
CREATE INDEX idx_commands_exit_code   ON commands(exit_code);
CREATE INDEX idx_commands_user        ON commands(username);
CREATE INDEX idx_commands_hostname    ON commands(hostname);
CREATE INDEX idx_commands_session     ON commands(session_id);
CREATE INDEX idx_commands_category    ON commands(category);
CREATE INDEX idx_commands_is_pinned   ON commands(is_pinned);
CREATE INDEX idx_tags_tag             ON tags(tag);
CREATE INDEX idx_tags_command_id      ON tags(command_id);
```

---

### 6.8 Full-Text Search

SQLite FTS5:

```sql
CREATE VIRTUAL TABLE commands_fts USING fts5(
    command,
    cwd,
    content='commands',
    content_rowid='id'
);
```

---

## 7. Configuration File

Default config path:

```bash
~/.config/mh/config.toml
```

```toml
[history]
max_entries = 100000
ignore_duplicates = true
ignore_space_prefix = true
save_failed_commands = true
save_successful_commands = true
auto_categorize = true        # NEW
dedupe_window_seconds = 5     # NEW: ignore duplicate commands within N seconds

[security]
mask_secrets = true
skip_secret_commands = false
private_mode_env = "MH_PRIVATE"
audit_log = true              # NEW

[database]
path = "~/.local/share/mh/history.db"
auto_vacuum = true            # NEW
max_size_mb = 512             # NEW: warn when DB exceeds this size

[display]
default_limit = 50
color = true
date_format = "%Y-%m-%d %H:%M:%S"
show_duration = true          # NEW
show_exit_code = true         # NEW

[ignore]
commands = [
  "history",
  "clear",
  "exit",
  "logout",
  "mh record"
]

patterns = [
  ".*password.*",
  ".*token.*",
  ".*secret.*",
  ".*api[_-]?key.*",
  ".*bearer.*"
]

[sync]
enabled = false
server_url = ""
auto_sync_interval_minutes = 60

[vault]
enabled = false
use_keyring = true

[categories]
# Auto-classification rules
git      = ["git ", "gh ", "hub "]
docker   = ["docker ", "docker-compose ", "podman "]
network  = ["curl ", "wget ", "ssh ", "nc ", "nmap ", "ping "]
system   = ["systemctl ", "journalctl ", "top ", "htop "]
package  = ["apt ", "apt-get ", "dpkg ", "snap ", "cargo ", "pip "]
```

---

## 8. Security Features

Security is the most critical aspect of this application. History data must never contain sensitive information in plain text.

Risky examples:

```bash
mysql -u root -pMyPassword
curl -H "Authorization: Bearer TOKEN"
export AWS_SECRET_ACCESS_KEY=xxxx
sshpass -p password ssh user@host
```

---

### 8.1 Secret Detection

Commands are analyzed before recording.

Detected keys:

* `password`, `passwd`, `pwd`
* `token`, `secret`, `api_key`, `apikey`
* `bearer`, `authorization`
* `aws_secret_access_key`, `aws_access_key_id`
* `private_key`, `rsa_key`
* `sshpass`, `MYSQL_PWD`
* `DATABASE_URL` (if contains credentials) *(new)*
* `GITHUB_TOKEN`, `GITLAB_TOKEN` *(new)*
* Credit card patterns (regex) *(new)*

Regex:

```regex
(?i)(password|passwd|token|secret|api[_-]?key|authorization|bearer|private[_-]?key|database[_-]?url)
```

---

### 8.2 Masking

If sensitive data is found, the command can be fully skipped or masked.

Original:

```bash
curl -H "Authorization: Bearer abc123" https://api.example.com
```

Masked:

```bash
curl -H "Authorization: Bearer ****" https://api.example.com
```

Masking strategy is configurable:

* `mask` — store with redacted values
* `skip` — do not store at all
* `audit` — store only in audit log

---

### 8.3 Private Mode

The user can temporarily suspend recording.

```bash
export MH_PRIVATE=1
```

Or:

```bash
mh private on
mh private off
mh private status
```

While private mode is active, no commands are recorded.

---

### 8.4 Ignore Rules

```toml
[ignore]
commands = ["clear", "history", "exit"]
patterns = [".*password.*", ".*token.*"]
```

---

### 8.5 Encrypted Vault *(New)*

Highly sensitive commands can be stored in an AES-256-GCM encrypted vault.

```bash
mh vault add "kubectl exec -it production-pod -- /bin/sh"
mh vault list
mh vault run 1
```

The vault passphrase can be stored in the OS keyring (libsecret / GNOME Keyring / KWallet).

---

## 9. Output Formats

Default table view:

```text
 ID     Time                  Exit  Duration  CWD             Command
 1201   2026-05-31 17:20:01   0     142ms     /opt/app        docker ps
 1202   2026-05-31 17:21:14   1     34ms      /etc            cat shadow
```

JSON output:

```bash
mh search docker --json
```

CSV output:

```bash
mh search docker --csv
```

Plain output (command text only):

```bash
mh search docker --plain
```

Markdown output *(new)*:

```bash
mh last 10 --markdown
```

---

## 10. TUI Design

```text
╔══════════════════════════════════════════════════════════════╗
║  🔍  Search: docker_________________  [regex] [fuzzy] [tag]  ║
╠══════════════════════════════════════════════════════════════╣
║  Time               Exit  Duration  CWD          Command     ║
║  2026-05-31 17:20   0     142ms     /opt/app     docker ps   ║
║  2026-05-31 17:21   1     34ms      /opt/app     docker run  ║
║  2026-05-30 09:10   0     215ms     /opt/api     docker pull ║
╠══════════════════════════════════════════════════════════════╣
║  [Enter] Select  [Ctrl+R] Run  [Ctrl+C] Copy  [p] Pin        ║
║  [t] Tag  [d] Delete  [/] Filter  [?] Help  [Esc] Exit       ║
╚══════════════════════════════════════════════════════════════╝
```

TUI features:

* Live fuzzy search
* Regex search mode toggle
* Tag filtering
* Category filtering
* Side-by-side command detail panel
* Copy to clipboard (arboard)
* Pin / unpin from TUI
* Tag / untag from TUI
* Delete with confirmation
* Keyboard shortcut help panel

---

## 11. Project Directory Structure

```text
modern-history/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── PROJECT_PLAN.md
├── CONTRIBUTING.md
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── migrations/
│   ├── 001_init.sql
│   ├── 002_sessions.sql
│   ├── 003_fts.sql
│   ├── 004_tags.sql
│   ├── 005_snippets.sql
│   ├── 006_audit_log.sql
│   └── 007_vault.sql
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── db.rs
│   ├── error.rs
│   ├── models.rs
│   ├── security.rs
│   ├── classifier.rs          # NEW: auto-category detection
│   ├── shell/
│   │   ├── mod.rs
│   │   ├── bash.rs
│   │   ├── zsh.rs
│   │   ├── fish.rs
│   │   └── nushell.rs         # NEW
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── record.rs
│   │   ├── search.rs
│   │   ├── last.rs
│   │   ├── stats.rs
│   │   ├── delete.rs
│   │   ├── clear.rs
│   │   ├── export.rs
│   │   ├── import.rs
│   │   ├── doctor.rs
│   │   ├── tui.rs
│   │   ├── tag.rs             # NEW
│   │   ├── pin.rs             # NEW
│   │   ├── snippet.rs         # NEW
│   │   ├── replay.rs          # NEW
│   │   ├── diff.rs            # NEW
│   │   ├── audit.rs           # NEW
│   │   ├── private.rs         # NEW
│   │   ├── sync.rs            # NEW (feature-gated)
│   │   └── vault.rs           # NEW
│   ├── output/
│   │   ├── mod.rs
│   │   ├── table.rs
│   │   ├── json.rs
│   │   ├── csv.rs
│   │   └── markdown.rs        # NEW
│   ├── sync/                  # NEW (feature-gated)
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── crypto.rs
│   └── tui/
│       ├── mod.rs
│       ├── app.rs
│       ├── ui.rs
│       ├── events.rs
│       └── clipboard.rs       # NEW
└── tests/
    ├── cli_tests.rs
    ├── db_tests.rs
    ├── security_tests.rs
    ├── shell_tests.rs
    ├── snippet_tests.rs        # NEW
    ├── vault_tests.rs          # NEW
    └── classifier_tests.rs    # NEW
```

---

## 12. Development Phases

### Phase 1 — Core CLI

Goals:

* Create the Rust project.
* Set up CLI scaffolding with `clap`.
* Implement basic commands.

Tasks:

* `cargo new modern-history --bin`
* `clap` integration with derive macros
* `mh --help` and `mh --version`
* `mh doctor` (basic version)

**Success criterion:** `mh --help` and `mh doctor` run without errors.

---

### Phase 2 — SQLite Integration

Goals:

* Initialize database.
* Set up migration system.
* Enable command recording.

Tasks:

* Add `rusqlite` with bundled feature.
* Define database path logic.
* Create `commands` table.
* Implement `mh record`.

**Success criterion:** `mh record --command "ls -al" --exit-code 0` writes to the database.

---

### Phase 3 — Search and Last

Goals:

* Display recorded commands.
* Enable basic search.

Tasks:

* `mh last` with configurable limit
* `mh search` with text filter
* Table output with `comfy-table`
* Date formatting

**Success criterion:** `mh last 20` and `mh search docker` work correctly.

---

### Phase 4 — Zsh Integration

Goals:

* Primary shell integration for Kali Linux.

Tasks:

* `mh init zsh` generates hook code.
* `preexec` hook captures command and timestamp.
* `precmd` hook captures exit code and duration.
* Root shell support.

**Success criterion:** After adding integration to `.zshrc`, commands are automatically recorded.

---

### Phase 5 — Bash and Fish Integration

Goals:

* Support additional shells.

Tasks:

* `mh init bash` with `PROMPT_COMMAND` and `DEBUG` trap.
* `mh init fish` with event hooks.
* `mh init nushell`.

**Success criterion:** Command history is recorded in Bash, Fish, and Nushell.

---

### Phase 6 — Security Layer

Goals:

* Prevent sensitive commands from being recorded.

Tasks:

* Secret detector with regex patterns.
* Masking logic.
* Private mode (`MH_PRIVATE` env var).
* Ignore rules (command list and regex patterns).
* Audit log.

**Success criterion:** The following command is not stored or is masked:

```bash
curl -H "Authorization: Bearer abc123" https://example.com
```

---

### Phase 7 — Advanced Search

Goals:

* Professional filtering capabilities.

Tasks:

* Date range filter.
* Exit code filter.
* CWD filter.
* User and host filter.
* Regex search.
* Fuzzy search with `fuzzy-matcher`.
* SQLite FTS5 full-text search.

**Success criterion:**

```bash
mh search docker --cwd /opt --failed --after 2026-05-01
```

returns correct results.

---

### Phase 8 — Statistics Module

Goals:

* Generate usage analytics.

Tasks:

* Command frequency analysis.
* Error-prone command detection.
* Longest-running commands.
* Daily usage timeline.
* Category breakdown.
* Hourly heatmap.

**Success criterion:** `mh stats`, `mh stats --today`, `mh stats --month` produce meaningful output.

---

### Phase 9 — TUI

Goals:

* Modern interactive terminal interface.

Tasks:

* `ratatui` layout with search box and results list.
* Keyboard navigation.
* Fuzzy filter with live updates.
* Detail panel.
* Clipboard copy with `arboard`.

**Success criterion:** `mh tui` opens a working interactive interface.

---

### Phase 10 — Tags, Pins, Snippets

Goals:

* Organizational features.

Tasks:

* `mh tag`, `mh pin`, `mh snippet` commands.
* Tag filter in search and TUI.
* Snippet placeholder system.
* Tag and pin support in TUI.

**Success criterion:** Commands can be tagged, pinned, and retrieved by tag. Snippets with placeholders run correctly.

---

### Phase 11 — Export / Import

Goals:

* Make history data portable.

Tasks:

* JSON and CSV export.
* Compressed export with `zstd`.
* JSON import with duplicate detection.
* `--dry-run` import preview.

**Success criterion:**

```bash
mh export --json backup.json
mh import backup.json --merge
```

both work correctly.

---

### Phase 12 — Vault

Goals:

* Secure storage for sensitive commands.

Tasks:

* AES-256-GCM encryption with `aes-gcm`.
* OS keyring integration with `keyring`.
* `mh vault` command set.

**Success criterion:** Vault commands are encrypted at rest and require passphrase to access.

---

### Phase 13 — Sync (Optional)

Goals:

* Cross-machine history synchronization.

Tasks:

* Compile-time `sync` feature flag.
* HTTP client with `reqwest`.
* End-to-end encryption before transmission.
* Self-hostable server companion (separate repository).

**Success criterion:**

```bash
mh sync push
mh sync pull
```

successfully synchronize encrypted history.

---

### Phase 14 — Packaging

Goals:

* Make the application easy to install.

Tasks:

* Release build: `cargo build --release`
* `.deb` package generation.
* Install script (`install.sh`).
* Shell completion scripts (bash, zsh, fish).
* Man page (`mh.1`).
* GitHub Actions CI/CD pipeline.

```bash
sudo install -m 755 target/release/mh /usr/local/bin/mh
```

---

## 13. Test Plan

### 13.1 Unit Tests

Modules to test:

* Config parser
* Secret detector
* Command classifier *(new)*
* Database insert
* Database search
* Date parser
* Ignore rules
* Output formatter
* Snippet placeholder substitution *(new)*
* Vault encryption / decryption *(new)*

---

### 13.2 Integration Tests

Test scenarios:

* Command recording
* Command search with multiple filters
* Command deletion
* JSON export and import round-trip
* CSV export
* Config read and write
* Database migration
* Tag and pin operations *(new)*
* Audit log entries *(new)*

---

### 13.3 Shell Tests

Environments to test:

* Kali Linux — Zsh
* Bash (interactive and non-interactive)
* Root shell
* SSH session
* Fish
* Nushell *(new)*

---

## 14. Security Testing

The following commands must be tested for correct behavior:

```bash
mysql -u root -pSecret123
curl -H "Authorization: Bearer TOKEN" https://api.test
export AWS_SECRET_ACCESS_KEY=abc
sshpass -p password ssh root@1.1.1.1
export GITHUB_TOKEN=ghp_xxxxxxxxxxxx
```

Expected result:

* Command is either not recorded at all.
* Or sensitive values are masked.
* Audit log entry is created.

---

## 15. Performance Targets

* `mh record` must complete in under **20 ms**.
* `mh last` must return output in under **100 ms**.
* Full-text search across **1 million records** must complete in under **500 ms**.
* SQLite indexes must be used correctly — verify with `EXPLAIN QUERY PLAN`.
* Shell hooks must not introduce noticeable terminal latency.
* Compressed export of 100k records must complete in under **5 seconds**.

---

## 16. Example Usage Flow

Installation:

```bash
cargo build --release
sudo install -m 755 target/release/mh /usr/local/bin/mh
```

Zsh integration:

```bash
echo 'eval "$(mh init zsh)"' >> ~/.zshrc
source ~/.zshrc
```

Daily usage:

```bash
docker ps
cd /opt
ls -al
mh last
mh search docker
mh stats
mh tui
mh tag 1201 production
mh pin 1202
mh snippet save "docker-prune" "docker system prune -af"
mh snippet run docker-prune
```

---

## 17. Minimum Viable Product

Must be in v1.0:

* Rust CLI binary
* SQLite recording
* Zsh integration
* `mh record`
* `mh last`
* `mh search`
* Secret masking
* Config file
* `mh doctor`

Not required in v1.0:

* TUI
* Fish and Nushell support
* Export / import
* Sync
* Vault
* Tags and pins
* Snippets

---

## 18. Future Roadmap

Features planned for future versions:

| Feature | Priority | Version Target |
|---|---|---|
| Encrypted sync | High | v1.2 |
| Multi-device sync | High | v1.2 |
| Web UI dashboard | Medium | v2.0 |
| REST API | Medium | v2.0 |
| AI-powered command suggestions | Low | v2.1 |
| AI-powered command explanations | Low | v2.1 |
| Risky command warnings | High | v1.1 |
| Team history sharing | Low | v3.0 |
| Project-scoped history | Medium | v1.3 |
| Git commit correlation | Medium | v1.3 |
| Docker container shell history | Medium | v1.4 |
| VS Code / Neovim plugin | Low | v2.0 |
| macOS support | Medium | v1.5 |

---

## 19. Risks and Mitigations

### 19.1 Shell Differences

Bash, Zsh, and Fish hook mechanisms are structurally different.

**Mitigation:**

* Separate module for each shell under `src/shell/`.
* Shared `record` API used by all shell modules.

---

### 19.2 Performance

`mh record` runs after every command, creating potential for latency.

**Mitigation:**

* Record operation must be minimal — open SQLite, insert, close.
* Avoid unnecessary processing in the hot path.
* Consider async background recording for future improvement.

---

### 19.3 Sensitive Data Risk

History data may contain passwords and tokens.

**Mitigation:**

* Secret masking enabled by default.
* Users can disable it if desired.
* Private mode supported.
* Vault available for deliberate sensitive command storage.

---

### 19.4 Root / Regular User Separation

`sudo su` may spawn a different shell with a different home directory.

**Mitigation:**

* Root requires separate `mh init`.
* `/root/.zshrc` and `/root/.bashrc` are handled independently.
* Per-user database separation.

---

### 19.5 Database Corruption

SQLite files can become corrupted on system crashes.

**Mitigation:**

* `mh doctor` checks integrity via `PRAGMA integrity_check`.
* Scheduled auto-vacuum.
* `mh export --compressed` for regular backups.

---

## 20. Recommended Development Order

1. Initialize the Rust project (`cargo new modern-history --bin`).
2. Set up CLI structure with `clap` derive macros.
3. Implement SQLite connection and migration runner.
4. Create `commands` table and related indexes.
5. Implement `mh record`.
6. Implement `mh last`.
7. Implement `mh search` with basic text filter.
8. Generate `mh init zsh` hooks.
9. Test Zsh integration on Kali Linux.
10. Add secret masking and ignore rules.
11. Add audit logging.
12. Add config file support with `toml`.
13. Implement `mh doctor`.
14. Add Bash and Fish integration.
15. Implement statistics module (`mh stats`).
16. Add command classifier.
17. Implement tags and pins.
18. Implement snippet system.
19. Develop TUI with `ratatui`.
20. Add compressed export/import.
21. Implement vault with AES-256-GCM.
22. (Optional) Implement sync feature.
23. Package as `.deb`, add man page, shell completions.
24. Set up CI/CD with GitHub Actions.

---

## 21. Conclusion

When built with Rust, this project can become a far more modern, secure, and professional alternative to the classic Linux `history` command.

The initial target must remain small:

```text
Zsh + SQLite + Search + Secret Masking
```

Once this core is solid, TUI, statistics, export/import, tags, pins, snippets, vault, and advanced filtering can be layered on top.

The key differentiators over existing tools (`atuin`, `mcfly`, `hstr`) are:

* **Full audit trail** — every mask and skip event is logged.
* **Encrypted vault** — for commands that must never appear in plain history.
* **Snippet system** — reusable commands with placeholder variables.
* **Category auto-classification** — instant analytics without manual tagging.
* **Self-hostable sync** — privacy-first multi-machine sync.

> **Final reminder:** All code, identifiers, comments, commit messages, and documentation in this project must be written in English. This is non-negotiable.
