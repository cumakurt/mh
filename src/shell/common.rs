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
  command mh policy check "$1" --cwd "$PWD" --quiet 2>/dev/null
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
  __mh_history_index=-1
  __mh_history_saved_line=""
  __mh_history_current_line=""

  __mh_history_reset() {
    __mh_history_index=-1
    __mh_history_saved_line=""
    __mh_history_current_line=""
  }

  __mh_history_load() {
    local selected
    selected="$(command mh last 1 --plain --offset "$__mh_history_index" 2>/dev/null)"
    if [[ -n "$selected" ]]; then
      READLINE_LINE="$selected"
      READLINE_POINT="${#READLINE_LINE}"
      __mh_history_current_line="$selected"
      return 0
    fi
    return 1
  }

  __mh_history_step() {
    local direction="$1"
    if [[ "${__mh_history_index:--1}" -ge 0 && "$READLINE_LINE" != "$__mh_history_current_line" ]]; then
      __mh_history_reset
    fi

    if [[ "$direction" == "older" ]]; then
      if [[ "${__mh_history_index:--1}" -lt 0 && "${READLINE_POINT:-0}" -lt "${#READLINE_LINE}" ]]; then
        READLINE_POINT=$((READLINE_POINT + 1))
        return 0
      fi
      if [[ "${__mh_history_index:--1}" -lt 0 ]]; then
        __mh_history_saved_line="$READLINE_LINE"
        __mh_history_index=0
      else
        __mh_history_index=$((__mh_history_index + 1))
      fi
      if ! __mh_history_load; then
        if [[ "$__mh_history_index" -eq 0 ]]; then
          __mh_history_reset
        else
          __mh_history_index=$((__mh_history_index - 1))
        fi
      fi
      return 0
    fi

    if [[ "${__mh_history_index:--1}" -lt 0 ]]; then
      if [[ "${READLINE_POINT:-0}" -gt 0 ]]; then
        READLINE_POINT=$((READLINE_POINT - 1))
      fi
      return 0
    fi
    if [[ "$__mh_history_index" -eq 0 ]]; then
      READLINE_LINE="$__mh_history_saved_line"
      READLINE_POINT="${#READLINE_LINE}"
      __mh_history_reset
    else
      __mh_history_index=$((__mh_history_index - 1))
      __mh_history_load || true
    fi
  }

  __mh_history_older() {
    trap - DEBUG
    __mh_history_step older
    trap '__mh_preexec' DEBUG
  }

  __mh_history_newer() {
    trap - DEBUG
    __mh_history_step newer
    trap '__mh_preexec' DEBUG
  }

  __mh_history_picker() {
    trap - DEBUG
    __mh_history_reset
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
  bind -x '"\e[C": __mh_history_older'
  bind -x '"\eOC": __mh_history_older'
  bind -x '"\e[D": __mh_history_newer'
  bind -x '"\eOD": __mh_history_newer'
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
  typeset -g _mh_history_index=-1
  typeset -g _mh_history_saved_buffer=""
  typeset -g _mh_history_current_buffer=""

  _mh_history_reset() {
    _mh_history_index=-1
    _mh_history_saved_buffer=""
    _mh_history_current_buffer=""
  }

  _mh_history_load() {
    local selected
    selected="$(command mh last 1 --plain --offset "$_mh_history_index" 2>/dev/null)"
    if [[ -n "$selected" ]]; then
      BUFFER="$selected"
      CURSOR=${#BUFFER}
      _mh_history_current_buffer="$selected"
      zle redisplay
      return 0
    fi
    return 1
  }

  _mh_history_older() {
    if (( _mh_history_index >= 0 )) && [[ "$BUFFER" != "$_mh_history_current_buffer" ]]; then
      _mh_history_reset
    fi
    if (( _mh_history_index < 0 && CURSOR < ${#BUFFER} )); then
      zle .forward-char
      return
    fi
    if (( _mh_history_index < 0 )); then
      _mh_history_saved_buffer="$BUFFER"
      _mh_history_index=0
    else
      _mh_history_index=$((_mh_history_index + 1))
    fi
    if ! _mh_history_load; then
      if (( _mh_history_index == 0 )); then
        _mh_history_reset
      else
        _mh_history_index=$((_mh_history_index - 1))
      fi
    fi
  }

  _mh_history_newer() {
    if (( _mh_history_index >= 0 )) && [[ "$BUFFER" != "$_mh_history_current_buffer" ]]; then
      _mh_history_reset
    fi
    if (( _mh_history_index < 0 )); then
      if (( CURSOR > 0 )); then
        zle .backward-char
      fi
      return
    fi
    if (( _mh_history_index == 0 )); then
      BUFFER="$_mh_history_saved_buffer"
      CURSOR=${#BUFFER}
      _mh_history_reset
      zle redisplay
    else
      _mh_history_index=$((_mh_history_index - 1))
      _mh_history_load || true
    fi
  }

  _mh_history_picker() {
    _mh_history_reset
    local selected
    selected="$(command mh pick --limit "${MH_PICK_LIMIT:-100}" </dev/tty)"
    if [[ -n "$selected" ]]; then
      BUFFER="$selected"
      CURSOR=${#BUFFER}
      zle redisplay
    fi
  }

  zle -N _mh_history_older
  zle -N _mh_history_newer
  zle -N _mh_history_picker
  bindkey '^[[A' _mh_history_picker
  bindkey '^[OA' _mh_history_picker
  bindkey '^[[C' _mh_history_older
  bindkey '^[OC' _mh_history_older
  bindkey '^[[D' _mh_history_newer
  bindkey '^[OD' _mh_history_newer
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
  command mh policy check "$cmd" --cwd "$PWD" --quiet 2>/dev/null
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
  mh_history_reset
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

set -g __mh_history_index -1
set -g __mh_history_saved_line ""
set -g __mh_history_current_line ""

function mh_history_reset
  set -g __mh_history_index -1
  set -g __mh_history_saved_line ""
  set -g __mh_history_current_line ""
end

function mh_history_load
  set -l selected (command mh last 1 --plain --offset $__mh_history_index 2>/dev/null)
  if test -n "$selected"
    commandline --replace "$selected"
    commandline --cursor (string length -- "$selected")
    set -g __mh_history_current_line "$selected"
    return 0
  end
  return 1
end

function mh_history_older
  set -l current (commandline)
  if test $__mh_history_index -ge 0; and test "$current" != "$__mh_history_current_line"
    mh_history_reset
  end

  set -l cursor (commandline --cursor)
  if test $__mh_history_index -lt 0; and test $cursor -lt (string length -- "$current")
    commandline --function forward-char
    return
  end

  if test $__mh_history_index -lt 0
    set -g __mh_history_saved_line "$current"
    set -g __mh_history_index 0
  else
    set -g __mh_history_index (math "$__mh_history_index + 1")
  end

  if not mh_history_load
    if test $__mh_history_index -eq 0
      mh_history_reset
    else
      set -g __mh_history_index (math "$__mh_history_index - 1")
    end
  end
end

function mh_history_newer
  set -l current (commandline)
  if test $__mh_history_index -ge 0; and test "$current" != "$__mh_history_current_line"
    mh_history_reset
  end

  if test $__mh_history_index -lt 0
    set -l cursor (commandline --cursor)
    if test $cursor -gt 0
      commandline --function backward-char
    end
    return
  end

  if test $__mh_history_index -eq 0
    commandline --replace "$__mh_history_saved_line"
    commandline --cursor (string length -- "$__mh_history_saved_line")
    mh_history_reset
  else
    set -g __mh_history_index (math "$__mh_history_index - 1")
    mh_history_load
  end
end

bind \e\[A mh_history_picker
bind \eOA mh_history_picker
bind \e\[C mh_history_older
bind \eOC mh_history_older
bind \e\[D mh_history_newer
bind \eOD mh_history_newer
end
"#
);
