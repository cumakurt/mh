# mh

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](LICENSE)

`mh` is a modern Linux command history manager written in Rust. It records shell commands into a local SQLite database, then makes that history searchable, filterable, taggable, auditable, exportable, and usable from an interactive terminal interface. Enterprise features include a unified policy engine, tamper-evident audit logs, session forensics, legal holds, runbooks, environment classification, SIEM streaming, and break-glass mode.

**License:** `mh` is distributed under the [GNU Affero General Public License v3.0 or later](LICENSE) (AGPL-3.0-or-later). You may use, modify, and redistribute it under the terms of that license. If you run a modified version as a network service, you must offer corresponding source to users interacting with it over the network.

The command name is intentionally short:

```bash
mh
```

## Developer

- Author: Cuma Kurt <cumakurt@gmail.com>
- GitHub: https://github.com/cumakurt/mh
- LinkedIn: https://www.linkedin.com/in/cuma-kurt-34414917/
- License: [AGPL-3.0-or-later](LICENSE) — see the [License](#license) section below

## What mh Does

Traditional shell history is plain text and usually answers only one question: "What did I type before?" `mh` stores structured command records so you can answer better questions:

- Which command failed in this project yesterday?
- Which Docker commands did I run last week?
- What did I run in this SSH session?
- Which command contained a token and was masked or skipped?
- Which commands should be pinned before cleanup?
- Which snippets do I reuse often?
- Which commands are worth moving into an encrypted vault?
- Which critical commands were blocked by policy in production?
- Can the audit log prove it was not tampered with?
- What exactly ran in this session during an incident?

The database is local-first. By default, regular users and root have separate data paths because they have different home directories.

## Feature Overview

- Shell hooks for Bash, Zsh, Fish, and Nushell.
- Up arrow binding that opens an interactive command history picker.
- SQLite storage with FTS5 full-text search.
- Command metadata: command text, working directory, shell, user, host, exit code, duration, session ID, TTY, SSH/root flags, Git repository, Git branch, Git commit, category, tags, environment context, and command hash.
- Secret detection, masking, ignore rules, private mode, audit logs, and oversized-command rejection (256 KiB max).
- `mh doctor` health checks with human-readable output or `--json` for automation/CI.
- Safe file writes for config, exports, completions, and man pages (atomic temp+rename, mode `0600`, symlink rejection).
- Unified policy engine with allow, warn, deny, require-approval actions, and shared-key signed policy packs.
- Tamper-evident audit log with SHA-256 hash chain verification.
- Session timeline forensics, incident bundle export, legal holds, retention purge, and runbooks.
- Environment classification (production, staging, development) on every recorded command.
- SIEM-friendly audit streaming (syslog, webhook, CEF/JSON) via `mh watch`.
- Break-glass mode for emergency recording override with mandatory reason and TTL.
- Search modes: substring, regex, fuzzy, FTS, and local natural-language ranking.
- Filters for CWD, user, shell, date range, success/failure, tag, category, pinned state, and duration.
- `last`, `stats`, `delete`, `clear`, `export`, and `import` workflows.
- Tags, pins, reusable snippets, replay, diff, audit, policy, incident, timeline, hold, runbook, watch, and break-glass commands.
- Risk-aware replay guidance with optional preview commands for destructive operations.
- Ratatui terminal UI with live fuzzy filtering, dashboard mode, detail panel, clipboard copy, pin/unpin, tagging, and delete confirmation.
- Encrypted command vault using AES-256-GCM with passphrase input.
- Optional encrypted remote sync with `sync` feature flag (`--features sync`).
- Installer that detects Linux distribution, package manager, and shell.
- Portable shell hooks (GNU/BusyBox `date`, Bash 5+ `EPOCHREALTIME`, `python3`, or `perl` for duration timing).
- Shell completions and man page generation.
- Debian package build script.

## Platform and Shell Compatibility

`mh` targets **Linux on x86_64 and aarch64** with a local SQLite database and Unix-domain record daemon. The bundled SQLite build avoids distro-specific library mismatches.

### Tested Linux distributions

The installer (`install.sh`) detects the OS via `/etc/os-release` and installs build dependencies for:

| Family | Package managers | Examples |
|--------|------------------|----------|
| Debian | `apt` | Debian, Ubuntu, Linux Mint, Kali Linux, Pop!_OS |
| RHEL | `dnf`, `yum` | Fedora, RHEL, CentOS Stream, Rocky, Alma |
| Arch | `pacman` | Arch Linux, Manjaro, EndeavourOS |
| SUSE | `zypper` | openSUSE, SLE |
| Alpine | `apk` | Alpine Linux, postmarketOS |

Other FHS-compliant distributions usually work when Rust, a C toolchain, and `pkg-config` are available.

### Supported interactive shells

| Shell | Binary paths (examples) | Integration | Config file |
|-------|-------------------------|-------------|-------------|
| **Bash** | `/bin/bash`, `/usr/bin/bash`, **`rbash`** (restricted bash) | `DEBUG` trap + `PROMPT_COMMAND` | `~/.bashrc`, `~/.bash_profile`, `~/.profile` |
| **Zsh** | `/bin/zsh`, `/usr/bin/zsh` | `preexec` / `precmd` hooks | `~/.zshrc`, `~/.zshenv` |
| **POSIX sh / dash** | `/bin/sh`, `/usr/bin/sh`, `/usr/bin/dash` | `fc` history + prompt hook (`mh init sh`) | `~/.profile` |
| **Fish** | `/usr/bin/fish` | `fish_preexec` / `fish_postexec` | `$XDG_CONFIG_HOME/fish/config.fish` |
| **Nushell** | `/usr/bin/nu` | `pre_execution` / `pre_prompt` | `$XDG_CONFIG_HOME/nushell/config.nu` |
| **PowerShell** | `/usr/bin/pwsh`, `/opt/microsoft/powershell/7/pwsh` | PSReadLine `AddToHistoryHandler` | `$XDG_CONFIG_HOME/powershell/Microsoft.PowerShell_profile.ps1` |

**Not shells (no hook on the multiplexer itself):** `screen`, `tmux` — run `mh init <your-login-shell>` inside the pane; `$SHELL` may point at tmux/screen but hooks belong in bash/zsh/etc.

When `/bin/sh` is a symlink to bash, `mh init sh` tells you to use `mh init bash` for full `DEBUG` trap support.

Install integration:

```bash
mh init                  # detects $SHELL, installs the managed block, prints the source command
mh init --install        # same behavior, explicit
mh init bash --install   # force a shell
mh init zsh --install
mh init sh --install     # dash / posix sh
mh init pwsh --install   # PowerShell 7+
mh init fish --install
mh init nushell --install
```

Or use `./install.sh --shell <shell>` which picks the first existing config file from the list above.

### Portable duration timing

Hook scripts never rely on GNU-only `date +%s%3N` alone. Millisecond duration uses, in order:

1. Bash 5+ / Zsh `EPOCHREALTIME` (Zsh loads `zmodload zsh/datetime` when available)
2. `python3`
3. `perl` with `Time::HiRes`
4. GNU/BusyBox `date +%s%3N`
5. Second-precision `date +%s` (×1000) as a last resort

Install `python3` on minimal images (Alpine, slim containers) for best duration accuracy.

### Multi-user, SSH, and privilege separation

- **Regular user vs root**: separate home directories → separate `~/.local/share/mh` / config paths by default.
- **SSH sessions**: `SSH_CONNECTION` / `SSH_CLIENT` set `is_ssh` on records; use `MH_SESSION_ID` (set by hooks) with `mh last --session`.
- **`sudo` / `su`**: switching users switches the active home and database; hooks in the target user shell record to that user DB.
- **Record daemon**: Unix socket under `$XDG_RUNTIME_DIR/mh/record.sock` when set (typical on systemd/logind desktops), otherwise `~/.local/share/mh/record.sock`. Only the same UID may write (peer credential check). No fallback to world-writable `/tmp`.

### Multiline commands and HEREDOCs

Shell hooks record the command line as the shell presents it to `preexec` / `DEBUG` / `fish_preexec`. Commands built from HEREDOCs, continued lines, or editor buffers may appear as a single logical line or only capture the first line depending on the shell. Prefer `mh record` manually or snippets/runbooks for multi-step flows you need preserved verbatim.

### Not supported in this release

- macOS / BSD (hooks are Linux-oriented; database code is portable but not CI-tested outside Linux)
- Windows / WSL is best-effort only
- **Fleet control plane (`mh-server`)** — remote sync uses optional `mh sync` with `--features sync` and a compatible HTTP API; a dedicated multi-tenant server is not part of this repository

## Command Gallery

Every command below was verified with `scripts/verify-all-commands.sh` in an isolated demo environment.

```bash
./scripts/verify-all-commands.sh
```

The SVG screenshots are generated from `docs/examples/*.txt`:

```bash
./scripts/render-example-screenshots.py
```

### Core

#### `mh about`

![mh about](docs/screenshots/about.svg)

#### `mh doctor`

![mh doctor](docs/screenshots/doctor.svg)

```bash
mh doctor
mh doctor --strict          # exit non-zero when warnings are reported
mh doctor --json            # machine-readable report for scripts and CI
```

JSON output includes `status`, `warning_count`, `checks[]`, and `summary` (config/database paths, schema version, command count, daemon/private-mode state, mh version). Use with `jq` in pipelines:

```bash
mh doctor --json | jq -e '.status == "ok" and .warning_count == 0'
```

#### `mh init`

![mh init zsh](docs/screenshots/init-zsh.svg)

```bash
mh init
mh init bash
mh init zsh
mh init fish
mh init nushell
```

#### `mh config`

![mh config show](docs/screenshots/config-show.svg)

```bash
mh config show
mh config path
mh config validate
mh config set history.max_entries 200000
mh config reset
```

### History

#### `mh record`

Shell hooks call this automatically. Manual example:

```bash
mh record --command "git status" --cwd "$PWD" --shell zsh --exit-code 0 --duration-ms 42
```

When the record daemon is running, `mh record` sends events over a Unix socket instead of opening SQLite on every hook. Use `--no-daemon` or `MH_NO_DAEMON=1` to force the direct database path.

#### `mh daemon`

Keeps one long-lived database connection for high-frequency shell hooks:

```bash
mh daemon start    # background
mh daemon status
mh daemon stop
mh daemon run      # foreground
mh daemon install  # write ~/.config/systemd/user/mh-record-daemon.service
```

Default socket: `$XDG_RUNTIME_DIR/mh/record.sock`, or `~/.local/share/mh/record.sock` when `XDG_RUNTIME_DIR` is unset (override with `MH_DAEMON_SOCKET`).

**systemd user service** (starts at login, survives until logout):

```bash
mh daemon install
systemctl --user daemon-reload
systemctl --user enable --now mh-record-daemon.service
systemctl --user status mh-record-daemon.service
```

A reference unit file is also in `contrib/systemd/user/mh-record-daemon.service`. `mh doctor` reports whether the daemon is running or a stale socket is present.

#### `mh last`

![mh last](docs/screenshots/last.svg)

```bash
mh last
mh last 20
mh last 1 --offset 3 --plain
mh last --failed
mh last --json
mh last --plain
```

#### `mh search`

![mh search](docs/screenshots/search.svg)

![mh search --fuzzy](docs/screenshots/search-fuzzy.svg)

```bash
mh search docker --limit 10
mh search --fts git
mh search --fuzzy dps
mh search --regex '^git '
mh search docker --json
```

#### `mh pick`

![mh pick](docs/screenshots/pick.svg)

```bash
mh pick
mh pick --query docker --limit 100
mh pick --fuzzy --query dps
```

`mh pick` ranks results by current directory, successful exit codes, and recency when `display.context_ranking` is true (default). Up-arrow in integrated shells uses the same picker.

#### `mh tui`

![mh tui](docs/screenshots/tui.svg)

```bash
mh tui
mh tui --query docker --limit 500
```

### Organization

#### `mh tag`, `mh tags`, `mh pin`

![mh tags list](docs/screenshots/tags-list.svg)

![mh pinned](docs/screenshots/pinned.svg)

```bash
mh tag 152 deploy prod
mh tag --last 5 investigation
mh untag 152 prod
mh tags list
mh pin 152
mh unpin 152
mh pinned
```

#### `mh snippet`

![mh snippet list](docs/screenshots/snippet-list.svg)

![mh snippet run](docs/screenshots/snippet-run.svg)

```bash
mh snippet save docker-clean "docker system prune -af"
mh snippet list
mh snippet run docker-clean --dry-run
mh snippet export snippets.json
```

### Analysis

#### `mh stats`

![mh stats](docs/screenshots/stats.svg)

```bash
mh stats
mh stats --today --top 20
mh stats --week --heatmap
```

#### `mh diff`

![mh diff](docs/screenshots/diff-session.svg)

```bash
mh diff --session session-a --session session-b
mh diff --host laptop --host server
```

#### `mh risk`

![mh risk list](docs/screenshots/risk-list.svg)

![mh risk check](docs/screenshots/risk-check.svg)

![mh risk scan](docs/screenshots/risk-scan.svg)

```bash
mh risk list
mh risk check "rm -rf /"
mh risk scan --critical --today
```

#### `mh context`

![mh context](docs/screenshots/context.svg)

![mh context repos](docs/screenshots/context-repos.svg)

![mh context history](docs/screenshots/context-history.svg)

```bash
mh context
mh context repos
mh context branches --repo /path/to/repo
mh context history --branch main
```

### Security

#### `mh audit`

![mh audit](docs/screenshots/audit.svg)

```bash
mh audit
mh audit --today --format json
mh audit --verify-chain
```

#### `mh policy`

```bash
mh policy list
mh policy check "rm -rf /"
mh policy check "rm -rf /" --hostname prod-web-01 --env production --json
# Shell hooks use --quiet (exit 2 = deny, 3 = require_approval):
mh policy check "rm -rf /" --cwd "$PWD" --quiet
```

With `policy.enforce_in_shell = true` (default), **bash** and **zsh** block denied commands at Enter; **fish** cancels the line in `preexec`. One-off approval: `MH_POLICY_APPROVE=1 your-command`. Disable enforcement: `mh config set policy.enforce_in_shell false`.

#### `mh timeline`

Session forensics: list commands in chronological order for a session ID.

```bash
mh timeline --session demo-session-001
mh timeline --session demo-session-001 --plain
mh timeline --session demo-session-001 --json
```

#### `mh hold`

Legal hold and retention management.

```bash
mh hold add incident-1 --session demo-session-001 --reason "investigation"
mh hold add prod-freeze --git-repo /srv/app --reason "audit window"
mh hold list
mh hold remove 1
mh hold purge --dry-run
mh hold purge
```

`mh hold purge` requires `retention.enabled = true` in config. Legal-hold records are never deleted by retention or `mh clear`.

#### `mh runbook`

Create reusable multi-step command sequences from a session timeline.

```bash
mh runbook create deploy --session demo-session-001 --desc "Deploy flow"
mh runbook list
mh runbook show deploy
mh runbook run deploy --dry-run
mh runbook run deploy
```

#### `mh break-glass`

Emergency override when private mode would block recording. Requires a reason and expires automatically.

```bash
mh break-glass on --reason "prod incident response" --ttl-hours 4
mh break-glass status
mh break-glass off
```

#### `mh watch`

Stream audit events for SIEM integration. Enable `[siem]` in config for live syslog/webhook delivery on new audit entries.

```bash
mh watch --limit 20 --format json
```

#### `mh private`

![mh private status](docs/screenshots/private-status.svg)

```bash
mh private on
mh private off
mh private status
```

#### `mh vault`

![mh vault list](docs/screenshots/vault-list.svg)

```bash
MH_VAULT_PASSPHRASE=secret mh vault add "kubectl get pods" --label prod
mh vault list
mh vault run 1 --dry-run
mh vault delete 1
```

### Data Management

#### `mh export` and `mh import`

![mh export](docs/screenshots/export-json.svg)

![mh import](docs/screenshots/import-dry.svg)

```bash
mh export --json history.json
mh export --csv history.csv
mh export --markdown history.md
mh export --sqlite backup.db
mh export --json full.json --include-secrets   # opt out of redaction
mh import history.json --dry-run
mh import history.json --merge
```

#### `mh delete` and `mh clear`

```bash
mh delete 152 --yes
mh delete --older-than 90d --yes
mh clear
mh clear --keep-pinned
```

`mh clear` asks for confirmation. Use `mh delete --yes` for scripted cleanup.

#### `mh replay`

![mh replay --dry-run](docs/screenshots/replay-dry.svg)

```bash
mh replay 152 --dry-run
mh replay 152
mh replay 152 --yes
mh replay 152 --reason "approved by on-call" --yes
mh replay 152 --risk-preview
```

Policy rules with `require_approval` need `--reason` or `--yes` before execution.

### Sync

![mh sync status](docs/screenshots/sync-status.svg)

```bash
mh sync status
mh sync init --server https://mh.example.test    # generates E2E token + device id
mh sync setup https://mh.example.test token-value # other machines
mh sync enable
mh sync push   # or pull
mh sync disable
```

Remote push/pull requires building with `--features sync`. Payloads are **AES-256-GCM** encrypted by default (`sync.encrypt_payload`, on by default).

### Shell Tooling

#### `mh completions`

![mh completions zsh](docs/screenshots/completions-zsh.svg)

```bash
mh completions bash
mh completions zsh --output _mh
mh completions fish
```

#### `mh man`

![mh man](docs/screenshots/man.svg)

```bash
mh man
mh man --output mh.1
```

## Installation

The recommended installation path is the included installer:

```bash
./install.sh
```

The installer performs these steps:

- Confirms the system is Linux.
- Detects `/etc/os-release`.
- Detects the package manager: `apt`, `dnf`, `yum`, `pacman`, `zypper`, or `apk`.
- Installs build dependencies for the detected distribution.
- Installs Rust with `rustup` if `cargo` and `rustc` are not already available.
- Builds the release binary with `cargo build --release`.
- Installs `mh` into `/usr/local/bin` or `~/.local/bin`.
- Installs Bash, Zsh, and Fish completions.
- Installs the `mh` man page.
- Detects the current shell and enables shell integration via `mh init`.
- Runs `mh doctor` at the end.

Shell hooks are installed by delegating to `mh init` (same code path as manual setup). If integration fails, run `mh init --repair` to repair the detected shell, or `mh init <shell> --repair` to force a shell.

Common installer options:

```bash
./install.sh --user
./install.sh --system
./install.sh --shell zsh
./install.sh --shell bash
./install.sh --shell fish
./install.sh --shell nushell
./install.sh --install-dir "$HOME/.local/bin"
./install.sh --no-deps
./install.sh --no-build
./install.sh --no-enable
./install.sh --no-completions
./install.sh --no-man
```

Useful environment variables for the installer:

```bash
INSTALL_DIR="$HOME/.local/bin" ./install.sh
MH_SHELL=zsh ./install.sh
```

After installation, open a new shell session or reload your shell config. `mh init` prints the exact command for your detected shell:

```bash
source ~/.zshrc
source ~/.bashrc
```

For Fish:

```fish
source ~/.config/fish/config.fish
```

For Nushell:

```nu
source ~/.config/nushell/config.nu
```

## Build From Source

Requirements:

- Rust toolchain
- C compiler and build tools
- `pkg-config`
- `git`
- `curl`
- `ca-certificates`

Build and run:

```bash
cargo build
cargo run -- --help
```

Build an optimized binary:

```bash
cargo build --release
./target/release/mh --help
```

Run tests:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Use a temporary database while testing:

```bash
export MH_CONFIG=/tmp/mh-config.toml
export MH_DB=/tmp/mh-history.db
cargo run -- record --command "docker ps" --exit-code 0
cargo run -- last
```

## Shell Integration

`mh init` detects `$SHELL`, appends the managed integration block to the resolved shell config file, and prints the command needed to activate it in the current terminal. New shell sessions load the integration automatically.

Because `mh` runs as a child process, it cannot directly `source` the parent shell process for you. Run the printed `source ...` command once in the current terminal, or start a new shell.

`mh init <shell>` still prints shell integration code for manual setups and tests. Add `--install` when you want to force installation for a specific shell.

Supported shells:

```bash
mh init
mh init bash
mh init zsh
mh init fish
mh init nushell
```

Manual Zsh setup:

```zsh
eval "$(mh init zsh)"
```

Manual Bash setup:

```bash
eval "$(mh init bash)"
```

Manual Fish setup:

```fish
mh init fish | source
```

Manual Nushell setup:

```nu
mh init nushell | save -f ~/.config/nushell/mh.nu
source ~/.config/nushell/mh.nu
```

`mh init` and `mh init <shell> --install` append a managed block to the resolved config file (see **Platform and Shell Compatibility**). The installer uses the same path resolution rules.

Repair duplicate or broken integration:

```bash
mh init --repair
mh init zsh --repair
mh init bash --repair
```

`--repair` removes all managed `mh` blocks from the config file, then reinstalls a single clean block. Re-run it if hooks were appended more than once.

On **login shells** that only source `~/.bash_profile` (common on macOS-style layouts and some RHEL defaults), `mh init` and `mh init bash --install` target that file when `.bashrc` is missing.

### Hook Behavior

The shell integration records commands by calling `mh record` after a command finishes.

Zsh uses:

- `zmodload zsh/datetime` when available for sub-millisecond timing.
- `preexec` to capture the command text and start time.
- `precmd` to capture the exit code and duration.

Bash uses:

- `DEBUG` trap to capture the command before execution.
- `PROMPT_COMMAND` to record the command after execution.

Fish uses:

- `fish_preexec`
- `fish_postexec`

Nushell uses:

- `pre_execution`
- `pre_prompt`

Each shell integration sets `MH_SESSION_ID` if it is not already set. That lets you filter the current shell session:

```bash
mh last --session
```

### Up Arrow Picker

For Bash, Zsh, Fish, and Nushell, the integration binds the up arrow key to `mh pick`. Pressing up opens a selectable command list instead of only stepping through raw shell history.

For Bash, Zsh, and Fish, you can optionally bind the left and right arrow keys to step through recent `mh` commands on the prompt (without opening the picker). Set `MH_HISTORY_ARROWS=1` before loading shell integration (or add it to your shell rc file). With it enabled, press right repeatedly to move to older commands and left to move back toward the original prompt line.

Inside the picker:

- Type to fuzzy-filter commands.
- Use `Up` and `Down` to move.
- Use `PageUp` and `PageDown` for larger jumps.
- Use `Home` and `End` in the picker.
- Press `Enter` to select a command.
- Press `Esc` or `Ctrl-C` to cancel.

Limit picker results:

```bash
MH_PICK_LIMIT=250
```

Open the picker manually:

```bash
mh pick
mh pick --limit 200
mh pick --query docker
mh pick --cwd /srv/app
mh pick --failed
mh pick --tag deploy
mh pick --category git
mh pick --pinned
mh pick --fuzzy --query "dps"
```

In non-interactive mode, `mh pick` prints the first matching command.

## Data Paths

Default config path:

```bash
~/.config/mh/config.toml
```

Default database path:

```bash
~/.local/share/mh/history.db
```

Override paths:

```bash
export MH_CONFIG=/path/to/config.toml
export MH_DB=/path/to/history.db
```

`MH_DB` must point to a regular file, not a directory. Config and database files are created with restrictive permissions where the application controls the write path.

Root uses root's own home directory, so root history is separate by default:

```bash
sudo mh doctor
```

## Configuration

Show the active config:

```bash
mh config show
```

Print the active config path:

```bash
mh config path
```

Open the config in `$EDITOR`:

```bash
EDITOR=nano mh config edit
```

Set individual values:

```bash
mh config set history.max_entries 200000
mh config set history.ignore_duplicates true
mh config set history.dedupe_window_seconds 10
mh config set security.mask_secrets true
mh config set security.skip_secret_commands false
mh config set display.default_limit 100
mh config set database.max_size_mb 1024
mh config set sync.auto_sync_interval_minutes 30
```

Validate the config:

```bash
mh config validate
```

Reset to defaults:

```bash
mh config reset
```

Default config shape:

```toml
[history]
max_entries = 100000
ignore_duplicates = true
ignore_space_prefix = true
save_failed_commands = true
save_successful_commands = true
auto_categorize = true
dedupe_window_seconds = 5

[security]
mask_secrets = true
skip_secret_commands = false
private_mode_env = "MH_PRIVATE"
audit_log = true

[database]
path = "/home/user/.local/share/mh/history.db"
auto_vacuum = true
max_size_mb = 512

[display]
default_limit = 50
color = true
date_format = "%Y-%m-%d %H:%M:%S"
show_duration = true
show_exit_code = true

[ignore]
commands = ["history", "clear", "exit", "logout", "mh record"]
patterns = [".*password.*", ".*token.*", ".*secret.*", ".*api[_-]?key.*", ".*bearer.*"]

[categories]
git = ["git ", "gh "]
docker = ["docker ", "docker-compose ", "podman "]
network = ["curl ", "wget ", "ssh ", "nc ", "nmap ", "ping "]
system = ["systemctl ", "journalctl ", "top ", "htop "]
package = ["apt ", "apt-get ", "dpkg ", "snap ", "cargo ", "pip "]

[sync]
enabled = false
server_url = ""
token = ""
auto_sync_interval_minutes = 60

[vault]
enabled = false
use_keyring = true

[policy]
default_action = "allow"

[[policy.rules]]
id = "deny-critical-prod"
action = "deny"
risk_level = "critical"
environment = "production"
message = "Critical commands are blocked in production"

[[policy.rules]]
id = "approval-critical"
action = "require_approval"
risk_level = "critical"
message = "Critical commands require explicit approval"

[[policy.rules]]
id = "warn-high"
action = "warn"
risk_level = "high"
message = "High risk command detected"

[retention]
enabled = false
retention_days = 365
respect_legal_hold = true

[environment]

[[environment.rules]]
tier = "production"
hostname_contains = "prod"

[[environment.rules]]
tier = "staging"
hostname_contains = "stage"

[[environment.rules]]
tier = "development"
hostname_contains = "dev"

[siem]
enabled = false
format = "syslog"   # syslog | json | cef
syslog_url = "127.0.0.1:5140"
webhook_url = ""

[break_glass]
default_ttl_hours = 4
```

Older config files that do not contain newer sections are filled with defaults at load time.

## Recording Commands

Normally you do not call `mh record` yourself. Shell hooks call it after each command.

For interactive shells that record very frequently, start `mh daemon start` once per login session. Hooks keep calling `mh record`; the CLI forwards to the daemon when the socket is available and falls back to SQLite otherwise.

Manual examples are useful for testing:

```bash
mh record --command "docker ps" --exit-code 0
mh record --command "curl https://example.test" --cwd /tmp --shell zsh --exit-code 0 --duration-ms 84
mh record --command "git status" --tags "work,git"
mh record --command "pytest tests" --exit-code 1 --duration-ms 2400 --env-context virtualenv
```

Fields captured by `mh record` include:

- Command text
- SHA-256 command hash
- Working directory
- Shell
- Username
- Hostname
- Exit code
- Duration in milliseconds
- Start and finish timestamps
- Session ID
- TTY
- SSH session flag
- Root user flag
- Git repository path
- Git branch
- Git commit
- Auto category
- Environment context
- Environment tier (`production`, `staging`, `development`, or `unknown`)
- User tags
- Pinned flag
- Masked flag
- Legal hold flag (set by `mh hold`)

Environment context is detected for common cases:

- `docker`
- `virtualenv`
- `nix-shell`

Git context is detected when the command is recorded inside a Git work tree.

Environment tier is classified from hostname, working directory, and Git repository path using `[environment]` rules in config. Production hosts matching `prod` in the hostname are tagged automatically.

Policy evaluation runs on every recorded command. Critical commands can be denied in production, warned in other environments, or require approval on replay.

## Listing Recent History

Show recent commands:

```bash
mh last
mh last 20
```

Show only failed commands:

```bash
mh last --failed
```

Show commands from a working directory:

```bash
mh last --cwd /srv/app
```

Show commands from the current shell session:

```bash
mh last --session
```

Show by tag, category, or pinned state:

```bash
mh last --tag deploy
mh last --category docker
mh last --pinned
```

Machine-readable output:

```bash
mh last --json
mh last --plain
```

Plain output prints only command text. It is useful for pipes:

```bash
mh last --plain | head
```

## Searching History

Basic substring search:

```bash
mh search docker
mh search "ssh root"
mh search "kubectl get pods"
```

Limit results:

```bash
mh search docker --limit 10
mh search docker -n 10
```

Search by working directory:

```bash
mh search --cwd /srv/app
mh search docker --cwd /srv/app
```

Search by command outcome:

```bash
mh search --failed
mh search --success
```

Search by user or shell:

```bash
mh search --user root
mh search --shell zsh
```

Search by time range:

```bash
mh search --after 2026-05-01
mh search --before 2026-05-31
mh search docker --after 2026-05-01 --before 2026-05-31
```

Regex search:

```bash
mh search --regex '^git (status|log)'
mh search --regex 'docker .*--format'
```

Fuzzy search:

```bash
mh search --fuzzy "dps"
mh search --fuzzy "gco main"
```

FTS5 full-text search:

```bash
mh search --fts "docker NEAR ps"
mh search --fts "git"
```

Local natural-language search:

```bash
mh search --semantic "today failed deploy commands in prod"
mh search --nl "root ssh commands from last week"
```

`--semantic` uses local parsing and ranking only. It derives likely filters such as failure/success, date range, environment, SSH/root state, category, and risk terms from the query. Explicit flags still win when you pass them.

Tag and category filters:

```bash
mh search --tag deploy
mh search --category git
mh search docker --tag prod --category docker
```

Pinned commands:

```bash
mh search --pinned
```

Duration filters:

```bash
mh search --duration-gt 1000
mh search --duration-lt 50
mh search pytest --duration-gt 5000
```

Git and environment filters:

```bash
mh search --git-repo /srv/app --git-branch main
mh search --env production
mh last --env staging
```

Output modes:

```bash
mh search docker --json
mh search docker --plain
```

Search mode rule: `--regex`, `--fuzzy`, `--fts`, and `--semantic` are mutually exclusive.

## Terminal UI

Launch the TUI:

```bash
mh tui
mh tui --dashboard
```

Start with a filter:

```bash
mh tui --query docker
mh tui --failed
mh tui --tag deploy
mh tui --category git
mh tui --pinned
mh tui --limit 1000
```

Keyboard actions:

- `Up` / `Down`: move selection
- `PageUp` / `PageDown`: jump by 10 rows
- Type text: fuzzy-filter visible rows
- `Backspace`: remove filter character
- `Enter`: print the selected command after exiting
- `Ctrl-C`: copy the selected command to clipboard
- `p`: pin or unpin the selected command
- `t`: open tag input mode
- `d`: ask for delete confirmation
- `q` / `Esc`: exit

The TUI shows a list on the left and command details on the right. Details include ID, pinned state, masked state, timestamp, exit code, duration, shell, CWD, category, tags, and full command text.

Dashboard mode shows high-level command volume, success/failure split, top commands, risky recent commands, and active environment tiers in one terminal screen.

When stdin or stderr is not a terminal, `mh tui` falls back to normal table output.

## Tags

Add tags to one command:

```bash
mh tag 152 deploy prod
```

Add tags to the last N commands:

```bash
mh tag --last 5 investigation
mh tag --last 3 docker cleanup
```

Remove tags:

```bash
mh untag 152 prod
mh untag 152 deploy prod
```

List known tags with counts:

```bash
mh tags list
```

Search or list by tag:

```bash
mh search --tag investigation
mh last --tag docker
```

## Pins

Pin commands that you want to preserve or find quickly:

```bash
mh pin 152
mh pin 152 153 154
```

Unpin commands:

```bash
mh unpin 152
```

Show pinned commands:

```bash
mh pinned
mh pinned 20
mh pinned --json
mh pinned --plain
```

Pinned commands can be preserved during clear operations:

```bash
mh clear --keep-pinned
```

## Statistics

Show all-time statistics:

```bash
mh stats
```

Time periods:

```bash
mh stats --today
mh stats --week
mh stats --month
```

Show more top entries:

```bash
mh stats --top 20
```

Include category breakdown:

```bash
mh stats --category
```

Show hourly activity:

```bash
mh stats --heatmap
mh stats --week --heatmap
```

Statistics include:

- Total command count
- Successful command count
- Failed command count
- Average duration
- Longest duration
- Top commands
- Top directories
- Shell usage
- Category counts
- Optional hourly heatmap

Only one of `--today`, `--week`, or `--month` can be used at a time.

## Delete And Clear

Delete one command by ID:

```bash
mh delete 152
```

Delete without interactive confirmation:

```bash
mh delete 152 --yes
```

Delete by filters:

```bash
mh delete --older-than 90d
mh delete --older-than 12h
mh delete --older-than 30m
mh delete --contains "temporary command"
mh delete --failed
mh delete --tag scratch
```

`mh delete` requires either an ID or at least one filter. Without `--yes`, it asks for confirmation on a terminal.

Clear matching history:

```bash
mh clear
mh clear --user root
mh clear --before 2026-01-01
mh clear --keep-pinned
```

`mh clear` always asks for confirmation. In non-interactive input, confirmation cannot be provided, so the clear is cancelled.

## Export And Import

By default, text exports **redact secrets** (passwords, tokens, bearer headers). Use `--include-secrets` only for encrypted offline backups you control. SQLite exports support `--sanitize`, `--sanitize-audit`, and `--without-audit`.

Export files are written atomically with mode `0600`. Export refuses to overwrite symlink targets.

Large exports (>100,000 rows) load the full result set into memory; narrow with `--after`, `--before`, or `--tag` when possible.

Export JSON:

```bash
mh export --json history.json
```

Export CSV:

```bash
mh export --csv history.csv
```

Export Markdown:

```bash
mh export --markdown history.md
```

Export compressed JSON with Zstandard:

```bash
mh export --compressed history.json.zst
```

Filter exports:

```bash
mh export --after 2026-05-01 --json may-history.json
mh export --before 2026-01-01 --compressed old-history.json.zst
mh export --tag deploy --json deploy-history.json
mh export --category docker --csv docker-history.csv
```

Import JSON:

```bash
mh import history.json
```

Import CSV:

```bash
mh import history.csv
```

Import compressed JSON:

```bash
mh import history.json.zst
```

Dry-run import:

```bash
mh import history.json --dry-run
```

Import applies the same secret detection and masking rules as live recording. Compressed imports are bounded (64 MB compressed / 256 MB decompressed / 1,000,000 rows) to prevent decompression bombs.

Merge import and skip commands whose command hash already exists:

```bash
mh import history.json --merge
```

Export requires exactly one target format. Import supports JSON, CSV, and `.zst` compressed JSON.

## Snippets

Snippets are named reusable commands stored in the SQLite database.

Save a simple snippet:

```bash
mh snippet save docker-clean "docker system prune -af"
```

Save with description and tags:

```bash
mh snippet save git-undo "git reset --soft HEAD~1" --desc "Undo the last commit softly" --tags "git,undo"
```

List snippets:

```bash
mh snippet list
```

Run a snippet:

```bash
mh snippet run docker-clean
```

Print a snippet without executing:

```bash
mh snippet run docker-clean --dry-run
```

Use placeholders:

```bash
mh snippet save ssh-host "ssh {{user}}@{{host}}" --desc "SSH to a host"
mh snippet run ssh-host --var user=admin --var host=192.168.1.10 --dry-run
mh snippet run ssh-host --var user=admin --var host=192.168.1.10
```

Delete a snippet:

```bash
mh snippet delete docker-clean
```

Export snippets:

```bash
mh snippet export snippets.json
```

Placeholder variables must use `KEY=VALUE` syntax through repeated `--var` options.

## Replay

Replay runs a command from history by ID.

Print the command without running it:

```bash
mh replay 152 --dry-run
```

Run it immediately:

```bash
mh replay 152
```

Ask before running:

```bash
mh replay 152 --confirm
```

Run a safer preview first when mh can derive one:

```bash
mh replay 152 --risk-preview
mh replay 152 --no-risk-guidance
```

Replay with policy approval reason:

```bash
mh replay 152 --reason "change ticket INC-1234" --yes
```

Replay uses `$SHELL -c <command>` (non-login). If `$SHELL` is not set, it falls back to `/bin/sh`.

Before execution, `mh replay` prints warnings when the stored command was masked, still matches secret heuristics, or will run with your current privileges. For risky commands, it also prints safer alternatives, preview commands, and a short review checklist when known. Use `--dry-run` to print the redacted command without executing. `mh runbook run` applies the same warnings per step.

When a matching policy rule has action `deny`, replay is blocked and an audit entry is written. When action is `require_approval`, pass `--reason` or `--yes`.

## Diff

Compare unique command text between two sessions:

```bash
mh diff --session session-a --session session-b
```

Compare two hosts:

```bash
mh diff --host laptop --host server
```

Compare today and yesterday:

```bash
mh diff --today --yesterday
```

`mh diff` prints counts for each side, then commands only present on the left and commands only present on the right.

## Security

`mh` processes every command before storing it.

Default skip rules:

- Empty commands are skipped.
- Commands that start with a space are skipped when `history.ignore_space_prefix` is enabled.
- Exact ignored commands are skipped: `history`, `clear`, `exit`, `logout`, and `mh record`.
- Private mode skips every command.

Default secret detection looks for sensitive terms and patterns such as:

- `password`, `passwd`, `pwd` (context-aware; e.g. `psql -p5432` is a port, not a password)
- `token`, `secret`, `api_key`, `authorization`, `bearer` (including `Authorization: Basic`)
- Inline PEM blocks (`-----BEGIN … PRIVATE KEY-----`)
- Database URLs with credentials (`postgresql://user:pass@host/…`)
- `aws_secret_access_key`, `aws_access_key_id` (including bare `KEY=value` without `export`)
- `github_token`, `gitlab_token`, `PGPASSWORD=`
- `sshpass` (`-p` and `SSHPASS=`), `mysql`/`mariadb` `-p`, `docker login -p`
- `kubectl --token=`, `--from-literal=`, `redis-cli -a`
- `npm_config_*` variables containing `token`, `password`, `secret`, `auth`, or `key`
- `helm … --set …password|secret|token…=`
- `pip install … --password …`
- `poetry config http-basic.*` and `POETRY_*TOKEN*` environment variables
- `cargo login` tokens and `CARGO_REGISTRIES_*_TOKEN`
- `curl -u user:pass` and `curl --user` (scoped to `curl` only)
- `wget --password=`
- Credit-card-like number sequences (Luhn-validated to reduce false positives)

Examples that are masked by default:

```bash
mysql -u root -pSecret123
curl -H "Authorization: Bearer abc123" https://example.test
export AWS_SECRET_ACCESS_KEY=xxxx
AWS_SECRET_ACCESS_KEY=xxxx
sshpass -p password ssh root@1.1.1.1
docker login -u user -p password
kubectl config set-credentials user --token=abc
export GITHUB_TOKEN=ghp_secret
helm upgrade app chart --set secret.password=topsecret
npm_config_//registry.npmjs.org/:_authToken=npm_secret
mysql --password supersecret
sshpass -p mypassword ssh user@host
curl https://user:pass@example.test
```

Quoted and spaced argument forms are handled (`-p'Secret'`, `-p "pass"`, `-u user:pass` on `curl` only). Inline OpenSSH/PEM private key material in a command is masked or skipped.

Switch from masking to skipping secret commands:

```bash
mh config set security.skip_secret_commands true
```

Disable masking:

```bash
mh config set security.mask_secrets false
```

View audit entries:

```bash
mh audit
mh audit --today
mh audit --limit 100
mh audit --format json
mh audit --verify-chain
```

Verify the tamper-evident hash chain:

```bash
mh audit --verify-chain
```

Each audit entry stores `prev_hash` and `entry_hash` fields. Verification walks the chain chronologically and fails if any entry was modified.

Audit entries are created for skipped, masked, risky, policy, replay, break-glass, and purge events when `security.audit_log` is enabled.

## Policy Engine

List configured rules:

```bash
mh policy list
```

Evaluate a command against the active policy:

```bash
mh policy check "rm -rf /"
mh policy check "curl https://x.com | bash" --hostname prod-web --env production --json
```

Default rules:

| Rule ID | Action | Condition |
|---------|--------|-----------|
| `deny-critical-prod` | deny | critical risk in production |
| `approval-critical` | require_approval | critical risk elsewhere |
| `warn-high` | warn | high risk |

Customize rules in `[policy]` in config. Each rule can match on risk level, regex pattern, environment tier, or hostname pattern.

Export, verify, and apply a shared-key signed policy pack:

```bash
export MH_POLICY_PACK_KEY='change-this-shared-secret'
mh policy pack export policy-pack.json
mh policy pack verify policy-pack.json
mh policy pack apply policy-pack.json
```

Use `--key` on individual commands when you do not want to read the key from `MH_POLICY_PACK_KEY`.

## Session Timeline

Inspect every command in a session for incident response:

```bash
mh timeline --session "$MH_SESSION_ID"
mh timeline --session abc-123 --json
```

Output includes command text, timestamps, exit codes, environment tier, and detected risk level.

## Incident Bundles

Export a session bundle for review or handoff:

```bash
mh incident export --session "$MH_SESSION_ID" --output incident.json
```

The bundle includes the session timeline, risky commands, audit-chain verification status, and recent audit events. Command text and audit messages are redacted by default; add `--include-secrets` only for a controlled forensic handoff.

## Legal Hold And Retention

Place a legal hold on a session, command, tag, or Git repository:

```bash
mh hold add incident-2026 --session "$MH_SESSION_ID" --reason "security review"
mh hold add repo-freeze --git-repo /srv/payments --reason "compliance audit"
mh hold list
mh hold remove 1
```

Held commands are excluded from `mh clear`, max-entry enforcement, and retention purge.

Enable retention and purge old records:

```bash
mh config set retention.enabled true
mh config set retention.retention_days 365
mh hold purge --dry-run
mh hold purge
```

## Runbooks

Turn a session into a reusable playbook:

```bash
mh runbook create rollback --session "$MH_SESSION_ID" --desc "Rollback steps"
mh runbook show rollback
mh runbook run rollback --dry-run
```

## Break-Glass Mode

Use during incidents when private mode is on but recording must continue:

```bash
mh break-glass on --reason "database recovery" --ttl-hours 2
# run diagnostic commands — they are recorded and audited
mh break-glass off
```

Break-glass state is stored in `~/.config/mh/break_glass` with an expiry timestamp.

## SIEM Streaming

Enable forwarding in config:

```toml
[siem]
enabled = true
format = "cef"
syslog_url = "siem.example.com:514"
webhook_url = "https://siem.example.com/hooks/mh"
```

View recent audit events on stdout:

```bash
mh watch --limit 50 --format json
```

Webhook delivery requires building with `--features sync` (uses the `reqwest` dependency). Syslog TCP delivery works in the default build.

## Private Mode

Enable private mode:

```bash
mh private on
```

Disable private mode:

```bash
mh private off
```

Check status:

```bash
mh private status
```

Use the environment variable configured by `security.private_mode_env`:

```bash
export MH_PRIVATE=1
```

Commands are not recorded while private mode is active. The file-based private mode state is stored next to the config file.

When policy rules deny a command in production or other environments, recording is skipped silently by default. To surface policy denials in the shell hook stderr stream:

```bash
export MH_POLICY_VERBOSE=1
```

This works together with `MH_RECORD_VERBOSE`, which shows all record diagnostics.

## Encrypted Vault

The vault stores commands encrypted with AES-256-GCM. Vault entries are stored in the `vault` table as encrypted bytes and a nonce. The plaintext command is not listed by `mh vault list`.

Set a passphrase for non-interactive use:

```bash
export MH_VAULT_PASSPHRASE='change-this-passphrase'
```

Add a vault command:

```bash
mh vault add "kubectl exec -it prod-pod -- /bin/sh" --label prod-shell
mh vault add "psql postgresql://user:secret@db/prod" --label prod-db
```

List vault entries:

```bash
mh vault list
```

Print a decrypted command without running it:

```bash
mh vault run 3 --dry-run
```

Run a vault command:

```bash
mh vault run 3
```

Delete a vault entry:

```bash
mh vault delete 3
```

Check passphrase prompting:

```bash
mh vault unlock
```

Clear persistent unlocked state:

```bash
mh vault lock
```

Current vault behavior:

- `MH_VAULT_PASSPHRASE` is used first when it is set.
- Otherwise `mh` prompts for a passphrase without echoing it.
- `mh vault lock` reports that there is no persistent unlocked process state.
- The `vault.use_keyring` config key is reserved for OS keyring integration; this build does not store vault passphrases in the OS keyring.

## Sync

Build with sync support to enable encrypted remote push/pull:

```bash
cargo build --release --features sync
```

`mh sync` stores local sync settings and, when built with the `sync` feature, encrypts history with AES-256-GCM before sending it to a compatible server at `/api/v1/sync/push` and `/api/v1/sync/pull`.

Show sync status:

```bash
mh sync status
```

Save server URL and token:

```bash
mh sync setup https://mh.example.test token-value
```

Enable sync state:

```bash
mh sync enable
```

Disable sync state:

```bash
mh sync disable
```

Tune interval:

```bash
mh config set sync.auto_sync_interval_minutes 15
```

Current push/pull behavior:

```bash
mh sync push
mh sync pull
```

These commands require building with `--features sync` and a compatible remote server. The token is stored in the local config file, so do not commit or share `config.toml`.

## Doctor

Run health checks:

```bash
mh doctor
mh doctor --strict    # non-zero exit when any warning is reported
mh doctor --json      # structured report on stdout (no styled text)
```

`mh doctor` checks:

- Config file loading and validation (`mh config validate` hints on failure)
- Environment overrides (`MH_CONFIG`, `MH_DB`) — rejects `MH_DB` when it points to a directory; warns when the database path is owned by another user or lives under another user's `/home`
- Database opening and writable data directory
- Config/database/WAL file permissions and symlink targets
- Database file size versus configured max size
- Available disk space on the database volume (≥100 MB recommended free)
- SQLite integrity
- Schema version (currently **11**; pending migrations reported with upgrade hint)
- Tamper-evident audit chain verification when enterprise tables exist
- Command count
- Security and policy engine configuration (invalid ignore regex reported)
- Duplicate shell hook lines and managed integration blocks
- Private mode marker/env
- Record daemon status (running, stale PID/socket, or `MH_NO_DAEMON`)
- Sync enabled/configured state
- Vault enabled state and keyring preference
- Current shell and whether `mh` is available in `PATH`

Human-readable example:

```text
[OK]   Config loaded from /home/user/.config/mh/config.toml
[OK]   Database opened at /home/user/.local/share/mh/history.db
[INFO] Database size: 0.25 MB / 512 MB
[OK]   Database integrity check passed
[OK]   Schema version: 11
[INFO] Command count: 42
[INFO] Record daemon: running (pid 12345)
[INFO] Sync: disabled (no server configured)
[INFO] Vault config: disabled (keyring: enabled)
[INFO] Current shell: /usr/bin/zsh
[OK]   Binary is available in PATH
```

JSON example (automation/CI):

```bash
mh doctor --json | jq .
```

```json
{
  "status": "ok",
  "warning_count": 0,
  "checks": [
    { "code": "check_…", "level": "ok", "message": "Config loaded from …" }
  ],
  "summary": {
    "mh_version": "0.1.0",
    "config_path": "/home/user/.config/mh/config.toml",
    "database_path": "/home/user/.local/share/mh/history.db",
    "schema_version": 11,
    "command_count": 42,
    "daemon_running": true,
    "private_mode": false,
    "strict": false
  }
}
```

## Output Formats

Table output is the default for user-facing commands:

```bash
mh search docker
mh last
mh pinned
```

JSON output:

```bash
mh search docker --json
mh last --json
mh pinned --json
mh audit --format json
```

Plain command output:

```bash
mh search docker --plain
mh last --plain
mh pinned --plain
```

File exports:

```bash
mh export --json history.json
mh export --csv history.csv
mh export --markdown history.md
mh export --compressed history.json.zst
```

## Completions And Man Page

Generate completions:

```bash
mh completions bash --output mh.bash
mh completions zsh --output _mh
mh completions fish --output mh.fish
mh completions power-shell --output mh.ps1
mh completions elvish --output mh.elv
```

Print completion script to stdout:

```bash
mh completions zsh
```

Generate a man page:

```bash
mh man --output mh.1
```

Print the man page to stdout:

```bash
mh man
```

The installer installs generated Bash, Zsh, and Fish completions and a man page automatically unless `--no-completions` or `--no-man` is used.

## About

Show application and developer metadata:

```bash
mh about
```

Example:

```text
mh 0.1.0
Author: Cuma Kurt <cumakurt@gmail.com>
GitHub: https://github.com/cumakurt/mh
LinkedIn: https://www.linkedin.com/in/cuma-kurt-34414917/
```

The same developer metadata is also present in CLI help output.

## Debian Package

Build a `.deb` package:

```bash
scripts/package-deb.sh
```

Use an existing release binary:

```bash
scripts/package-deb.sh --no-build
```

Write artifacts to another directory:

```bash
scripts/package-deb.sh --output-dir /tmp/mh-package
```

The package installs:

- `/usr/bin/mh`
- Bash completion
- Zsh completion
- Fish completion
- `mh.1.gz` man page

Inspect the package:

```bash
dpkg-deb --info target/package/mh_0.1.0_amd64.deb
dpkg-deb --contents target/package/mh_0.1.0_amd64.deb
```

Install it:

```bash
sudo dpkg -i target/package/mh_0.1.0_amd64.deb
```

## Database Schema

The schema is applied through numbered SQL migrations (currently version **11**). Existing databases migrate automatically on first open after an upgrade. Core tables:

- `commands` — command records with metadata, environment tier, and legal-hold flag
- `sessions`
- `tags`
- `snippets`
- `audit_log` — security events with SHA-256 hash chain columns (`prev_hash`, `entry_hash`)
- `vault`
- `commands_fts` — FTS5 virtual table (kept in sync via triggers since schema v10)
- `legal_holds` — legal hold definitions
- `runbooks` and `runbook_steps` — reusable command sequences
- `purge_audit` — retention purge audit trail

Important indexes:

- `idx_commands_command`
- `idx_commands_cwd`
- `idx_commands_started_at`
- `idx_commands_exit_code`
- `idx_commands_user`
- `idx_commands_hostname`
- `idx_commands_session`
- `idx_commands_category`
- `idx_commands_is_pinned`
- `idx_commands_dedupe_lookup` — `(command, cwd, started_at DESC)` for duplicate-window lookups (v11)
- `idx_tags_tag`
- `idx_tags_command_id`

FTS5 is used by `mh search --fts`. Duplicate commands within `history.dedupe_window_seconds` are suppressed using a transactional insert (`BEGIN IMMEDIATE`) so parallel terminals do not create duplicate rows.

## Environment Variables

Runtime:

- `MH_CONFIG`: override config file path.
- `MH_DB`: override database file path.
- `MH_CONFIG_NO_CACHE`: bypass in-process config/engine cache (tests and debugging).
- `MH_NO_DAEMON`: force shell hooks to write directly to SQLite instead of the record daemon.
- `MH_SKIP_GIT_DETECT`: skip Git metadata subprocess during record (set by default in shell hooks).
- `MH_PRIVATE`: default private mode environment variable.
- `MH_POLICY_VERBOSE`: print policy denial messages from shell hooks to stderr.
- `MH_RECORD_VERBOSE`: print record diagnostics from shell hooks to stderr.
- `MH_SESSION_ID`: shell session identifier used by hooks.
- `MH_PICK_LIMIT`: default result limit for up-arrow picker integration.
- `MH_HISTORY_ARROWS`: when set (e.g. `1`), bind left/right arrow keys to step through recent `mh` commands on the prompt (Bash, Zsh, Fish).
- `MH_VAULT_PASSPHRASE`: non-interactive vault passphrase source.
- `SHELL`: shell used by replay, snippets, vault run, and shell detection.
- `EDITOR`: editor used by `mh config edit`.

Installer:

- `INSTALL_DIR`: binary install directory.
- `MH_SHELL`: shell override for installation.

## Practical Workflows

Review recent failures in a project:

```bash
mh search --failed --cwd "$PWD"
```

Find a Docker command from memory:

```bash
mh search --fuzzy "dps"
```

Pin the last command and tag it:

```bash
mh pin "$(mh last 1 --json | jq '.[0].id')"
mh tag --last 1 important
```

Export this month's Git commands:

```bash
mh export --after 2026-05-01 --category git --json git-may.json
```

Save and test a reusable SSH command:

```bash
mh snippet save ssh-host "ssh {{user}}@{{host}}"
mh snippet run ssh-host --var user=admin --var host=10.0.0.5 --dry-run
```

Work without recording:

```bash
mh private on
# run sensitive commands
mh private off
```

Store a sensitive command outside normal history:

```bash
export MH_VAULT_PASSPHRASE='change-this-passphrase'
mh vault add "kubectl exec -it prod-pod -- /bin/sh" --label prod-shell
mh vault run 1 --dry-run
```

Investigate a session after an incident:

```bash
mh timeline --session "$MH_SESSION_ID" --json
mh audit --verify-chain
mh hold add incident --session "$MH_SESSION_ID" --reason "forensics"
```

Check policy before running a risky replay:

```bash
mh policy check "rm -rf /tmp/cache"
mh replay 42 --reason "approved cleanup" --yes
```

Freeze production history during an audit:

```bash
mh hold add audit-q2 --git-repo /srv/payments --reason "quarterly review"
mh search --env production --limit 100
```

## Current Limitations

- Remote sync requires `--features sync` and a compatible server endpoint.
- SIEM webhook delivery requires `--features sync`; syslog TCP works in the default build.
- Fleet control plane (`mh-server`) is not included in this release.
- Vault passphrases can be stored in the OS keyring when `vault.use_keyring = true`. `MH_VAULT_PASSPHRASE` remains supported.
- PowerShell integration targets PowerShell 7+ with PSReadLine available.
- Import supports JSON, CSV, and compressed JSON. Markdown export is for human-readable reports, not re-import.
- `mh init` prints an activation command because a child process cannot directly source the already-running parent shell.
- JSON/CSV export loads the full filtered result set into memory; very large exports may need narrower date/tag filters.

## Development Checklist

Run the full command verification suite before release:

```bash
./scripts/verify-all-commands.sh
cargo test                    # 250+ tests (unit, integration, security, shell, migration, doctor)
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Continuous integration

GitHub Actions workflows under `.github/workflows/`:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | push/PR to `main`/`master` | fmt, check, clippy, full test suite (incl. `sync` feature), record benchmarks (≤20 ms/op), search bench at 10k + 100k with `MH_BENCH_ASSERT`, `mh doctor --json` validation, `cargo audit`, release builds |
| `zsh-smoke.yml` | push/PR + manual | `scripts/zsh-smoke.sh` — Zsh hook + `__mh_now_ms` + record path (Kali-style non-interactive) |
| `bench-load.yml` | weekly + manual | Search/load benchmark at 100k–1M rows (`MH_BENCH_SEARCH_SIZE`) |
| `release.yml` | tags | Release artifacts |

Performance thresholds enforced in CI:

```bash
cargo bench --bench record_bench              # direct insert; must stay under 20 ms/op
cargo bench --bench record_pipeline_bench     # full record path; must stay under 20 ms/op
MH_BENCH_ASSERT=1 cargo bench --bench search_bench   # default 10k rows

# Optional local load test (100k–1M rows)
MH_BENCH_SEARCH_SIZE=100000 MH_BENCH_ASSERT=1 \
  MH_BENCH_MAX_FTS_MS=100 MH_BENCH_MAX_LAST_MS=50 MH_BENCH_MAX_FUZZY_MS=250 \
  cargo bench --bench search_bench
```

Before submitting changes:

```bash
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Useful smoke test:

```bash
tmpdir="$(mktemp -d)"
export MH_CONFIG="$tmpdir/config.toml"
export MH_DB="$tmpdir/history.db"
target/release/mh record --command "git status" --cwd "$PWD" --shell zsh --exit-code 0 --duration-ms 12 --tags "git,test"
target/release/mh last
target/release/mh search --category git
target/release/mh stats --heatmap
target/release/mh export --json "$tmpdir/history.json"
target/release/mh import "$tmpdir/history.json" --dry-run
target/release/mh doctor
target/release/mh doctor --json | jq -e '.status'
```

## License

`mh` is **free software** released under the **GNU Affero General Public License v3.0 or later** (AGPL-3.0-or-later).

- Full license text: [LICENSE](LICENSE)
- SPDX identifier: `AGPL-3.0-or-later`

You are free to use, study, modify, and redistribute this program in accordance with the AGPL. Copyleft obligations apply, including when you offer the software to others over a network (SaaS/API). Derivative works must remain under the same license and include prominent notices as required by the AGPL.

Copyright © 2026 Cuma Kurt. See [LICENSE](LICENSE) for the complete terms.
