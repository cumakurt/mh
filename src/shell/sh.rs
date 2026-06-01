//! POSIX `sh` / `dash` integration using `fc` history (no DEBUG trap).

pub const INTEGRATION: &str = concat!(
    "# mh shell integration for POSIX sh (dash, ash, /bin/sh)\n",
    r#"
# If this shell is bash acting as sh, use: mh init bash --install
if [ -n "${BASH_VERSION-}" ]; then
  echo "mh: use 'mh init bash' when running bash as /bin/sh" >&2
else

if [ -z "${__MH_SH_LOADED-}" ]; then
__MH_SH_LOADED=1

if [ -z "${MH_SESSION_ID-}" ]; then
  MH_SESSION_ID="$(date +%s)-$$"
  export MH_SESSION_ID
fi
MH_SKIP_GIT_DETECT=1
export MH_SKIP_GIT_DETECT

__mh_now_ms() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null && return 0
  fi
  if command -v perl >/dev/null 2>&1; then
    perl -MTime::HiRes=time -e 'print int(time()*1000)' 2>/dev/null && return 0
  fi
  _mh_ms="$(date +%s%3N 2>/dev/null || true)"
  case $_mh_ms in
    ''|*[!0-9]*) printf '%s000' "$(date +%s 2>/dev/null || echo 0)" ;;
    *) printf '%s' "$_mh_ms" ;;
  esac
}

__mh_record() {
  if [ -n "${MH_RECORD_VERBOSE-}" ] || [ -n "${MH_POLICY_VERBOSE-}" ]; then
    command mh record "$@" 1>&2
  else
    command mh record "$@" >/dev/null 2>&1
  fi
}

__mh_should_skip() {
  case $1 in
    ''|__mh_*|mh\ *|command\ mh\ *|*:mh\ *|fc\ *|trap\ *|export\ MH_*|set\ *) return 0 ;;
  esac
  return 1
}

__mh_fc_last_command() {
  fc -ln 2>/dev/null | tail -n 1 | sed -e 's/^[[:space:]]*[0-9][[:space:]]*//' -e 's/^[[:space:]]*//'
}

__mh_before_prompt() {
  _mh_exit=$?
  if [ -n "${MH_PENDING_CMD-}" ]; then
    if ! __mh_should_skip "$MH_PENDING_CMD"; then
      _mh_end="$(__mh_now_ms 2>/dev/null || echo 0)"
      _mh_dur=0
      if [ -n "${MH_PENDING_START-}" ]; then
        _mh_dur=$((_mh_end - MH_PENDING_START))
      fi
      __mh_record \
        --command "$MH_PENDING_CMD" \
        --cwd "$PWD" \
        --shell "sh" \
        --exit-code "$_mh_exit" \
        --duration-ms "$_mh_dur" \
        --session-id "$MH_SESSION_ID"
    fi
  fi
  MH_PENDING_CMD="$(__mh_fc_last_command)"
  MH_PENDING_START="$(__mh_now_ms 2>/dev/null || echo 0)"
  return $_mh_exit
}

set -o history 2>/dev/null || true

case ${PS1-} in
  *'__mh_before_prompt'*) ;;
  *) PS1='`__mh_before_prompt 2>/dev/null`'${PS1:-'$ '} ;;
esac

fi
fi
"#
);
