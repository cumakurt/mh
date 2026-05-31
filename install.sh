#!/usr/bin/env bash
set -euo pipefail

APP_NAME="mh"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_SYSTEM_INSTALL_DIR="/usr/local/bin"
DEFAULT_USER_INSTALL_DIR="${HOME}/.local/bin"
ORIGINAL_PATH="${PATH:-}"

INSTALL_DIR="${INSTALL_DIR:-}"
REQUESTED_SHELL="${MH_SHELL:-}"
INSTALL_DEPS=1
BUILD_BINARY=1
ENABLE_SHELL=1
INSTALL_COMPLETIONS=1
INSTALL_MAN_PAGE=1
INSTALL_SCOPE="auto"

log() {
  printf '[mh-install] %s\n' "$*"
}

warn() {
  printf '[mh-install] warning: %s\n' "$*" >&2
}

fail() {
  printf '[mh-install] error: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: ./install.sh [options]

Options:
  --no-deps            Skip system dependency installation.
  --no-build           Skip cargo build and install the existing release binary.
  --no-enable          Skip shell integration setup.
  --no-completions     Skip shell completion installation.
  --no-man             Skip man page installation.
  --shell SHELL        Override detected shell: bash, zsh, fish, or nushell.
  --install-dir DIR    Install the mh binary into DIR.
  --user               Install into ~/.local/bin.
  --system             Install into /usr/local/bin.
  -h, --help           Show this help text.

Environment:
  INSTALL_DIR          Override the binary install directory.
  MH_SHELL             Override detected shell.
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-deps)
        INSTALL_DEPS=0
        ;;
      --no-build)
        BUILD_BINARY=0
        ;;
      --no-enable)
        ENABLE_SHELL=0
        ;;
      --no-completions)
        INSTALL_COMPLETIONS=0
        ;;
      --no-man)
        INSTALL_MAN_PAGE=0
        ;;
      --shell)
        [[ $# -ge 2 ]] || fail "--shell requires a value"
        REQUESTED_SHELL="$2"
        shift
        ;;
      --install-dir)
        [[ $# -ge 2 ]] || fail "--install-dir requires a value"
        INSTALL_DIR="$2"
        shift
        ;;
      --user)
        INSTALL_SCOPE="user"
        ;;
      --system)
        INSTALL_SCOPE="system"
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown option: $1"
        ;;
    esac
    shift
  done
}

require_linux() {
  local kernel
  kernel="$(uname -s)"
  [[ "$kernel" == "Linux" ]] || fail "this installer currently supports Linux only"
}

detect_os() {
  OS_ID="unknown"
  OS_ID_LIKE=""
  OS_NAME="unknown Linux"

  if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    OS_ID="${ID:-unknown}"
    OS_ID_LIKE="${ID_LIKE:-}"
    OS_NAME="${PRETTY_NAME:-${NAME:-unknown Linux}}"
  fi

  log "Detected operating system: ${OS_NAME}"
}

have_command() {
  command -v "$1" >/dev/null 2>&1
}

sudo_cmd() {
  if [[ "${EUID}" -eq 0 ]]; then
    return 0
  fi

  have_command sudo || fail "sudo is required for system package installation"
  printf 'sudo'
}

detect_package_manager() {
  if have_command apt-get; then
    PACKAGE_MANAGER="apt"
  elif have_command dnf; then
    PACKAGE_MANAGER="dnf"
  elif have_command yum; then
    PACKAGE_MANAGER="yum"
  elif have_command pacman; then
    PACKAGE_MANAGER="pacman"
  elif have_command zypper; then
    PACKAGE_MANAGER="zypper"
  elif have_command apk; then
    PACKAGE_MANAGER="apk"
  else
    PACKAGE_MANAGER="unknown"
  fi

  log "Detected package manager: ${PACKAGE_MANAGER}"
}

ensure_package_manager_temp_dir() {
  local tmp="${HOME}/.cache/mh-install/tmp"
  mkdir -p "${tmp}"
  chmod 700 "${tmp}" 2>/dev/null || true
  export TMPDIR="${tmp}"
}

apt_build_dependencies_ready() {
  local pkg
  for pkg in build-essential pkg-config curl ca-certificates git; do
    dpkg -s "${pkg}" >/dev/null 2>&1 || return 1
  done
  return 0
}

install_system_dependencies() {
  [[ "${INSTALL_DEPS}" -eq 1 ]] || {
    log "Skipping system dependency installation."
    return 0
  }

  local sudo
  sudo="$(sudo_cmd)"

  case "${PACKAGE_MANAGER}" in
    apt)
      ensure_package_manager_temp_dir
      if apt_build_dependencies_ready; then
        log "Build dependencies already installed; skipping apt-get update."
      else
        ${sudo:+$sudo} apt-get update
      fi
      ${sudo:+$sudo} apt-get install -y build-essential pkg-config curl ca-certificates git
      ;;
    dnf)
      ${sudo:+$sudo} dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl ca-certificates git
      ;;
    yum)
      ${sudo:+$sudo} yum install -y gcc gcc-c++ make pkgconfig curl ca-certificates git
      ;;
    pacman)
      ${sudo:+$sudo} pacman -Sy --needed --noconfirm base-devel pkgconf curl ca-certificates git
      ;;
    zypper)
      ${sudo:+$sudo} zypper --non-interactive install -y gcc gcc-c++ make pkg-config curl ca-certificates git
      ;;
    apk)
      ${sudo:+$sudo} apk add --no-cache build-base pkgconfig curl ca-certificates git bash
      ;;
    *)
      fail "unsupported package manager; rerun with --no-deps after installing Rust, git, curl, ca-certificates, pkg-config, make, and a C compiler"
      ;;
  esac
}

load_cargo_env() {
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    . "${HOME}/.cargo/env"
  fi
}

install_rust_toolchain() {
  load_cargo_env
  if have_command cargo && have_command rustc; then
    log "Rust toolchain is already available."
    return 0
  fi

  have_command curl || fail "curl is required to install Rust with rustup"
  log "Installing Rust with rustup."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  load_cargo_env
  have_command cargo || fail "cargo was not found after rustup installation"
}

build_release_binary() {
  if [[ "${BUILD_BINARY}" -eq 1 ]]; then
    log "Building release binary."
    cargo build --release --manifest-path "${PROJECT_ROOT}/Cargo.toml"
  else
    log "Skipping release build."
  fi

  [[ -x "${PROJECT_ROOT}/target/release/${APP_NAME}" ]] || fail "release binary not found at target/release/${APP_NAME}"
}

choose_install_dir() {
  if [[ -n "${INSTALL_DIR}" ]]; then
    return 0
  fi

  case "${INSTALL_SCOPE}" in
    user)
      INSTALL_DIR="${DEFAULT_USER_INSTALL_DIR}"
      ;;
    system)
      INSTALL_DIR="${DEFAULT_SYSTEM_INSTALL_DIR}"
      ;;
    auto)
      if [[ "${EUID}" -eq 0 ]] || have_command sudo; then
        INSTALL_DIR="${DEFAULT_SYSTEM_INSTALL_DIR}"
      else
        INSTALL_DIR="${DEFAULT_USER_INSTALL_DIR}"
      fi
      ;;
    *)
      fail "invalid install scope: ${INSTALL_SCOPE}"
      ;;
  esac
}

install_binary() {
  choose_install_dir
  log "Installing ${APP_NAME} into ${INSTALL_DIR}."

  if [[ "${INSTALL_DIR}" == "${HOME}"* ]]; then
    mkdir -p "${INSTALL_DIR}"
    install -m 755 "${PROJECT_ROOT}/target/release/${APP_NAME}" "${INSTALL_DIR}/${APP_NAME}"
  else
    local sudo
    if [[ "${EUID}" -eq 0 ]]; then
      sudo=""
    else
      have_command sudo || fail "sudo is required to install into ${INSTALL_DIR}; rerun with --user"
      sudo="sudo"
    fi
    ${sudo:+$sudo} mkdir -p "${INSTALL_DIR}"
    ${sudo:+$sudo} install -m 755 "${PROJECT_ROOT}/target/release/${APP_NAME}" "${INSTALL_DIR}/${APP_NAME}"
  fi

  [[ -x "${INSTALL_DIR}/${APP_NAME}" ]] || fail "installed binary is not executable"
  if ! path_contains_install_dir; then
    export PATH="${INSTALL_DIR}:${PATH}"
  fi
}

is_user_install() {
  [[ "${INSTALL_SCOPE}" == "user" || "${INSTALL_DIR}" == "${HOME}"* ]]
}

install_data_file() {
  local source_file="$1"
  local target_file="$2"
  local target_dir

  target_dir="$(dirname "${target_file}")"
  if [[ "${target_file}" == "${HOME}"* ]]; then
    mkdir -p "${target_dir}"
    install -m 644 "${source_file}" "${target_file}"
  else
    local sudo
    if [[ "${EUID}" -eq 0 ]]; then
      sudo=""
    else
      have_command sudo || fail "sudo is required to install ${target_file}; rerun with --user"
      sudo="sudo"
    fi
    ${sudo:+$sudo} mkdir -p "${target_dir}"
    ${sudo:+$sudo} install -m 644 "${source_file}" "${target_file}"
  fi
}

install_completions() {
  [[ "${INSTALL_COMPLETIONS}" -eq 1 ]] || {
    log "Skipping shell completion installation."
    return 0
  }

  local bash_target
  local zsh_target
  local fish_target
  local tmp_file

  if is_user_install; then
    bash_target="${HOME}/.local/share/bash-completion/completions/${APP_NAME}"
    zsh_target="${HOME}/.local/share/zsh/site-functions/_${APP_NAME}"
    fish_target="${XDG_CONFIG_HOME:-${HOME}/.config}/fish/completions/${APP_NAME}.fish"
  else
    bash_target="/usr/share/bash-completion/completions/${APP_NAME}"
    zsh_target="/usr/local/share/zsh/site-functions/_${APP_NAME}"
    fish_target="/usr/share/fish/vendor_completions.d/${APP_NAME}.fish"
  fi

  tmp_file="$(mktemp)"
  "${INSTALL_DIR}/${APP_NAME}" completions bash --output "${tmp_file}"
  install_data_file "${tmp_file}" "${bash_target}"
  "${INSTALL_DIR}/${APP_NAME}" completions zsh --output "${tmp_file}"
  install_data_file "${tmp_file}" "${zsh_target}"
  "${INSTALL_DIR}/${APP_NAME}" completions fish --output "${tmp_file}"
  install_data_file "${tmp_file}" "${fish_target}"
  rm -f "${tmp_file}"

  log "Installed shell completions."
}

install_man_page() {
  [[ "${INSTALL_MAN_PAGE}" -eq 1 ]] || {
    log "Skipping man page installation."
    return 0
  }

  local man_target
  local tmp_file

  if is_user_install; then
    man_target="${HOME}/.local/share/man/man1/${APP_NAME}.1"
  else
    man_target="/usr/local/share/man/man1/${APP_NAME}.1"
  fi

  tmp_file="$(mktemp)"
  "${INSTALL_DIR}/${APP_NAME}" man --output "${tmp_file}"
  install_data_file "${tmp_file}" "${man_target}"
  rm -f "${tmp_file}"

  log "Installed man page."
}

detect_shell() {
  local shell_name="${REQUESTED_SHELL}"

  if [[ -z "${shell_name}" && -n "${SHELL:-}" ]]; then
    shell_name="$(basename "${SHELL}")"
  fi

  if [[ -z "${shell_name}" ]]; then
    shell_name="$(ps -p "$$" -o comm= 2>/dev/null | awk '{print $1}')"
  fi

  case "${shell_name}" in
    bash)
      DETECTED_SHELL="bash"
      ;;
    zsh)
      DETECTED_SHELL="zsh"
      ;;
    fish)
      DETECTED_SHELL="fish"
      ;;
    nu|nushell)
      DETECTED_SHELL="nushell"
      ;;
    *)
      DETECTED_SHELL="unsupported"
      ;;
  esac

  log "Detected shell: ${DETECTED_SHELL}"
}

shell_config_candidates() {
  case "${DETECTED_SHELL}" in
    bash)
      printf '%s\n' "${HOME}/.bashrc" "${HOME}/.bash_profile" "${HOME}/.profile"
      ;;
    zsh)
      printf '%s\n' "${HOME}/.zshrc" "${HOME}/.zshenv"
      ;;
    fish)
      printf '%s\n' "${XDG_CONFIG_HOME:-${HOME}/.config}/fish/config.fish"
      ;;
    nushell)
      printf '%s\n' "${XDG_CONFIG_HOME:-${HOME}/.config}/nushell/config.nu"
      ;;
    *)
      return 1
      ;;
  esac
}

shell_config_path() {
  local candidate=""
  while IFS= read -r candidate; do
    if [[ -f "${candidate}" ]]; then
      printf '%s' "${candidate}"
      return 0
    fi
  done < <(shell_config_candidates)

  shell_config_candidates | head -n 1
}

refuse_symlink_config() {
  local config_file="$1"
  if [[ -L "${config_file}" ]]; then
    warn "refusing to modify symlinked shell config: ${config_file}"
    return 1
  fi
  return 0
}

path_contains_install_dir() {
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) return 0 ;;
    *) return 1 ;;
  esac
}

original_path_contains_install_dir() {
  case ":${ORIGINAL_PATH}:" in
    *":${INSTALL_DIR}:"*) return 0 ;;
    *) return 1 ;;
  esac
}

append_path_to_shell_config() {
  original_path_contains_install_dir && return 0

  local config_file
  config_file="$(shell_config_path)"
  refuse_symlink_config "${config_file}" || return 0
  local directory
  directory="$(dirname "${config_file}")"
  mkdir -p "${directory}"
  touch "${config_file}"

  case "${DETECTED_SHELL}" in
    bash|zsh)
      if ! grep -Fq "${INSTALL_DIR}" "${config_file}" 2>/dev/null; then
        {
          printf '\n# mh installer PATH\n'
          printf 'export PATH="%s:$PATH"\n' "${INSTALL_DIR}"
        } >> "${config_file}"
      fi
      ;;
    fish)
      if ! grep -Fq "${INSTALL_DIR}" "${config_file}" 2>/dev/null; then
        {
          printf '\n# mh installer PATH\n'
          printf 'if not contains "%s" $PATH\n' "${INSTALL_DIR}"
          printf '  set -gx PATH "%s" $PATH\n' "${INSTALL_DIR}"
          printf 'end\n'
        } >> "${config_file}"
      fi
      ;;
    nushell)
      if ! grep -Fq "${INSTALL_DIR}" "${config_file}" 2>/dev/null; then
        {
          printf '\n# mh installer PATH\n'
          printf '$env.PATH = ($env.PATH | prepend "%s" | uniq)\n' "${INSTALL_DIR}"
        } >> "${config_file}"
      fi
      ;;
  esac
}

append_zsh_completion_fpath() {
  [[ "${DETECTED_SHELL}" == "zsh" ]] || return 0
  [[ "${INSTALL_COMPLETIONS}" -eq 1 ]] || return 0

  local config_file
  local site_functions
  config_file="$(shell_config_path)"
  refuse_symlink_config "${config_file}" || return 0
  site_functions="${HOME}/.local/share/zsh/site-functions"
  [[ -d "${site_functions}" ]] || return 0
  if grep -Fq 'site-functions' "${config_file}" 2>/dev/null; then
    return 0
  fi
  {
    printf '\n# mh installer zsh completions\n'
    printf 'fpath=( "%s" $fpath )\n' "${site_functions}"
  } >> "${config_file}"
}

enable_shell_integration() {
  [[ "${ENABLE_SHELL}" -eq 1 ]] || {
    log "Skipping shell integration setup."
    return 0
  }

  detect_shell
  if [[ "${DETECTED_SHELL}" == "unsupported" ]]; then
    warn "unsupported shell; run '${APP_NAME} init <shell>' manually for bash, zsh, fish, or nushell"
    return 0
  fi

  local config_file
  config_file="$(shell_config_path)"
  append_path_to_shell_config
  append_zsh_completion_fpath

  if "${INSTALL_DIR}/${APP_NAME}" init "${DETECTED_SHELL}" --install; then
    log "Enabled ${DETECTED_SHELL} integration in ${config_file} via mh init --install."
  elif "${INSTALL_DIR}/${APP_NAME}" init "${DETECTED_SHELL}" --repair; then
    log "Repaired ${DETECTED_SHELL} integration in ${config_file} via mh init --repair."
  else
    warn "shell integration setup failed; run '${APP_NAME} init ${DETECTED_SHELL} --repair' manually"
  fi
}

run_post_install_fixes() {
  if "${INSTALL_DIR}/${APP_NAME}" config fix; then
    log "Applied config permission and legacy pattern fixes."
  else
    warn "config fix failed; run '${APP_NAME} config fix' manually"
  fi

  if ! "${INSTALL_DIR}/${APP_NAME}" audit --verify-chain >/dev/null 2>&1; then
    if "${INSTALL_DIR}/${APP_NAME}" audit --rebuild-chain --yes >/dev/null 2>&1; then
      log "Rebuilt audit hash chain for legacy entries."
    else
      warn "audit chain repair failed; run '${APP_NAME} audit --rebuild-chain --yes' manually"
    fi
  fi
}

run_doctor() {
  if "${INSTALL_DIR}/${APP_NAME}" doctor; then
    return 0
  fi
  warn "doctor reported a problem; review the output above"
}

main() {
  parse_args "$@"
  require_linux
  detect_os
  detect_package_manager

  if [[ "${BUILD_BINARY}" -eq 1 ]]; then
    install_system_dependencies
    install_rust_toolchain
  else
    log "Skipping build dependency checks because --no-build was selected."
  fi

  build_release_binary
  install_binary
  install_completions
  install_man_page
  enable_shell_integration
  run_post_install_fixes
  run_doctor

  log "Installation completed."
  log "Open a new shell session or reload your shell config to activate command recording."
}

main "$@"
