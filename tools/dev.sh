#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

setup_only=0
skip_ui_install=0
skip_system_deps=0
cargo_args=()

usage() {
  cat <<'USAGE'
Usage: bash tools/dev.sh [--setup-only] [--skip-system-deps] [--skip-ui-install] [cargo run args...]

Examples:
  bash tools/dev.sh
  bash tools/dev.sh -- --dev
  bash tools/dev.sh --setup-only
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --setup-only)
      setup_only=1
      shift
      ;;
    --skip-ui-install)
      skip_ui_install=1
      shift
      ;;
    --skip-system-deps)
      skip_system_deps=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      cargo_args+=("$1")
      shift
      cargo_args+=("$@")
      break
      ;;
    *)
      cargo_args+=("$1")
      shift
      ;;
  esac
done

step() {
  printf '\n==> %s\n' "$1"
}

run_sudo() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

load_cargo_env() {
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    . "${HOME}/.cargo/env"
  fi
}

ensure_linux_system_deps() {
  step "Linux desktop build dependencies"

  if command -v apt-get >/dev/null 2>&1; then
    local packages=(
      libwebkit2gtk-4.1-dev
      libasound2-dev
      libusb-1.0-0-dev
      build-essential
      curl
      wget
      file
      libxdo-dev
      libssl-dev
      libudev-dev
      pkg-config
      libayatana-appindicator3-dev
      librsvg2-dev
    )
    local missing=()
    local package
    for package in "${packages[@]}"; do
      if ! dpkg-query -W -f='${Status}' "${package}" 2>/dev/null | grep -q "install ok installed"; then
        missing+=("${package}")
      fi
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
      run_sudo apt-get update
      run_sudo apt-get install -y "${missing[@]}"
    else
      echo "Linux desktop packages found."
    fi
  elif command -v dnf >/dev/null 2>&1; then
    local packages=(
      webkit2gtk4.1-devel
      alsa-lib-devel
      libusb1-devel
      openssl-devel
      systemd-devel
      pkgconf-pkg-config
      curl
      wget
      file
      libappindicator-gtk3-devel
      librsvg2-devel
      libxdo-devel
    )
    local missing=()
    local package
    for package in "${packages[@]}"; do
      if ! rpm -q "${package}" >/dev/null 2>&1; then
        missing+=("${package}")
      fi
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
      run_sudo dnf install -y "${missing[@]}"
    else
      echo "Linux desktop packages found."
    fi
    if ! command -v cc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1; then
      run_sudo dnf group install -y "c-development" || run_sudo dnf group install -y "Development Tools"
    fi
  elif command -v pacman >/dev/null 2>&1; then
    run_sudo pacman -Syu --needed \
      webkit2gtk-4.1 \
      alsa-lib \
      libusb \
      base-devel \
      curl \
      wget \
      file \
      openssl \
      pkgconf \
      systemd \
      appmenu-gtk-module \
      libappindicator-gtk3 \
      librsvg \
      xdotool
  elif command -v zypper >/dev/null 2>&1; then
    local packages=(
      webkit2gtk3-devel
      alsa-devel
      libusb-1_0-devel
      libopenssl-devel
      libudev-devel
      pkg-config
      curl
      wget
      file
      libappindicator3-1
      librsvg-devel
    )
    local missing=()
    local package
    for package in "${packages[@]}"; do
      if ! rpm -q "${package}" >/dev/null 2>&1; then
        missing+=("${package}")
      fi
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
      run_sudo zypper --non-interactive refresh
      run_sudo zypper --non-interactive install "${missing[@]}"
    else
      echo "Linux desktop packages found."
    fi
    if ! command -v cc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1; then
      run_sudo zypper --non-interactive install -t pattern devel_basis
    fi
  elif command -v apk >/dev/null 2>&1; then
    run_sudo apk add \
      build-base \
      webkit2gtk-4.1-dev \
      alsa-lib-dev \
      libusb-dev \
      curl \
      wget \
      file \
      openssl \
      pkgconf \
      eudev-dev \
      libayatana-appindicator-dev \
      librsvg
  else
    echo "Unsupported Linux package manager. Install the Tauri Linux prerequisites manually, then rerun this script." >&2
  fi
}

ensure_macos_system_deps() {
  step "macOS desktop build dependencies"

  if ! xcode-select -p >/dev/null 2>&1; then
    xcode-select --install || true
    echo "Finish the Xcode Command Line Tools installer, then rerun bash tools/dev.sh." >&2
    exit 1
  fi
}

ensure_system_deps() {
  if [[ "${skip_system_deps}" -eq 1 ]]; then
    step "Desktop build dependencies"
    echo "Skipping system dependency install."
    return
  fi

  case "$(uname -s)" in
    Linux)
      ensure_linux_system_deps
      ;;
    Darwin)
      ensure_macos_system_deps
      ;;
    *)
      echo "Unsupported Unix platform: $(uname -s)" >&2
      exit 1
      ;;
  esac
}

ensure_rustup() {
  load_cargo_env
  if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
    load_cargo_env
  fi
  if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup was installed but is still not on PATH. Restart the shell, then rerun bash tools/dev.sh." >&2
    exit 1
  fi
}

activate_canonical_toolchain() {
  step "Canonical Rust, Node, npm, and Python contract"
  if ! command -v python3 >/dev/null 2>&1; then
    echo "Python 3 is required before bootstrap; install the version recorded in tools/bootstrap/toolchain.json." >&2
    exit 1
  fi
  ensure_rustup
  sh "$repo_root/tools/bootstrap/install-rust-toolchain.sh"
  local node_bin
  node_bin="$(sh "$repo_root/tools/bootstrap/install-node.sh")"
  PATH="$node_bin:$PATH"
  export PATH
  sh "$repo_root/tools/bootstrap/verify-toolchain.sh" --check-installed
}

ensure_ui_dependencies() {
  step "Svelte dependencies"

  if [[ "${skip_ui_install}" -eq 1 ]]; then
    echo "Skipping npm ci."
    return
  fi

  if [[ -f node_modules/.package-lock.json && ! package-lock.json -nt node_modules/.package-lock.json ]]; then
    echo "node_modules is current."
    return
  fi

  npm ci
}

run_app() {
  step "Run Chataigne2"
  cargo run "${cargo_args[@]}"
}

ensure_system_deps
activate_canonical_toolchain
ensure_ui_dependencies

if [[ "${setup_only}" -eq 0 ]]; then
  run_app
fi
