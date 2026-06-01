//! Shared shell integration fragments embedded into per-shell scripts.

macro_rules! bash_zsh_time_helpers {
    () => {
        r#"
# mh portable millisecond clock (GNU date, bash 5+ EPOCHREALTIME, python3, perl)
__mh_now_ms() {
  if [[ -n "${EPOCHREALTIME-}" ]]; then
    local _mh_sec="${EPOCHREALTIME%%.*}"
    local _mh_frac="${EPOCHREALTIME#*.}"
    _mh_frac="${_mh_frac:-0}"
    printf '%s' $(( _mh_sec * 1000 + 10#${_mh_frac:0:3} ))
    return 0
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null && return 0
  fi
  if command -v perl >/dev/null 2>&1; then
    perl -MTime::HiRes=time -e 'print int(time()*1000)' 2>/dev/null && return 0
  fi
  local _mh_ms=""
  _mh_ms="$(date +%s%3N 2>/dev/null || true)"
  if [[ "${_mh_ms}" =~ ^[0-9]+$ ]]; then
    printf '%s' "${_mh_ms}"
    return 0
  fi
  printf '%s000' "$(date +%s 2>/dev/null || echo 0)"
}
"#
    };
}

macro_rules! fish_time_helpers {
    () => {
        r#"
# mh portable millisecond clock for fish
function __mh_now_ms
  if command -vq python3
    python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null
    return
  end
  if command -vq perl
    perl -MTime::HiRes=time -e 'print int(time()*1000)' 2>/dev/null
    return
  end
  set -l _mh_ms (date +%s%3N 2>/dev/null | string trim)
  if test -n "$_mh_ms"; and string match -qr '^[0-9]+$' -- $_mh_ms
    echo $_mh_ms
    return
  end
  math "(date +%s) * 1000"
end
"#
    };
}

macro_rules! mh_record_helper_bash_zsh {
    () => {
        r#"
__mh_record() {
  if [[ -n "${MH_RECORD_VERBOSE:-}" || -n "${MH_POLICY_VERBOSE:-}" ]]; then
    command mh record "$@" 1>&2
  else
    command mh record "$@" >/dev/null 2>&1
  fi
}
"#
    };
}

macro_rules! mh_policy_helpers_bash_zsh {
    () => {
        r#"
__mh_policy_skip() {
  case "$1" in
    ''|__mh_*|_mh_*|trap\ *|PROMPT_COMMAND=*|local\ *|unset\ *|return\ *|export\ MH_*|mh\ *|command\ mh\ *|*/mh\ *) return 0 ;;
  esac
  return 1
}

__mh_policy_allow() {
  __mh_policy_skip "$1" && return 0
  command mh policy check --command "$1" --cwd "$PWD" --quiet 2>/dev/null
}
"#
    };
}

macro_rules! mh_bash_accept_line {
    () => {
        r#"
  __mh_dispatch_accept() {
    local cmd="$READLINE_LINE"
    if __mh_policy_skip "$cmd"; then
      READLINE_LINE=""
      READLINE_POINT=0
      return 0
    fi
    if ! __mh_policy_allow "$cmd"; then
      READLINE_LINE=""
      READLINE_POINT=0
      return 0
    fi
    trap - DEBUG
    history -s "$cmd" 2>/dev/null || true
    eval "$cmd"
    READLINE_LINE=""
    READLINE_POINT=0
    trap '__mh_preexec' DEBUG
  }
  bind -x '"\r": __mh_dispatch_accept' 2>/dev/null || true
  bind -x '"\C-m": __mh_dispatch_accept' 2>/dev/null || true
"#
    };
}

macro_rules! mh_zsh_accept_line {
    () => {
        r#"
  _mh_accept_line() {
    local cmd="$BUFFER"
    if __mh_policy_skip "$cmd"; then
      zle .accept-line
      return
    fi
    if ! __mh_policy_allow "$cmd"; then
      zle -M "mh: policy blocked"
      return 1
    fi
    zle .accept-line
  }
  zle -N accept-line _mh_accept_line
"#
    };
}

macro_rules! mh_record_helper_fish {
    () => {
        r#"
function __mh_record
  if set -q MH_RECORD_VERBOSE; or set -q MH_POLICY_VERBOSE
    command mh record $argv 1>&2
  else
    command mh record $argv >/dev/null 2>&1
  end
end
"#
    };
}

pub const BASH_INTEGRATION: &str = concat!(
    "# mh shell integration for bash\n",
    bash_zsh_time_helpers!(),
    mh_record_helper_bash_zsh!(),
    mh_policy_helpers_bash_zsh!(),
    r#"
if [[ -z "${__MH_BASH_INTEGRATION_LOADED:-}" ]]; then
__MH_BASH_INTEGRATION_LOADED=1

if [[ -z "${MH_SESSION_ID:-}" ]]; then
  export MH_SESSION_ID="$(date +%s)-$$"
fi
: "${MH_SKIP_GIT_DETECT:=1}"
export MH_SKIP_GIT_DETECT

__mh_preexec() {
  local current_command="$BASH_COMMAND"
  case "$current_command" in
    __mh_*|_mh_*|trap\ *|PROMPT_COMMAND=*|local\ *|unset\ *|return\ *|export\ MH_*|mh\ *|command\ mh\ *|*/mh\ *) return ;;
  esac
  MH_LAST_COMMAND="$current_command"
  MH_START_TIME="$(__mh_now_ms)"
}

__mh_precmd() {
  local exit_code=$?
  if [[ -n "${MH_LAST_COMMAND:-}" ]]; then
    local end_time="$(__mh_now_ms)"
    local duration_ms=0
    if [[ -n "${MH_START_TIME:-}" ]]; then
      duration_ms=$((end_time - MH_START_TIME))
    fi
    __mh_record \
      --command "$MH_LAST_COMMAND" \
      --cwd "$PWD" \
      --shell "bash" \
      --exit-code "$exit_code" \
      --duration-ms "$duration_ms" \
      --session-id "$MH_SESSION_ID"
    unset MH_LAST_COMMAND MH_START_TIME
  fi
}

trap '__mh_preexec' DEBUG
case ";$PROMPT_COMMAND;" in
  *";__mh_precmd;"*) ;;
  *) PROMPT_COMMAND="__mh_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
esac

if [[ $- == *i* ]]; then
  __mh_history_picker() {
    trap - DEBUG
    local selected
    selected="$(command mh pick --limit "${MH_PICK_LIMIT:-100}" </dev/tty)"
    if [[ -n "$selected" ]]; then
      READLINE_LINE="$selected"
      READLINE_POINT="${#READLINE_LINE}"
    fi
    trap '__mh_preexec' DEBUG
  }

  bind -x '"\e[A": __mh_history_picker'
  bind -x '"\eOA": __mh_history_picker'
"#,
    mh_bash_accept_line!(),
    r#"
fi
fi
"#
);

pub const ZSH_INTEGRATION: &str = concat!(
    "# mh shell integration for zsh\n",
    "zmodload zsh/datetime 2>/dev/null\n",
    bash_zsh_time_helpers!(),
    mh_record_helper_bash_zsh!(),
    mh_policy_helpers_bash_zsh!(),
    r#"
if [[ -z "${__MH_ZSH_INTEGRATION_LOADED:-}" ]]; then
__MH_ZSH_INTEGRATION_LOADED=1

if [[ -z "${MH_SESSION_ID:-}" ]]; then
  export MH_SESSION_ID="$(date +%s)-$$"
fi
: "${MH_SKIP_GIT_DETECT:=1}"
export MH_SKIP_GIT_DETECT

_mh_preexec() {
  case "$1" in
    __mh_*|_mh_*|trap\ *|PROMPT_COMMAND=*|local\ *|unset\ *|return\ *|export\ MH_*|mh\ *|command\ mh\ *|*/mh\ *) return ;;
  esac
  MH_LAST_COMMAND="$1"
  MH_START_TIME="$(__mh_now_ms)"
}

_mh_precmd() {
  local exit_code=$?
  if [[ -n "${MH_LAST_COMMAND:-}" ]]; then
    local end_time="$(__mh_now_ms)"
    local duration_ms=0
    if [[ -n "${MH_START_TIME:-}" ]]; then
      duration_ms=$((end_time - MH_START_TIME))
    fi
    __mh_record \
      --command "$MH_LAST_COMMAND" \
      --cwd "$PWD" \
      --shell "zsh" \
      --exit-code "$exit_code" \
      --duration-ms "$duration_ms" \
      --session-id "$MH_SESSION_ID"
    unset MH_LAST_COMMAND MH_START_TIME
  fi
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec _mh_preexec
add-zsh-hook precmd _mh_precmd

if [[ -o interactive ]]; then
  _mh_history_picker() {
    local selected
    selected="$(command mh pick --limit "${MH_PICK_LIMIT:-100}" </dev/tty)"
    if [[ -n "$selected" ]]; then
      BUFFER="$selected"
      CURSOR=${#BUFFER}
      zle redisplay
    fi
  }

  zle -N _mh_history_picker
  bindkey '^[[A' _mh_history_picker
  bindkey '^[OA' _mh_history_picker
"#,
    mh_zsh_accept_line!(),
    r#"
fi
fi
"#
);

pub const FISH_INTEGRATION: &str = concat!(
    "# mh shell integration for fish\n",
    fish_time_helpers!(),
    mh_record_helper_fish!(),
    r#"
if not set -q __mh_fish_integration_loaded
  set -g __mh_fish_integration_loaded 1

if not set -q MH_SESSION_ID
  set -gx MH_SESSION_ID (date +%s)-$fish_pid
end
if not set -q MH_SKIP_GIT_DETECT
  set -gx MH_SKIP_GIT_DETECT 1
end

function __mh_policy_allow_fish -a cmd
  switch $cmd
    case '' '__mh_*' '_mh_*' 'mh *' 'command mh *' '*/mh *'
      return 0
  end
  command mh policy check --command "$cmd" --cwd "$PWD" --quiet 2>/dev/null
end

function mh_preexec --on-event fish_preexec
  switch $argv[1]
    case '__mh_*' '_mh_*' 'mh *' 'command mh *' '*/mh *' 'set -e MH_*' 'set -g MH_*' 'set -x MH_*' 'set -u MH_*' 'export MH_*' 'function *' 'end' 'return' 'local *' 'unset *'
      return
  end
  if not __mh_policy_allow_fish $argv[1]
    echo "mh: policy blocked" >&2
    commandline -f cancel
    return 1
  end
  set -g MH_LAST_COMMAND $argv[1]
  set -g MH_START_TIME (__mh_now_ms)
end

function mh_postexec --on-event fish_postexec
  set -l exit_code $status
  if set -q MH_LAST_COMMAND
    set -l end_time (__mh_now_ms)
    set -l duration_ms 0
    if set -q MH_START_TIME
      set duration_ms (math "$end_time - $MH_START_TIME")
    end
    __mh_record \
      --command "$MH_LAST_COMMAND" \
      --cwd "$PWD" \
      --shell "fish" \
      --exit-code "$exit_code" \
      --duration-ms "$duration_ms" \
      --session-id "$MH_SESSION_ID"
    set -e MH_LAST_COMMAND
    set -e MH_START_TIME
  end
end

function mh_history_picker
  set -l mh_pick_limit 100
  if set -q MH_PICK_LIMIT
    set mh_pick_limit $MH_PICK_LIMIT
  end

  set -l selected (command mh pick --limit $mh_pick_limit </dev/tty)
  if test -n "$selected"
    commandline --replace "$selected"
    commandline --cursor (string length -- "$selected")
  end
end

bind \e\[A mh_history_picker
bind \eOA mh_history_picker
end
"#
);
