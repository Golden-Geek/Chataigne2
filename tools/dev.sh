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

ensure_git_submodules() {
  step "Git submodules"

  if ! command -v git >/dev/null 2>&1; then
    echo "git was not found on PATH. Install Git, then rerun bash tools/dev.sh." >&2
    exit 1
  fi

  git submodule update --init --recursive
}

ensure_linux_system_deps() {
  step "Linux desktop build dependencies"

  if command -v apt-get >/dev/null 2>&1; then
    local packages=(
      libwebkit2gtk-4.1-dev
      libasound2-dev
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

ensure_rust() {
  step "Rust toolchain"
  load_cargo_env

  if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    load_cargo_env
  fi

  if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup was installed but is still not on PATH. Restart the shell, then rerun bash tools/dev.sh." >&2
    exit 1
  fi

  rustup toolchain install stable
  rustup default stable
  load_cargo_env

  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found after installing rustup. Restart the shell, then rerun bash tools/dev.sh." >&2
    exit 1
  fi

  cargo --version
}

node_supported() {
  if ! command -v node >/dev/null 2>&1; then
    return 1
  fi

  local version major minor patch
  version="$(node -p 'process.versions.node' 2>/dev/null || true)"
  IFS=. read -r major minor patch <<< "${version}"

  if [[ ! "${major}" =~ ^[0-9]+$ || ! "${minor}" =~ ^[0-9]+$ ]]; then
    return 1
  fi

  if (( major == 20 && minor >= 19 )); then
    return 0
  fi

  if (( major == 22 && minor >= 12 )); then
    return 0
  fi

  if (( major >= 23 )); then
    return 0
  fi

  return 1
}

load_nvm() {
  export NVM_DIR="${NVM_DIR:-${HOME}/.nvm}"
  if [[ -s "${NVM_DIR}/nvm.sh" ]]; then
    # shellcheck disable=SC1091
    . "${NVM_DIR}/nvm.sh"
  fi
}

ensure_nvm() {
  load_nvm

  if command -v nvm >/dev/null 2>&1; then
    return
  fi

  local nvm_version
  nvm_version="${NVM_INSTALL_VERSION:-v0.40.4}"
  curl -o- "https://raw.githubusercontent.com/nvm-sh/nvm/${nvm_version}/install.sh" | bash
  load_nvm

  if ! command -v nvm >/dev/null 2>&1; then
    echo "nvm was installed but could not be loaded. Restart the shell, then rerun bash tools/dev.sh." >&2
    exit 1
  fi
}

ensure_node() {
  step "Node.js and npm"
  load_nvm

  if ! node_supported || ! command -v npm >/dev/null 2>&1; then
    ensure_nvm
    nvm install --lts
    nvm use --lts
  fi

  if ! node_supported; then
    local found
    found="$(command -v node >/dev/null 2>&1 && node --version || echo "not found")"
    echo "Node.js 20.19+ or 22.12+ is required by the Svelte/Vite frontend. Found: ${found}." >&2
    exit 1
  fi

  if ! command -v npm >/dev/null 2>&1; then
    echo "npm was not found after installing Node.js. Restart the shell, then rerun bash tools/dev.sh." >&2
    exit 1
  fi

  node --version
  npm --version
}

ensure_ui_dependencies() {
  step "Svelte dependencies"

  if [[ "${skip_ui_install}" -eq 1 ]]; then
    echo "Skipping npm ci."
    return
  fi

  if [[ -f src-ui/node_modules/.package-lock.json && ! src-ui/package-lock.json -nt src-ui/node_modules/.package-lock.json ]]; then
    echo "src-ui/node_modules is current."
    return
  fi

  (cd src-ui && npm ci)
}

run_app() {
  step "Run Chataigne2"
  cargo run "${cargo_args[@]}"
}

ensure_git_submodules
ensure_system_deps
ensure_rust
ensure_node
ensure_ui_dependencies

if [[ "${setup_only}" -eq 0 ]]; then
  run_app
fi
