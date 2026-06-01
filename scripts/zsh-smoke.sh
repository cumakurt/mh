#!/usr/bin/env bash
# Smoke-test mh Zsh integration in a non-interactive shell (CI-friendly).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MH="${MH_BIN:-$ROOT/target/release/mh}"

if [[ ! -x "$MH" ]]; then
  echo "mh binary not found at $MH (run: cargo build --release)" >&2
  exit 1
fi

CONFIG_HOME="$(mktemp -d)"
DATA_HOME="$(mktemp -d)"
INIT_FILE="$(mktemp)"
trap 'rm -rf "$CONFIG_HOME" "$DATA_HOME" "$INIT_FILE"' EXIT

export PATH="$(dirname "$MH"):$PATH"
export XDG_CONFIG_HOME="$CONFIG_HOME"
export XDG_DATA_HOME="$DATA_HOME"
export MH_CONFIG_NO_CACHE=1
export MH_NO_DAEMON=1

"$MH" init zsh >"$INIT_FILE"

if ! grep -q '__mh_now_ms' "$INIT_FILE"; then
  echo "zsh integration missing portable clock helper" >&2
  exit 1
fi

if ! grep -q 'add-zsh-hook preexec _mh_preexec' "$INIT_FILE"; then
  echo "zsh integration missing preexec hook" >&2
  exit 1
fi

zsh -fc "
  set -e
  source '$INIT_FILE'
  MH_LAST_COMMAND='echo zsh-smoke-ok'
  MH_START_TIME=\$(__mh_now_ms)
  _mh_precmd
"

LAST="$("$MH" last 5)"
if ! grep -q 'zsh-smoke-ok' <<<"$LAST"; then
  echo "expected recorded command in mh last output, got: $LAST" >&2
  exit 1
fi

echo "zsh smoke: ok"
