#!/usr/bin/env bash
set -euo pipefail

APP_NAME="mh"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${PROJECT_ROOT}/target/package"
BUILD_BINARY=1

log() {
  printf '[mh-package] %s\n' "$*"
}

fail() {
  printf '[mh-package] error: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: scripts/package-deb.sh [options]

Options:
  --no-build           Use the existing target/release/mh binary.
  --output-dir DIR     Write package artifacts into DIR.
  -h, --help           Show this help text.
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-build)
        BUILD_BINARY=0
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fail "--output-dir requires a value"
        OUTPUT_DIR="$2"
        shift
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

require_tools() {
  command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb is required"
  command -v cargo >/dev/null 2>&1 || fail "cargo is required"
  command -v gzip >/dev/null 2>&1 || fail "gzip is required"
}

package_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "${PROJECT_ROOT}/Cargo.toml" | head -n 1
}

package_architecture() {
  if command -v dpkg >/dev/null 2>&1; then
    dpkg --print-architecture
    return 0
  fi

  case "$(uname -m)" in
    x86_64)
      printf 'amd64\n'
      ;;
    aarch64|arm64)
      printf 'arm64\n'
      ;;
    armv7l|armv6l)
      printf 'armhf\n'
      ;;
    *)
      fail "unsupported architecture: $(uname -m)"
      ;;
  esac
}

build_binary() {
  if [[ "${BUILD_BINARY}" -eq 1 ]]; then
    log "Building release binary."
    cargo build --release --manifest-path "${PROJECT_ROOT}/Cargo.toml"
  else
    log "Skipping release build."
  fi

  [[ -x "${PROJECT_ROOT}/target/release/${APP_NAME}" ]] || fail "release binary not found"
}

install_payload() {
  local root="$1"

  install -D -m 755 "${PROJECT_ROOT}/target/release/${APP_NAME}" "${root}/usr/bin/${APP_NAME}"

  "${PROJECT_ROOT}/target/release/${APP_NAME}" completions bash \
    --output "${root}/usr/share/bash-completion/completions/${APP_NAME}"
  "${PROJECT_ROOT}/target/release/${APP_NAME}" completions zsh \
    --output "${root}/usr/share/zsh/vendor-completions/_${APP_NAME}"
  "${PROJECT_ROOT}/target/release/${APP_NAME}" completions fish \
    --output "${root}/usr/share/fish/vendor_completions.d/${APP_NAME}.fish"

  "${PROJECT_ROOT}/target/release/${APP_NAME}" man \
    --output "${root}/usr/share/man/man1/${APP_NAME}.1"
  gzip -9 -n "${root}/usr/share/man/man1/${APP_NAME}.1"
}

write_control_file() {
  local root="$1"
  local version="$2"
  local architecture="$3"
  local installed_size

  installed_size="$(du -sk "${root}/usr" | awk '{print $1}')"
  mkdir -p "${root}/DEBIAN"
  cat > "${root}/DEBIAN/control" <<EOF
Package: ${APP_NAME}
Version: ${version}
Section: utils
Priority: optional
Architecture: ${architecture}
Maintainer: Cuma Kurt <cumakurt@gmail.com>
Installed-Size: ${installed_size}
Depends: libc6
Homepage: https://github.com/cumakurt/mh
Description: Modern Linux command history manager
 mh records shell commands into SQLite and provides search, statistics,
 shell integration, snippets, encrypted vault storage, and a terminal UI.
EOF
}

build_deb() {
  local version="$1"
  local architecture="$2"
  local package_root="${OUTPUT_DIR}/${APP_NAME}_${version}_${architecture}"
  local deb_path="${OUTPUT_DIR}/${APP_NAME}_${version}_${architecture}.deb"

  rm -rf "${package_root}"
  mkdir -p "${package_root}"
  install_payload "${package_root}"
  write_control_file "${package_root}" "${version}" "${architecture}"

  dpkg-deb --root-owner-group --build "${package_root}" "${deb_path}" >/dev/null
  log "Built ${deb_path}"
}

main() {
  parse_args "$@"
  require_tools
  mkdir -p "${OUTPUT_DIR}"

  local version
  local architecture
  version="$(package_version)"
  [[ -n "${version}" ]] || fail "failed to read package version"
  architecture="$(package_architecture)"

  build_binary
  build_deb "${version}" "${architecture}"
}

main "$@"
