#!/usr/bin/env bash
# Verify every mh subcommand runs without error in an isolated demo environment.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MH="${MH_BIN:-$ROOT/target/release/mh}"
DEMO="$(mktemp -d)"
export XDG_CONFIG_HOME="$DEMO/config"
export XDG_DATA_HOME="$DEMO/data"
export MH_VAULT_PASSPHRASE="demo-passphrase"
SESSION="demo-session-001"
PASS=0
FAIL=0
SKIP=0

mkdir -p "$XDG_CONFIG_HOME/mh" "$XDG_DATA_HOME/mh" "$DEMO/out" "$ROOT/docs/examples"

log() { printf '[verify] %s\n' "$*"; }
ok() { PASS=$((PASS + 1)); log "OK   $*"; }
bad() { FAIL=$((FAIL + 1)); log "FAIL $*"; }
skip() { SKIP=$((SKIP + 1)); log "SKIP $*"; }

run() {
  local name=$1
  shift
  log "RUN  $name: $*"
  if "$@" >"$DEMO/out/${name}.stdout" 2>"$DEMO/out/${name}.stderr"; then
    ok "$name"
    return 0
  fi
  bad "$name"
  sed 's/^/    /' "$DEMO/out/${name}.stderr" >&2 || true
  return 1
}

run_expect_fail() {
  local name=$1
  shift
  log "RUN  $name (expect fail): $*"
  if "$@" >"$DEMO/out/${name}.stdout" 2>"$DEMO/out/${name}.stderr"; then
    bad "$name (expected failure)"
    return 1
  fi
  ok "$name (failed as expected)"
}

seed_history() {
  "$MH" private off >/dev/null 2>&1 || true
  "$MH" record --command "git status" --cwd /tmp/demo --shell zsh --exit-code 0 --duration-ms 42 \
    --session-id "$SESSION" --tags "git,work" >/dev/null
  "$MH" record --command "docker ps -a" --cwd /tmp/demo --shell zsh --exit-code 0 --duration-ms 120 \
    --session-id "$SESSION" --tags "docker" >/dev/null
  "$MH" record --command "curl https://example.com" --cwd /tmp/demo --shell zsh --exit-code 0 \
    --duration-ms 250 --session-id "$SESSION" >/dev/null
  "$MH" record --command "pytest tests/unit" --cwd /tmp/demo --shell zsh --exit-code 1 \
    --duration-ms 5400 --session-id "$SESSION" >/dev/null
  "$MH" record --command "rm -rf /tmp/sandbox" --cwd /tmp/demo --shell zsh --exit-code 0 \
    --duration-ms 5 --session-id "$SESSION" >/dev/null
  "$MH" record --command "kubectl get pods" --cwd /tmp/demo --shell zsh --exit-code 0 \
    --duration-ms 800 --session-id "$SESSION" --env-context docker >/dev/null
}

save_example() {
  local name=$1
  local cmd=$2
  {
    printf '$ %s\n' "$cmd"
    cat "$DEMO/out/${name}.stdout"
  } >"$ROOT/docs/examples/${name}.txt"
}

if [[ ! -x "$MH" ]]; then
  log "Building release binary..."
  (cd "$ROOT" && env -u CARGO_TARGET_DIR cargo build --release)
fi

log "Demo directory: $DEMO"
seed_history

# Core
run about "$MH" about
run version "$MH" --version
run help "$MH" --help
run doctor "$MH" doctor
run init-bash "$MH" init bash
run init-zsh "$MH" init zsh
run init-fish "$MH" init fish
run init-nushell "$MH" init nushell

# Config
run config-show "$MH" config show
run config-path "$MH" config path
run config-validate "$MH" config validate
run config-set "$MH" config set display.default_limit 25
run config-reset "$MH" config reset

# History views
run last "$MH" last 5
run last-json "$MH" last 2 --json
run last-plain "$MH" last 2 --plain
run last-failed "$MH" last --failed
run search "$MH" search docker --limit 5
run search-fts "$MH" search --fts git --limit 5
run search-fuzzy "$MH" search --fuzzy dps --limit 5
run search-json "$MH" search docker --json --limit 2
run pick "$MH" pick --query docker --limit 5
run tui "$MH" tui --limit 5

# Stats & diff
run stats "$MH" stats --top 5
run stats-today "$MH" stats --today --top 3
run diff-session "$MH" diff --session "$SESSION" --session other-session

# Tags & pins
ID=$("$MH" last 1 --json | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["id"])')
run tag "$MH" tag "$ID" demo tagged
run tags-list "$MH" tags list
run pin "$MH" pin "$ID"
run pinned "$MH" pinned 5
run unpin "$MH" unpin "$ID"
run untag "$MH" untag "$ID" demo

# Risk & context
run risk-list "$MH" risk list
run risk-check "$MH" risk check "rm -rf /"
run risk-scan "$MH" risk scan --limit 10
run context "$MH" context
run context-repos "$MH" context repos
run context-branches "$MH" context branches
run context-history "$MH" context history --limit 5

# Snippets
run snippet-save "$MH" snippet save demo-ls "ls -la {{path}}" --desc "List directory" --tags demo
run snippet-list "$MH" snippet list
run snippet-run "$MH" snippet run demo-ls --var path=/tmp --dry-run
run snippet-export "$MH" snippet export "$DEMO/out/snippets.json"

# Security
run audit "$MH" audit --limit 5
run audit-json "$MH" audit --format json --limit 2
run audit-verify "$MH" audit --verify-chain
run private-status "$MH" private status
run private-on "$MH" private on
run private-off "$MH" private off

# Enterprise
run policy-list "$MH" policy list
run policy-check "$MH" policy check "rm -rf /"
run timeline "$MH" timeline --session "$SESSION" --plain
run hold-add "$MH" hold add incident-demo --session "$SESSION" --reason demo
run hold-list "$MH" hold list
run hold-purge-dry "$MH" hold purge --dry-run
run runbook-create "$MH" runbook create deploy-demo --session "$SESSION" --desc "Demo flow"
run runbook-list "$MH" runbook list
run runbook-show "$MH" runbook show deploy-demo
run runbook-run "$MH" runbook run deploy-demo --dry-run
run break-glass-on "$MH" break-glass on --reason "demo incident" --ttl-hours 1
run break-glass-status "$MH" break-glass status
run break-glass-off "$MH" break-glass off
run watch "$MH" watch --limit 5 --format json

# Vault
run vault-add "$MH" vault add "echo vault-demo" --label demo
run vault-list "$MH" vault list
run vault-run "$MH" vault run 1 --dry-run
run vault-unlock "$MH" vault unlock
run vault-lock "$MH" vault lock
run vault-delete "$MH" vault delete 1

# Export / import
run export-json "$MH" export --json "$DEMO/out/history.json"
run export-csv "$MH" export --csv "$DEMO/out/history.csv"
run export-md "$MH" export --markdown "$DEMO/out/history.md"
run export-sqlite "$MH" export --sqlite "$DEMO/out/history-copy.db"
run import-dry "$MH" import "$DEMO/out/history.json" --dry-run

# Sync
run sync-status "$MH" sync status
run sync-setup "$MH" sync setup https://mh.example.test demo-token
run sync-enable "$MH" sync enable
run sync-disable "$MH" sync disable
run_expect_fail sync-push "$MH" sync push
run_expect_fail sync-pull "$MH" sync pull

# Replay & delete
run replay-dry "$MH" replay "$ID" --dry-run
run delete-yes "$MH" delete "$ID" --yes

# Completions & man
run completions-bash "$MH" completions bash
run completions-zsh "$MH" completions zsh
run man "$MH" man

# Save README examples for successful commands
EXAMPLE_CMD=(
  "about|mh about"
  "doctor|mh doctor"
  "init-zsh|mh init zsh"
  "config-show|mh config show"
  "last|mh last 5"
  "search|mh search docker --limit 5"
  "search-fuzzy|mh search --fuzzy dps --limit 5"
  "pick|mh pick --query docker --limit 5"
  "tui|mh tui --limit 5"
  "stats|mh stats --top 5"
  "diff-session|mh diff --session demo-session-001 --session other-session"
  "tags-list|mh tags list"
  "pinned|mh pinned 5"
  "risk-list|mh risk list"
  "risk-check|mh risk check \"rm -rf /\""
  "risk-scan|mh risk scan --limit 5"
  "context|mh context"
  "context-repos|mh context repos"
  "context-history|mh context history --limit 5"
  "snippet-list|mh snippet list"
  "snippet-run|mh snippet run demo-ls --var path=/tmp --dry-run"
  "audit|mh audit --limit 5"
  "private-status|mh private status"
  "vault-list|mh vault list"
  "sync-status|mh sync status"
  "replay-dry|mh replay 6 --dry-run"
  "export-json|mh export --json history.json"
  "import-dry|mh import history.json --dry-run"
  "completions-zsh|mh completions zsh | head -n 8"
  "man|mh man | head -n 12"
)

for entry in "${EXAMPLE_CMD[@]}"; do
  name=${entry%%|*}
  cmd=${entry#*|}
  if [[ -f "$DEMO/out/${name}.stdout" ]]; then
    save_example "$name" "$cmd"
  fi
done

log "Finished: $PASS passed, $FAIL failed, $SKIP skipped"
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
